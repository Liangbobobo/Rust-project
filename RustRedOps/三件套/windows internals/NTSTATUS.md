

## win中的ntstatus

win中,NTSTATUS不仅仅是一个简单的错误码,它是windows内核Ntoskrnl.exe与用户态ntdll.dll之间通信的最高标准协议

### 物理本质

在windows sdk中
```c
typedef LONG NTSTATUS;  // 即 32 位有符号整数 (Rust 中的 i32)
```
该32位整数并非随意分配,它是一个高度结构化的位域bitfield.微软将其划分为4部分

```text
  31 30 29 28  27                    16  15                             0
 +-----+--+--+-------------------------+---------------------------------+
 | Sev |C |R |     Facility            |               Code              |
 +-----+--+--+-------------------------+---------------------------------+
```


字段详细说明

- **Severity (Sev) [位 31-30]**
    - **描述**：严重级别（最核心的两位）。
    - **数值含义**：
        - `00` (0x0)：**Success（成功）**。操作完全成功。
        - `01` (0x4)：**Informational（信息）**。操作成功，但有一些额外信息（比如缓冲区刚好够用）。
        - `10` (0x8)：**Warning（警告）**。操作部分成功，或者存在潜在问题。
        - `11` (0xC)：**Error（错误）**。操作彻底失败。
    - > **注意**：由于有符号整数的最高位是符号位，只要 Severity 是 **Error** (`11`) 或 **Warning** (`10`)，这个 32 位整数必定是负数。这就是为什么底层代码经常用 `status < 0` 来判断是否失败。

- **Customer (C) [位 29]**
    - **描述**：客制化标志。
    - **数值含义**：
        - `0`：微软官方定义的系统状态码。
        - `1`：第三方驱动或客制化程序定义的状态码。

- **Reserved (R) [位 28]**
    - **描述**：保留位。
    - **数值含义**：通常为 `0`。

- **Facility [位 27-16]**
    - **描述**：设备/组件代码。
    - **含义**：指示是哪个 Windows 内核组件抛出的这个状态。
    - **示例**：
        - `1`：代表 RPC
        - `2`：代表 Dispatch 调度器
        - `17`：代表 DCOM 等

- **Code [位 15-0]**
    - **描述**：具体状态码。
    - **含义**：具体的错误或状态编号。



### NTSTATUS 与 Win32 Error (GetLastError) 的降维关系

高级语言（如 Python、常规 C++）时很少见到 NTSTATUS,因为 Windows 有两套 API 体系：
1. Native API (Ntdll.dll / 也就是你框架调用的层)：如NtAllocateVirtualMemory，它们直接通过 syscall进内核，返回的永远是原汁原味的 NTSTATUS
2. Win32 API (Kernel32.dll 等)：如 VirtualAlloc。它是给普通开发者用的
3. 当调用 VirtualAlloc 时，它内部其实调用了 NtAllocateVirtualMemory。如果内核返回了一个 NTSTATUS 错误（比如 0xC0000005），Kernel32.dll会调用一个内部函数 RtlNtStatusToDosError，把它翻译成一个简单的 Win32错误码（比如 ERROR_NOACCESS 即998），然后存放在当前线程环境块（TEB）中。开发者通过 GetLastError()拿到的就是这个 998

### 特性-NT_SUCCESS 宏：0 不代表唯一的成功

if (status == 0) 就是成功，其他都是失败。这在 Windows 内核中是大错特错的

有些系统调用成功了，但返回值不是 0 (STATUS_SUCCESS)，而是STATUS_IMAGE_NOT_AT_BASE (0x40000003) 等带有 Warning 或 Informational级别的值。如果你只判断 == 0，就会误杀正常逻辑.为此,，Windows SDK 提供了一个极其著名的宏
```c #define NT_SUCCESS(Status) (((NTSTATUS)(Status)) >= 0)```

这段c宏需要理解后翻译为rust.其表达的是如果ntstatus在rust中设为i32,可以用>=0表示内核认为的成功


任何大于等于 0 的 NTSTATUS（即最高位不是 1，Severity 是 00 或01），在内核眼中都是成功的！ 在您的 Rust 框架中，判断 OS API是否执行成功，也应该严格遵循这个逻辑(rust三件套中并没有实现这种判断)
1. **0xC0000005 (STATUS_ACCESS_VIOLATION)**
    * 本质：访问违例。
    * 红队场景：这是最常见的崩溃码。说明你的 Exploit 算错偏移了，或者 Shellcode 试图执行没有 PAGE_EXECUTE 权限的内存，触发了 DEP（数据执行保护）。
2. **0xC0000022 (STATUS_ACCESS_DENIED)**
    * 本质：权限被拒绝。
    * 红队场景：当使用 NtOpenProcess 尝试获取 lsass.exe 句柄时返回此码，90% 的概率是你的请求被 EDR 的内核回调（Callback，如 ObRegisterCallbacks）无情拦截了。
3. **0xC0000353 (STATUS_PORT_DISCONNECTED) / 0xC0000120 (STATUS_CANCELLED)**
    * 本质：端口断开 / 操作取消。
    * 红队场景：在进行某些涉及 RPC 或 ALPC 的高级利用（如通过特定的内核对象进行利用）时，如果防病毒软件通过微过滤驱动（Minifilter）强行掐断了通信，常常会返回这些非典型的网络/总线状态码。
4. **0x80000003 (STATUS_BREAKPOINT)**
    * 本质：触发了断点（Warning 级别，非 Error）。
    * 红队场景：你的代码执行到了 int 3 (0xCC)。这常常用于反调试（Anti-Debug）检测，或者 EDR 故意在你的内存中插入了断点来捕获执行流。
5. **0xC0000409 (STATUS_INVALID_CRUNTIME_PARAMETER)**
    * 本质：无效的 C 运行时参数。
    * 红队场景：在现代 Windows 10/11 中，如果触发了栈缓冲区溢出（Stack Buffer Overrun），且被 GS（/GS 编译选项）或 CFG（控制流保护）拦截，进程经常会以这种异常状态光速被系统内核击杀。




> 因为 NTSTATUS 承载了丰富/多维度的内核语境,必须使用 OsError(NTSTATUS) 包裹来区分.比如一次Syscall失败时，如果它返回了0xC0000022（拒绝访问），这意味着“代码写对了，但是环境（EDR）拦截了你”
> 如果您的框架“自作聪明”地把它映射成了 anyhow::bail!("Syscall failed")，就永远失去了知道“是被拦截了”还是“指针写错了 (0xC0000005)”的机会



















## 使用ntstatus场景(三件套为例)

这种场景下,工具代表os发声,或原样传达操作系统的声音

1. 作为transparent proxy透明代理:宏或函数拿到函数地址/ssn,并将参数传递给内核/底层api,api执行完毕并返回一个结果
    * 原封不动返回该ntstatus
```rust
// 底层 NtAllocateVirtualMemory 返回什么，宏就返回什么。
// 调用者拿到这个状态码，完全可以使用微软官方的文档去解析它。
 let status: NTSTATUS = syscall("NtAllocateVirtualMemory", ...);
if status != STATUS_SUCCESS { ... }
```

2. 写道原生错误上下文时:框架执行一个复杂任务(比如注入shellcode),其中某一步系统调用失败导致整个任务失败.需要告诉调用者任务失败,并且附带原因.此时,把ntstatus作为数据包裹在框架自定义的enum中
```rust
pub enum HypnusError {

// 明确告诉调用者这是系统调用导致的失败，并附上真实状态码
SyscallFailed(NTSTATUS),
   }
// 返回 Err(HypnusError::SyscallFailed(0xC0000005))
```

3. API Hooking/模拟操作系统行为:写一个Inline Hook或基于异常的Hook,拦截进程内部正常的NtQuerySystemInformation 调用（例如为了隐藏某个进程）.此时框架扮演操作系统角色,必须返回一个符合该api定义的ntstatus
```rust
// 拦截后，向调用者伪造一个系统级别的“权限拒绝”或“成功”
fn hooked_NtQuerySystemInformation(...) -> NTSTATUS {
  if is_hidden_process {
  return 0xC0000022; // STATUS_ACCESS_DENIED
            }
            // ...
        }
```

## 不能使用ntstatus的场景

错误完全由框架自身机制\逻辑\配置引发,操作系统内核没有参与,此时强行借用ntstatus会导致灾难性语义混淆/重叠

1. api解析与定位失败resolution failures:如遍历pe导出表时没有找到匹配的hash或者模块根本没有加载.
    * 错误做法：返回 STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139).为什么不行：调用者会以为是系统的 Loader（如LdrpGetProcedureAddress）抛出的错误。但实际上操作系统压根没执行，是你写的 Rust 遍历代码没找到
    * 正确做法：返回 None，或者 Result::Err(FrameworkError::HashNotFound)
这里能否返回自定义的enum中的错误

2.  前置参数校验与生命周期错误（Validation Errors）:调用宏时，参数超过了 11 个（比如 uwd 的 spoof 限制）；或者尚未初始化分配器就去调用堆操作
    * 返回 STATUS_INVALID_PARAMETER (0xC000000D)
    * 为什么不行：STATUS_INVALID_PARAMETER 是内核 API在校验指针是否对齐、句柄是否有效时给出的。如果框架层强行返回这个，开发者会疯狂检查自己传给 API 的指针，而实际上是框架的宏校验拦截了它
    * 正确做法：编译期报错（最佳），或者运行时返回 Result::Err(FrameworkError::TooManyArguments)

3. 环境特征不满足框架需求（Environment Mismatches）:hypnus 在尝试进行栈欺骗（Call Stack Spoofing）时，在 kernelbase.dll中找不到合适的 jmp rbx Gadget，或者 Unwind Table (pdata) 结构无法解析
    * 错误做法：返回 STATUS_NOT_FOUND (0xC0000225) 或 STATUS_UNSUCCESSFUL
    * 为什么不行：这完全是“红队免杀技术”特有的失败场景。操作系统没有任何一个标准 错误码能描述“找不到用于 ROP 的汇编片段”
    * 正确做法：返回 Result::Err(HypnusError::GadgetNotFound)

> 黄金法则（Rule of Thumb）：如果这段代码没有通过 syscall指令进入内核，也没有执行目标操作系统的真实函数，那么它就没有任何资格生成一个独立的 NTSTATUS 给外部调用者。它只能返回 Option、bool 或框架自定义的枚举


## 最佳实践

将框架自身错误/os原生错误收敛到同一个统一的rust enum中,配合类似bail!的自定义宏实现opsec免杀隐蔽和工程抽象

1. 定义统一的错误枚举: 
```rust
pub type NTSTATUS = i32;

// Clone Copy共同保证在栈/寄存器上的急速拷贝传值
#[derive(Clone, Copy, PartialEq, Eq)]
// 保持内存布局稳定和对应
#[repr(C)]
pub enum RedError {
    // 错误类型1:框架域错误 framework errors,不能在这里借用任何ntstatus
    ModuleNotFound,         // 模块未加载
    ApiNotFound,            // 导出表哈希未匹配
    GadgetNotFound,         // ROP 链 / 栈欺骗所需指令片段未找到
    InvalidArguments,       // 宏参数越界等
    StackAllocationFailed,  // 模拟堆栈分配失败

    // 错误类型2:os errors:将os原生ntstatus封装,代表框架执行了以此系统调用,但系统拒绝/失败
    OsError(NTSTATUS),


    // 定义统一的 Result 类型
    pub type Result<T> = core::result::Result<T, RedError>;

```

2. 结合debug_log!自定义实现控制流的宏:模拟anyhow::bail!的控制流(提前return),但在release模式下彻底抹除字符串.实现零体积,零ioc
```rust
#[macro_export]
macro_rules! stealth_bail {
// 模式 1：带错误信息（模拟 anyhow::bail!(err, "msg {}", var)）
($err:expr, $($arg:tt)*) => {
    {
    // debug_log! 在 Debug 模式下会调用 OutputDebugStringA
    // 在 Release 模式下，整个 debug_log! 会被编译器优化为空 ()
   $crate::debug_log!($($arg)*);
   
   // 提前返回枚举错误
    return core::result::Result::Err($err);
}
// 模式 2：极简模式，只返回错误类型不带日志
($err:expr) => {
    return core::result::Result::Err($err);
  };
};
```

3. 使用示例:可在一个同时有框架/os操作的函数中实现错误控制
```rust
use crate::error::{RedError, Result};
use crate::macros::stealth_bail; // 假设宏已导出

// 一个同时包含“框架操作”和“OS 操作”的业务函数

pub fn inject_and_execute(args: &[*const c_void]) -> Result<()> {

// 【场景 1：框架域错误】 —— 前置校验
if args.len() > 11 {
// Debug 下打印日志，Release 下静默提前返回 RedError::InvalidArguments
stealth_bail!(RedError::InvalidArguments, "Too many args ({} >11), spoof failed!", args.len());
}

// 【场景 1：框架域错误】 —— 解析 API
// 利用 Option::ok_or 丝滑地将 Option 转换为统一的 Result，并使用 ?提前返回
let p_alloc = get_proc_address(ntdll,hash).ok_or(RedError::ApiNotFound)?;

// 【场景 1：框架域错误】 —— 免杀逻辑（找 Gadget）
let gadget = find_gadget().ok_or(RedError::GadgetNotFound)?;

//===============================================

// 【场景 2：OS 域错误】 —— 真实系统调用
let status = unsafe {
// 假设这里执行了刚才找到的 API (伪代码)
call_api(p_alloc, gadget, ...)
};

// 判断系统调用是否成功 (假设 0 是 STATUS_SUCCESS)
if status != 0 {
// 将原生的状态码包裹进 OsError 变体中！
stealth_bail!(
RedError::OsError(status),
"[!] Native API Failed. NTSTATUS: {:#X}",status
);
}

Ok(()) 
}
```

外部调用者使用match处理结果,逻辑清晰
```rust
match inject_and_execute(args) {
 Ok(_) => println!("Injection success!"),
 Err(RedError::ApiNotFound) => println!("框架没找到函数"),
 Err(RedError::OsError(0xC0000005)) => println!("系统抛出了访问拒绝(Access Violation)！"),
  _ => (),
}
```

1. 准确知道是谁（框架还是 OS）在哪个阶段引发了错误
2. 零开销抽象（Zero-Cost Abstraction）:一个RedError 枚举在内存中:一个标志位（用于区分是哪种错误）+ 最大的数据体（i32 的 NTSTATUS）。在 64位系统下，它通常只需要 4 到 8 个字节。在执行 return Err(...)时，这几个字节会直接存放在 CPU 寄存器（如RAX）中返回，不涉及任何堆内存分配（No Heap Allocation）
3. 极致的免杀（OPSEC）：运行 cargo build --release 时，因为 debug_log! 宏配置了 `#[cfg(debug_assertions)]`，编译器在做 AST展开时，会直接把所有带有中英文字符串的 `$crate::debug_log!($($arg)*);` 删掉最终编译出的DLL/EXE，只有干净利落的寄存器跳转，完全消除了安全分析人员最喜欢抓的字符串特征（IoC）




