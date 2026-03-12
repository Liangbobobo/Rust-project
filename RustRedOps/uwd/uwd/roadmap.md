# uwd

核心目标:  
在执行敏感操作（如 Syscall 或调用Windows API）时，伪造一个看起来完全合法的调用栈（Call Stack），从而欺骗 EDR的栈回溯检查

## 结构

以下是 uwd/uwd 路径下的源码结构深度解析：

1. 核心架构概览

```rust
uwd/src/
├── lib.rs          # 库入口，导出核心模块
├── uwd.rs          # [重点] 核心实现逻辑（约37KB），包含了所有的栈伪造调度逻辑
├── types.rs        # [基础] 底层结构体定义（PE结构、异常处理、上下文等）
├── util.rs         # [辅助] 工具函数（内存查找、地址对齐等）
└── asm/            # [硬核] 底层汇编实现
    ├── msvc/       # MSVC 工具链对应的 .asm 文件
    │   ├── desync.asm     # 去同步模式实现
    │   └── synthetic.asm  # 合成栈帧的核心汇编实现
    └── gnu/        # GNU (MinGW) 工具链对应的汇编实现
```

2. 各模块深度职责

A. uwd.rs (核心逻辑)  
这是整个项目的“指挥部”。它之所以达到 37KB，是因为它需要处理极其复杂的边缘情况。  
* API 封装： 它模仿了类似 dinvk 的调用方式，但内部会先执行“洗栈”操作。  
* 模块搜寻： 它会搜索 Kernel32.dll 或 ntdll.dll 中符合条件的指令片段（Gadgets），用来作为伪造栈的“假起点”。  
* 上下文平衡： 它负责在调用汇编前，精确计算好所有寄存器（RSP, RBP, RIP等）的状态。

B. types.rs (数据结构)  
在 dinvk 中你只需要理解 PE 导出表，但在 uwd 中，你需要面对的是 Windows 异常处理机制的基石：  
* RUNTIME_FUNCTION: 定义了函数在 .pdata 段中的起始和结束地址。  
* UNWIND_INFO: 告诉系统如何撤销一个函数的栈帧（这是 EDR 回溯时的依据）。  
* STACK_FRAME: uwd 自定义的结构，用于描述一个虚假的栈层级。

C. asm/ (汇编指令) —— 这是 uwd 的灵魂  
uwd 不可能只靠 Rust 实现，因为它必须直接操作 RSP 寄存器。  
* synthetic.asm: 这个文件实现了所谓的 "Synthetic Frame"。它会手动在栈上压入特定的返回地址，并执行一个特殊的 JMP 或 RET，使得当 EDR 向上回溯时，看到的都是你预设好的合法 DLL 地址。  
* desync.asm: 这是 joaoviictorti 的高级特性。它利用了“去同步”技术，让执行流和栈回溯流在空间上分离。

3. 与 dinvk 的技术差异（学习重点）

* dinvk 关注点： 查找函数地址 -> 修改内存保护 -> 执行。  
* uwd 关注点： 查找合法模块的 pdata -> 寻找 ROP Gadget -> 伪造 UNWIND_INFO -> 手动操纵栈指针 (RSP) -> 执行 -> 清理并恢复栈指针。

4. 建议开始的步骤

为了不被 37KB 的代码淹没，我建议按照以下顺序进行源码阅读：

1. 先读 types.rs：弄明白它为了描述栈伪造定义了哪些底层结构。  
2. 再看 util.rs：了解它如何从现有 DLL 中提取合法的指令序列。  
3. 核心突破 uwd.rs：找一个具体的入口宏或函数（例如 uwd_call!），追踪它如何一步步走到汇编层的。  
4. 最后研究 asm/：这是最难的一部分，需要你理解 x64 的栈对齐要求（16-byte alignment）