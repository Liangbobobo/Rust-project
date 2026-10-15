- [features](#features)
  - [**Caller-Provided Buffer Pattern（调用者提供外包缓冲区模式）**：](#caller-provided-buffer-pattern调用者提供外包缓冲区模式)
- [buffer.rs是否需要no std](#bufferrs是否需要no-std)
- [关于通信中缓冲区选择的方案](#关于通信中缓冲区选择的方案)
  - [纯栈流式分片](#纯栈流式分片)


Mythic 作为server
Thanatos作为client

# features

## **Caller-Provided Buffer Pattern（调用者提供外包缓冲区模式）**：
**- 将 4KB 缓冲区的控制权上移至调用者**，底层纯计算叶子函数（不再调用其他函数，极致的叶子函数也不产生局部变量需要的栈空间）  
**- 内存解耦**：调用者可自由决定将 4KB 缓冲区放置于当前栈帧、静态数据段（`.data`/`.bss`）、专用私有堆（Private Heap）或会话状态机中。

传统的代码写法中，底层计算函数通常会自己决定内存从哪来：

- **硬编码堆分配**：`fn process() -> Vec<u8>`（强制调用者使用默认进程堆 `malloc`）
- **硬编码栈分配**：`fn process() { let mut buf = [0u8; 4096]; ... }`（强制在当前函数的栈顶压入 4KB）

而 `buffer.rs` 中，让所有核心函数（如 `encode_to_slice`、`generate_padding_slice`）仅接收 `&mut [u8]`。在 Rust 底层，`&mut [u8]` 只是一个 16 字节的“胖指针”（8 字节首地址指针 `*mut u8` + 8 字节长度 `len`）。底层叶子函数根本不需要、也无法知道这块 4KB 内存究竟物理存在于哪里。这就把“内存放在哪”的决定权，100% 交给了上层调用者（Caller）。

上层调用者可以根据不同的执行场景，自由将这 4KB 塞入不同的内存区域：

```text
 ┌──> 1. 当前栈帧 (Stack Frame)        ──>
  常规健康线程、零分配即用即走
                    │
                    ├──> 2. 静态数据段 (.data / .bss)     ──>
  线程劫持/浅栈注入，彻底不碰栈
    &mut [u8] (4KB) ┼
                    ├──> 3. 专用私有堆 (Private Heap)      ──>
  避开系统默认堆，防御 ETW-Ti 遥测
                    │
                    └──> 4. 会话状态机 (Session Struct)   ──>
  跨心跳周期持久复用，杜绝抖动
```

**1. 放置于当前栈帧（Stack Frame）**

- **代码形态**：
  ```rust
  let mut temp_buf = [0u8; 4096];
  encode_to_slice(data, &mut temp_buf);
  ```
- **适用工况**：主线程或常规工作线程，栈空间充裕（通常有 1MB 预留空间）。
- **优势**：生命周期随当前作用域结束立即自然销毁，无需任何内存回收，速度极快。

**2. 放置于静态数据段（.data / .bss）**

- **代码形态**：
  ```rust
  static mut GLOBAL_WORKSPACE: [u8; 4096] = [0u8; 4096];
  encode_to_slice(data, unsafe { &mut GLOBAL_WORKSPACE });
  ```
- **适用工况**：线程劫持（Thread Hijack）、早期的 APC 注入或栈伪造（Stack Spoofing）场景。
- **攻防价值**：
  - 在这些场景下，注入的目标线程当前剩余可用栈可能极其浅薄（可能只剩几百字节）；
  - 如果函数敢在栈上压入 4KB，会瞬间触碰 PAGE_GUARD 保护页或直接触发 STATUS_STACK_OVERFLOW 崩溃；
  - 调用者将 4KB 放在静态段中，传切片进去，当前栈帧开销仅仅是 16 字节的指针，彻底挽救浅栈崩溃。

**3. 放置于专用私有堆（Private Heap）**

- **代码形态**：
  ```rust
  // 调用 Win32 原生 HeapCreate 创建一个孤立私有堆，完全避开 GetProcessHeap()
  let private_heap = HeapCreate(HEAP_NO_SERIALIZE, 0, 0);
  let ptr = HeapAlloc(private_heap, 0, 4096) as *mut u8;
  let slice = core::slice::from_raw_parts_mut(ptr, 4096);
  encode_to_slice(data, slice);
  ```
- **适用工况**：对抗严格的 Windows 10/11 内存遥测（如 EDR 开启了基于 ETW-Ti 的堆分配溯源监控）。
- **攻防价值**：
  - 现代 EDR 会重点监控进程的“默认堆”（Default Heap）；
  - 私有堆在内存中与常规程序堆完全隔离，甚至可以随时通过 HeapDestroy 整体物理抹除，实现真正的防取证残留。

**但用户态的vad无法隐藏,即VAD 存在于 Ring 0 内核空间，Ring 3 完全不可写.导致该方案在用户态无法落地.**

**4. 放置于会话状态机（Session State Machine）**

`engine.rs:486-517` 正是严格贯彻了这一模式：

```rust
// src/engine.rs 中的主循环设计：
pub fn run(&mut self) {
    // 1. 在循环外部一次性划定工作区，固定栈水位线
    let mut workspace = ActiveStackWorkspace::new();
    let mut sysinfo_ws = SysInfoWorkspace::new();
    let mut exec_ws = TaskExecutionWorkspace::new();

    // 2. 进入常态心跳与任务调度主循环
    while !self.should_exit {
        // 每次轮询仅向下借用可变引用 &mut workspace，内存零变动
        self.build_get_tasking_payload(..., &mut workspace);

        // 网络收发...

        // 执行完原地刷零，工作面槽位不动，静待下一次心跳复用
        workspace.zeroize();

        // 3. 动态休眠
        std::thread::sleep(...);
    }
}
```

- **适用工况**：C2 长期运行的心跳轮询主循环。
- **优势**：
  - 不需要每次心跳都重新申请/压栈 4KB，整个 Agent 进程运行生命周期中只固定驻留这一处内存；
  - 避免了内存反复创建释放造成的微观特征抖动。

---


> “底层算法只负责干活（操作切片），不拥有内存；上层调用者根据当前的宿主特权、栈深安全度和对抗环境，爱把这 4KB 放在哪就放在哪。”
> 这既保证了上层架构极高的灵活性，又让底层的叶子计算模块保持了纯粹、安全与极致性能。














# buffer.rs是否需要no std
不需要,其内部基本上都是core的函数,其定义的数据结构都是纯栈分配
即使整个 Agent 是在常规环境（std 可用）下编译，buffer.rs
      在执行时也绝对不会调用系统的堆分配器（如 RtlAllocateHeap）



# 关于通信中缓冲区选择的方案

1. 能否使用(https://github.com/joaoviictorti/RustRedOps/tree/main/Module-Stomping)或https://github.com/joaoviictorti/RustRedOps/tree/main/Module-Stomping 的技术 将大文件加载到一个dll的.text节,然后再用私有堆传输,这样不就去掉了vad的特征了吗

2. 带缓冲区的异步私有堆流水线（生产者-消费者模型） 这种技术还是有vad的残留啊,还是能被edr发现的把.主要是用户态的vad无法隐藏,即VAD 存在于 Ring 0 内核空间，Ring 3 完全不可写.导致该方案在用户态无法落地.

3. ：HTTP Pipeline / 长连接（Keep-Alive）复用 ,这种方案thanatos或者说mythic是否支持呢.支持


只要私有堆做到**“按需短生命周期创建 + 传输完立即 HeapDestroy整体销毁 + 休眠期 PAGE_NOACCESS掩码保护”，就能完美平衡大文件高吞吐传输效率与VAD/内存抗扫描隐蔽性**！

纯栈流式切片,会引入异步问题,复杂度急剧上升

1. 函数使用大量的栈空间,这本身就是一种特征吧,很容易被edr发现吧?



## 纯栈流式分片

**先用纯栈流式分片的方式存活下来,然后指令关闭对应的edr,在使用堆传输文件**(该方案尚未论证)


1. 栈上的临时缓冲区会随着函数退栈而销毁,因此如果使用异步的方式,跨心跳异步阐述,会将复杂度指数级暴增.因此,需要在单个同步函数栈帧内,通过循环一次性把大文件的分片完全读取然后发送

2. 控制状态轻量化存留 + 数据缓冲区即用即销:控制状态（极小，仅几十字节）这些轻量元数据保存在 Agent 的 Task 状态列表中;数据缓冲区（大头，256KB）：绝留在栈上,每次心跳触发时，临时在栈上分配 256KB，去磁盘 Seek(seek_offset) 读取 1片、加密发走、物理刷零，然后立刻退栈

3. 如果要传输 几百兆甚至上 GB 的超级大文件，且要求必须在独立后台线程中传输.此时不能再考虑纯栈了,需要采用独立私有堆（Private Heap）+ 独立 Worker线程.之后传输完毕立即销毁堆,去掉vad的特征.这种路线是可接受的,对于任何GB级文件传输,网络流量本身就已经产生长期连接,追求零vad的边际效益在降低,只要私有堆在传输完成后被heapdestory,在接下来长期c2休眠期内vad树依然会恢复干净.

**在函数内部划拨几百 KB 的大栈帧，究竟会不会成为被 EDR 抓包的显著特征**

在 Windows 底层机制和 EDR的探针设计中，“栈空间的大小”本身几乎无法被 EDR动态监控或告警；但如果忽略了“栈探测（__chkstk）”和“挂钩函数调用（Hooked API Call）”，会导致系统崩溃或被堆栈回溯（Stack Walk）捕获

**为什么“大栈帧”本身不会被 EDR 标记为异常特征**

1. 内核视角的“栈”：只是一块普通的连续内存页:在 Windows 内核（Memory Manager）看来，每个线程创建时，内核就已经为它在 VAD树中注册了一个标准的 Thread Stack 内存区域（64 位 Windows 下默认预留 1MB虚拟地址空间，即 SizeOfStackReserve = 1MB）
• 堆分配（Heap/VirtualAlloc）：需要频繁向内核发起 API
  请求（NtAllocateVirtualMemory），内核会主动向 EDR 推送 ETW-Ti 遥测事件。
  • 栈划拨（Stack Allocation）：只是 CPU 的 rsp 指针在已预留的 1MB
  空间内上下移动（例如 sub rsp, 0x40000）。这个过程完全发生在 CPU 寄存器和
  MMU 硬件层面，根本不会产生任何系统调用（Syscall），也不会触发任何内核 ETW-
  Ti 事件。 EDR 根本接收不到“某某函数申请了 256KB 栈”的通知。


2. 2. EDR 无法对“栈帧尺寸”设置常规 Hook

  EDR（如 CrowdStrike、Defender）在用户态主要通过在 ntdll.dll / kernel32.dll
  的关键 API 入口插入 jmp 钩子（Inline Hook）来收集遥测。
  • 没有哪个合法 API 叫 AllocateStackFrame()。
  • EDR 无法在汇编指令层面去监控每一个函数的 sub rsp, N 指令（这会导致 CPU
  性能下降成百上千倍，系统直接瘫痪）。因此，单纯的栈空间变大，在 EDR
  看来是完全无感知的。

栈缓冲区控制在64kb兼顾了效率和隐蔽性,64kb的栈尺寸在任何win进程都是普遍的




