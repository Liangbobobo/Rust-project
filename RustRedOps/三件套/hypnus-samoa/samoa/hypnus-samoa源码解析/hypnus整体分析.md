# hypnus

各个文件作用及如何联系在一起的

底层环境准备:
1. winapis.rs:用于底层api封装,负责在no_std下解析PEB和导出表.动态获取并封装所需要的Native api,避免在导入表IAT中留下静态特征
2. gadget.rs:负责在合法系统模块(如ntdll.dll)代码段中,动态扫描特定的ROP Gadgets(如jmp rcx等).这些gadget作为后续执行无模块内存修改的基础执行单元.?展开一下
3. cfg.rs:负责windows control flow guard安全机制.通过调用内核接口,显示的将后续恢复上下文所需要的NtContinue注册为当前进程的合法间接调用目标,防止在回调时触发异常崩溃
4. config.rs:全局上下文配置,作为单例容器,负责在木马在初始化阶段,集中存储由上述三个模块解析出的函数指针\模块基址\gadget物理地址,供主混淆流程在无依赖状态下调用

执行流hypnus.rs负责执行混淆逻辑
1. 上下文快照:主线程调用RtCaptureContext,将当前cpu的完整寄存器状态(含rip和rsp)保存到一个CONTEXT结构体内存中,作为后续恢复执行的基准状态
2. 构造执行链(ROP chain):利用gadget.rs收集的地址,在内存中动态编排一条返回导向执行链.该执行链依次:修改内存属性为PAGE_READWRITE->加密内存数据->延时->解密内存数据->恢复内存属性为可执行
3. 异步调度与挂起:调用系统底层线程池api(tpalloctimer/tpsettimer),将上述构造好的ROP链的起始地址作为回调函数注册到os的定时器队列中.随后主线程调用ntwaitforsingleobject将自身挂起(阻塞休眠).此时主线程及内存段表现为加密且不可执行的安全状态
4. 系统代工与恢复:定时器到期后,win内核自动唤醒底层线程池工作线程worker thread.工作线程隐式的执行预设的rop链,完成内存解密与属性还原.在rop链末端,指令流引导向ntcontinue函数,并传入第一步保存的context快照.os据此重置所有寄存器,主线程在毫无感知的情况下,自原调用点恢复后续执行.

## hypnus项目作用

 Sliver、Cobalt Strike (CS) 的 Beacon、Meterpreter、Havoc这些顶级的远控载荷（Payload），都可以被塞进 hypnus 的hypnus.rs中struct Hypnus中（base 和size）运行.但
 1. 载荷形态必须是 Shellcode:hypnus 的宏接口（如 timer!(ptr, size, delay)）接收的是一个内存指针（ptr）.这意味着不能直接把 Sliver 生成的 .exe 或 .dll 文件丢给它。你必须在 Sliver服务端生成原生的、位置无关的 Shellcode（Position Independent Code, PIC）
    * 在加载器(即调用 hypnus 的 Rust 程序)中需要:
    * 用 NtAllocateVirtualMemory 申请一块内存
    * 把 Sliver 的 Shellcode 字节流拷贝进去
    * 把这块内存的起始地址（ptr）和大小（size）传给 hypnus
2. 劫持 Sliver 的睡眠（Sleep）: Sliver 执行完毕一次 C2 通信后，它需要等待 60秒再进行下一次心跳（Beaconing） 默认情况下，Sliver 的代码内部会直接调用操作系统的 API：Kernel32!Sleep(60000).如果 Sliver 调用了系统的Sleep，它就会在明文状态下直接挂起！hypnus 的代码根本不会被触发，EDR的内存扫描器（如 Beacon Hunter）过来一扫，瞬间就能在内存里抓到 Sliver的特征码
3. Sliver 的复杂性与 RWX 权限的博弈:ObfMode::Rwx 有什么用，Sliver 就是一个最典型的实战例子
    * sliver(Go语言编译),包含GC,Runtime,Goroutine调度.运行时必然频繁修改所在的内存区域的权限.要跑sliver,hypnus大概率要妥协使用ObfMode::Rwx 参数，哪怕这会增加被 EDR 盯上内存属性异常的风险

以上,hypnus 这种外部免杀框架，天生是为 C/C++ 编写的轻量级 Shellcode（如 Cobalt Strike, Meterpreter, Havoc）准备的.使用sliver有太多的问题需要解决.

hypnus需要最纯粹的、无运行时的、位置无关代码（PIC, Position Independent Code）但  
**绝对不要用 Rust 去从头写 C2Payload（Shellcode），这是在重新发明极为低效的轮子.工作量巨大**  
其需要的c2要
1. 绝对的 PIC（位置无关代码）：不能依赖任何硬编码的绝对地址
2. 零运行时依赖：不能依赖 C 运行库（CRT），也不能依赖 Rust 的标准库（std）
3. 自解析能力：必须通过遍历 PEB（进程环境块）自己找到 ntdll.dll 和kernel32.dll 的基址，然后通过 Hash 自己解析所有的 API 函数地址（就像 dinvk做的那样）

## 适配hypnus的c2载荷

目前红队工业界（包括各大 APT 组织）配合高级加载器（Loader）最完美的Shellcode，全部是用 C 语言或纯汇编编写的，因为它们天生没有复杂的 Runtime：
1. Havoc C2 (Demon 载荷)：目前开源界的最强王者。它的 Demon 载荷是用 C和汇编写的，编译出来的 Shellcode 极其干净，体积小（几十 KB），且支持睡眠混淆的外部 Hook，与 hypnus 是天作之合
2. Cobalt Strike (Beacon) / Brute Ratel (Badger) /Nighthawk：这些商业顶流的核心载荷也都是 C/C++编写的，天生适配任何外部加载器
3. YDHCUI/manjusaka,其核心Implant/Beacon是rust实现的
4. Real-Fruit-Snacks/Kraken
5. MythicAgents/thanatos



