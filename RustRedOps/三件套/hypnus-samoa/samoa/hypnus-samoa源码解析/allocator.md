

## ffi的调用方式见文件夹win api abi中rust-ffi方式


## 为什么不能使用dinvk::get_proc_address 

allocator.rs为啥不用dinvk::get_proc_address 这种可以完全抹除 IAT的调用方式:
1. 会产生“鸡生蛋，蛋生鸡”的自举（Bootstrapping）死锁
2. 使用let addr =  get_proc_address("RtlAllocateHeap");时
3. 在执行 get_proc_address 的过程中，不管是遍历 PEB，还是计算哈希，只要底层的某一行代码（或者某个隐式调用的 Rust 标准库函数）需要分配哪怕 1 个字节的堆内存（比如拼接了一个局部字符串，或者用到了 Vec）
4. 触发内存分配 ,Rust 呼叫全局分配器 alloc
5. 全局分配器 alloc 启动，发现 RtlAllocateHeap 还没解析出来，于是再次呼叫get_proc_address
6. 无限递归，瞬间栈溢出（Stack Overflow），木马在启动的第 0.001秒就崩溃了


## 必须使用私有堆

hypnus在主线程休眠期间,将整个内存堆遍历一遍,对所有正在使用的数据进行了xor加密(hypnus.rs的fn obfuscate_heap)
1. 如果使用默认进程堆(GetProcessHeap):宿主进程(简单如natepade.exe)本身也有后台线程在运行.如果使用默认堆,那么这时在进行xor加密时,合法线程去读取它原本数据时,读到的全部是乱码,这会十年间导致进程崩溃(Access Violation).
2. 必须使用RtlCreateHeap创建私有堆.这样只加密自己的堆中的数据,不影响宿主进程的正常运转
3. 私有堆带来轻微IoC,这里IoC是什么?

**这里新建了私有堆,那么在程序运行期间,系统默认堆是否仍然被使用**
1. 在 Windows 底层进程模型中私有堆和系统默认堆是共存的
2. 程序启动时，ntdll.dll会在执行您的代码之前，强制创建一个唯一的默认进程堆（Default Process Heap）
3. 即使通过 RtlCreateHeap 创建了私有堆，并将其设为`#[global_allocator]`，系统默认堆依然在运行.

## static mut HEAP_HANDLE

### 单线程多线程

这里的```rust static mut HEAP_HANDLE: Option<NonNull<c_void>> = None;```这里是多线程还是单线程环境:
1. 在Timer/Wait 模式下：通过`TpSetPoolMinThreads(pool, 1);`和`TpSetPoolMaxThreads(pool, 1);`创建私有线程池,强制最大线程数为1.所有ROP链,加密解密操作,都是串行分发给这个唯一worker线程的
2. 在Foliage (APC) 模式下：所有任务被压入一个处于挂起状态的单一傀儡线程的 APC 队列中，APC队列天生就是 FIFO（先进先出）串行执行的
3. 在hypnus.rs中执行流是被严格编排的.确保了单线程执行流.


### static mut HEAP_HANDLE引入spin::Once

是否引对static mut HEAP_HANDLE引入spin::Once是否真的有必要:
1. 没有必要.如果用 spin::Once,虽然满足了lazy加载,但每一次极微小的内存分配（alloc），底层都要执行一次 Atomic 操作（原子检查），这会带来微小的性能损耗。此外，引入外部 Crate 会轻微膨胀二进制体积
2. 在确认了初始化时序安全后，static mut 编译出的汇编指令，就是一次毫无开销的绝对内存地址寻址.在勒索软件或高级木马中，如果需要频繁对碎片化数据进行加密解密（大量小内存分配），省掉这一丝性能开销就是胜利。这是为了极致性能做出的极简妥协


## IAT污染

windows_targets::link!在最终生成的 DLL/EXE 的导入表中留下RtlCreateHeap、RtlAllocateHeap 和 RtlFreeHeap 的字符串.这是否足够关键,足以影响隐蔽性.作者在这里没有调用dinvk::module::get_proc_address.
1. 如果是 VirtualAlloc、CreateRemoteThread 出现在 IAT足以致命.。但 RtlCreateHeap 和 RtlAllocateHeap是最底层的内存原语.任何一个普通的 C/C++ 业务程序（只要它用到了 malloc 或new）在底层往往都会依赖这些函数.EDR 几乎不可能仅凭这三个函数在 IAT里就判定您是恶意软件，它太常见了
2. 不用dinvk::get_proc_address是绝对正确且必须的.allocator.rs本身就是用来分配代码中用到的堆内存的,dinvk::get_proc_address在查找函数地址时如果用到堆内存,会导致无限递归/栈溢出(鸡生蛋问题)



## 分配/释放时清零

在 RtlAllocateHeap 成功后，立刻调用core::ptr::write_bytes(ptr, 0, size)之前没有对分配的内存清零,可能读取到之前的脏数据.这里不是重新分配的一块内存吗?怎么会有脏数据?真正应该关心的是 RtlFreeHeap后对内存清零吧?  
**底层开发中最大的幻觉：**以为新分配的内存是干净的.Windows 的堆管理器（Heap Manager）中，为了追求极速，当调用 RtlFreeHeap释放一块内存后，系统绝对不会去把里面的数据抹掉，它只是把这块内存挂回了空闲链表（Free List）.当您下次调用 RtlAllocateHeap时，系统极大概率会把刚才那块内存原封不动地塞回
1. dealloc 时清零：是为了防止内存扫描器发现死去的敏感数据
2. alloc 时清零：是为了确保拿到的是一张“绝对白纸”，防止脏数据意外泄露或干扰解密算法
3. 对于高级载荷，性能为安全让路，多花几个时钟周期调用一次清零，换来的是绝对的确定性