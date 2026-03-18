# types

## struct Config

```rust
/// Configuration structure passed to the spoof ASM routine.
#[repr(C)]
#[derive(Debug)]
pub struct Config {
    /// Address RtlUserThreadStart
    pub rtl_user_addr: *const c_void,

    /// Stack Size RtlUserThreadStart
    pub rtl_user_thread_size: u64,

    /// Address BaseThreadInitThunk
    pub base_thread_addr: *const c_void,

    /// Stack Size BaseThreadInitThunk
    pub base_thread_size: u64,

    /// First (fake) return address frame
    pub first_frame_fp: *const c_void,

    /// Second (ROP) return address frame
    pub second_frame_fp: *const c_void,

    /// Gadget: `jmp [rbx]`
    pub jmp_rbx_gadget: *const c_void,

    /// Gadget: `add rsp, X; ret`
    pub add_rsp_gadget: *const c_void,

    /// Stack size of first spoofed frame
    pub first_frame_size: u64,

    /// Stack size of second spoofed frame
    pub second_frame_size: u64,

    /// Stack frame size where the `jmp [rbx]` gadget resides
    pub jmp_rbx_frame_size: u64,

    /// Stack frame size where the `add rsp, X` gadget resides
    pub add_rsp_frame_size: u64,

    /// Offset on the stack where `rbp` is pushed
    pub rbp_stack_offset: u64,

    /// The function to be spoofed / called
    pub spoof_function: *const c_void,

    /// Return address (used as stack-resume point after call)
    pub return_address: *const c_void,

    /// Checks if the target is a syscall
    pub is_syscall: u32,

    /// System Service Number (SSN)
    pub ssn: u32,

    /// Arguments that will be passed to the function that will be spoofed
    pub number_args: u64,
    pub arg01: *const c_void,
    pub arg02: *const c_void,
    pub arg03: *const c_void,
    pub arg04: *const c_void,
    pub arg05: *const c_void,
    pub arg06: *const c_void,
    pub arg07: *const c_void,
    pub arg08: *const c_void,
    pub arg09: *const c_void,
    pub arg10: *const c_void,
    pub arg11: *const c_void,
}

impl Default for Config {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
```

### uwd使用Config伪造栈帧的逻辑

EDR会追溯调用栈,如果函数的调用路径来起来像是系统dll函数的路径,就会骗过EDR的栈回溯

下面将Config各个字段填入uwd栈伪造的逻辑链条中

1. 构建合法根基,第一步确保调用栈的终点伪造成合法的win的线程起点.为什么调用栈的终点是win线程的起点?
* rtl_user_addr：提供 ntdll!RtlUserThreadStart 的 VA
* rtl_user_thread_size：提供该函数在栈上占据的字节数
* base_thread_addr：提供 kernel32!BaseThreadInitThunk 的 VA
* base_thread_size：提供该函数在栈上占据的字节数
* 汇编代码执行时,首先会在高地址处手动写入这两个地址,模拟线程的生成轨迹.size相关字段用于精确移动rsp指针,确保后续伪造的栈帧能与这两个根基帧完美对齐

2. 伪造链路,第二步插入1-2个来自合法dll(如kernelbase.dll)的函数帧,作为肉盾
* first_frame_fp / second_frame_fp：作为“假返回地址”填入栈中
* first_frame_size / second_frame_size：告知汇编每个假帧的大小
* rbp_stack_offset：如果肉盾函数使用了 RBP 帧指针，此字段指明 RBP应存放在该帧内的精确位置
* 汇编根据 size 移动 RSP，并在该位置写入 fp。当 EDR回溯时，每一层返回地址都指向合法的 DLL 内部。rbp_stack_offset 保证了即使EDR 检查 RBP 链，也不会发现异常（防止栈溢出或不一致检测）

3. 跳转跳板,为了隐藏从“肉盾”到“目标”的切换动作，不能直接使用 CALL，必须使用 ROPGadgets 这种“非典型”方式跳转
* jmp_rbx_gadget：指向一个包含 jmp rbx 指令的合法 DLL 地址
* jmp_rbx_frame_size：该指令所在函数的栈大小
* add_rsp_gadget：指向一个包含 add rsp, X; ret 指令的合法 DLL 地址
* add_rsp_frame_size：该指令所在函数的栈大小
* jmp_rbx 实现了“无返回地址”的间接跳转。add_rsp用于在目标函数返回后，清理掉栈上预设的垃圾数据或影子空间（Shadow Space），保证执行流能顺利“滑”回下一层伪造帧

4. 载荷与参数对齐,将真实的函数参数喂给目标函数
* spoof_function：真正的目标 API（如 VirtualAlloc）或 syscall 指令地址
* number_args：本次调用的参数个数
* arg01 - arg11：预存的 11 个 64 位参数
*  汇编代码读取 number_args
   *  将 arg01-04 分别加载到 RCX, RDX, R8, R9 寄存器
   *   如果有更多参数，按照 Config 中的顺序，将其压入当前 RSP之上的预留槽位中
   *   最后 jmp 到 spoof_function
  
5. 执行恢复与清理,目标函数执行完毕执行 RET 后，CPU 需要一个安全的“降落点”
* return_address：指向 uwd 内部的一个汇编指令位置（通常在 uwd.rs的入口之后）
* is_syscall：告知汇编这是否是一个系统调用
* ssn：如果是系统调用，提供对应的系统服务号（SSN）
* 目标函数 RET 时，会跳到 return_address。汇编代码随后根据is_syscall 判断是否需要清理寄存器或执行 SYSCALL






### Config字段

1. `#[repr(C)]`,保证rust中定义的结构体的字段顺序和内部布局与c/汇编完全一致,否则汇编会读到错误数据
* 作用:把rust搜集到的所有va/rva/size等通过一个指针传给汇编函数SpoofSynthetic

2. 线程踪迹相关字段,所有合法win线程必须以ntdll!RtlUserThreadStart和kernel32!BaseThreadInitThunk这两个函数为起点(为什么?),EDR溯源时,如果找不到这两个函数,就会发现异常
* rtl_user_addr:*const c_void,指向ntdll!RtUserThreadStart地址
* base_thread_addr :*const c_void,指向 kernel32!BaseThreadInitThunk
* rtl_user_thread_size (u64)/ base_thread_size (u64)
  * 上述两个函数在栈上占用的字节数
  * 汇编需要知道大小,以便在伪造栈时准确的扣除/跳过对应的空间,确保RSP最终的对齐

3. 伪造栈帧相关字段,uwd从kernelbase.dll或其他dll中找到的合法的函数外壳
* first_frame_fp / second_frame_fp (*const c_void):
  * 肉盾函数内部的地址,通常是函数序言之后
  * 作用:EDR回溯栈时,它会看到RIP指向这里,从而认为这个调用是合法的
  * first_frame_size / second_frame_size (u64)
    * 之前代码通过解析UNWIND_CODE计算的该肉盾函数的总栈大小
    * x64下,栈操作都是8字节对齐的,u64节能匹配寄存器宽度,又能防止溢出

4. 跳板和工具相关字段,用于在不同栈帧之间切换的汇编gadgets
* jmp_rbx_gadget / add_rsp_gadget (*const c_void)
  * 原型是指向dll中jmp rbx和add rsp,x;ret指令的物理地址
* jmp_rbx_frame_size / add_rsp_frame_size (u64)
  *  作用：为了保证 RET 时栈是对齐的，汇编必须知道这些 Gadget所在的函数本身占了多少栈空间
*  rbp_stack_offset (u64)
   *  关键点：如果肉盾函数使用了 RBP 帧指针，这个字段告诉汇编：“请把假的 RBP存在离栈顶多少字节的地方”。这是为了瞒过 RtlVirtualUnwind

5. 执行载荷相关字段
* spoof_function (*const c_void): 你最终要调用的那个 API（如VirtualAlloc）的真实地址
* return_address (*const c_void),当你的恶意操作执行完后，代码应该跳回哪里继续运行。通常是 uwd内部的一个恢复点
* is_syscall (u32): 布尔标记。如果是 1，代表我们要执行系统调用
* ssn (u32): 如果是系统调用，这里存的是系统调用号（SSN）。使用 u32 是因为Windows 系统调用号永远不会超过 32 位

6. 参数传递相关字段
* number_args (u64): 告诉汇编这次传了几个参数
* arg01 到 arg11 (*const c_void),全部强制转换为指针类型,汇编代码会遍历这些槽位，前 4 个进寄存器，剩下的压栈

7. core::mem::zeroed(),把这块内存全部抹为0,全0代表一个指针是NULL,是干净的初始状态.意味着在__private::spoof中,只需要给非0字段赋值,剩下的默认就是安全的空值