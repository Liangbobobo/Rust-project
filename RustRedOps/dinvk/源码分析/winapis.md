
# winapis

## LoadLibraryA

在 Windows 操作系统中，LoadLibraryA 是 Kernel32.dll 导出的一个非常核心的 API 函数。

它的核心定义：  
它的作用是将一个指定的模块（通常是 .dll 文件，也可以是 .exe）加载到调用进程的虚拟地址空间中。

这里的“A”是什么意思？  
Windows API 通常成对出现：

- LoadLibraryA：接受 ANSI 字符串（传统的单字节字符，如 C 语言的 char*）。
- LoadLibraryW：接受 Wide 字符（Unicode 字符串，如 wchar_t*）。
它们的功能完全一样，只是输入参数的字符编码不同。

它的具体工作流程（当你调用它时）：

1. 查找文件：系统会在硬盘上按照特定的搜索顺序（当前目录 -> 系统目录 -> PATH 环境变量等）寻找你指定名字的 DLL 文件（例如 user32.dll）。
2. 映射内存：如果找到了，系统会将这个 DLL 文件从硬盘“搬运”（映射）到你当前进程的内存空间里。
3. 处理依赖：如果这个 DLL 还需要其他的 DLL 才能运行，系统会递归地把那些 DLL 也加载进来。
4. 初始化：系统会执行该 DLL 的入口函数 DllMain，让 DLL 做一些初始化的准备工作。
5. 返回基址：最重要的一步，它会返回一个 HMODULE（句柄）。  
   - 这个句柄本质上就是该 DLL 在内存中的 起始地址（基地址，Base Address）。

为什么要用它？

- 动态扩展能力：程序不需要在编译时就把所有功能都打包进去。可以通过 LoadLibraryA 在运行时按需加载插件。
- 获取其他 API 的前置条件：如果你想调用某个 DLL 里的函数（比如 MessageBox），你首先必须确保这个 DLL 已经被加载到内存里了，并且你需要拿到它的基地址，才能计算出 MessageBox 在内存里的准确位置。

## NtAllocateVirtualMemory

是ntdll 导出的原始接口，参数细节比 Win32 API (VirtualAlloc) 更加底层  

```rust
pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    BaseAddress: *mut *mut c_void,
    ZeroBits: usize,
    RegionSize: *mut usize,
    AllocationType: u32,
    Protect: u32,
) -> NTSTATUS;
```

- `NtAllocateVirtualMemory` 是 Windows 内存管理的核心 Native API。在 dinvk  
  项目中，它是实现 Shellcode 加载、间接系统调用（Indirect  
  Syscall）和参数欺骗（Argument Spoofing）的关键载体。

  下面我结合 dinvk 的源码，从函数定义、参数细节到实战调用三个维度详细解释。

- **1. 函数定义 (Rust 视角)**

  在 Rust 中，我们要严格匹配 C 语言的 ABI。参考 dinvk 项目中 `src/types.rs` 或  
  `src/winapis.rs` 的定义：

  ```rust
  // 摘自 dinvk/src/types.rs 或类似的类型定义
  use std::ffi::c_void;

  // pub type 类型别名
  // extern "system" 符合目标平台的调用约定(其他形式如 extern "C")
  // fn()->i32 表示是一个函数指针,也是一种符合这种形式的函数的类型
  pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
      ProcessHandle: HANDLE,          // 目标进程句柄
      BaseAddress: *mut *mut c_void,  // [关键] 指向指针的指针 (IN/OUT)
      ZeroBits: usize,                // 零位掩码 (通常为0)
      RegionSize: *mut usize,         // [关键] 指向大小的指针 (IN/OUT)
      AllocationType: u32,            // 分配类型 (Reserve/Commit)
      Protect: u32                    // 内存权限 (如 RWX)
  ) -> i32; // NTSTATUS
  ```

### 双指针

如果函数需要修改你手里的指针变量让它指向别处，你就必须传这个指针变量的地址（双指针）  
双指针,指向的是具体内容的指针的地址,如果修改双指针,修改的是指向具体内容的指针的地址  
双指针就是为了给函数一个“修改权”，让它能把你手里的那个指针重定向到新的地方。

### in out

分别代表传入的值和返回的值

- 我们来把 `NtAllocateVirtualMemory` 的 6 个参数拆解到“原子级”，结合内核原理、Rust 类型以及攻防对抗（Red Team vs EDR）的视角来详细解释。

  这是 dinvk 中定义的函数签名：

  ```rust
  pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
      ProcessHandle: HANDLE,          // 参数 1
      BaseAddress: *mut *mut c_void,  // 参数 2
      ZeroBits: usize,                // 参数 3
      RegionSize: *mut usize,         // 参数 4
      AllocationType: u32,            // 参数 5
      Protect: u32                    // 参数 6
  ) -> i32;
  ```

  ---

- **1. ProcessHandle (目标进程句柄)**  
  - 类型: `HANDLE` (通常是 `isize` 或 `*mut c_void`)  
  - 含义: 你想在谁的脑子里塞东西？  
  - 关键值:  
    - `-1` (即 `0xFFFFFFFFFFFFFFFF`):  
      - 含义: `NtCurrentProcess`，当前进程伪句柄。  
      - 原理: 内核看到 -1，直接操作当前线程所属的进程结构体 (`EPROCESS`)。  
      - 优势: 不需要 `OpenProcess`，速度最快，没有权限检查（永远拥有所有权）。  
    - 其他值 (如 `0x1234`):  
      - 含义: 其他进程的句柄。  
      - 前提: 你必须先调用 `NtOpenProcess` 拿到这个句柄，并且打开时必须请求 `PROCESS_VM_OPERATION` (允许操作虚拟内存) 权限。  
      - 底层: 内核会进行上下文切换 (`KeAttachProcess`)，把页表（CR3 寄存器）切到目标进程，操作完再切回来。这是远程代码注入的基础。

- **2. BaseAddress (基址 - IN/OUT)**  
  - 类型: `*mut *mut c_void` (指向指针的指针)  
  - 含义: 既是“我希望在哪里分配”，也是“实际在哪里分配了”。  
  - 输入 (IN):  
    - `NULL` (指向的变量值为 0):  
      - 系统开启 ASLR (地址空间布局随机化)，随机找个空闲位置。这是最常用、最安全的方式。  
    - 具体地址 (如 `0x00007FF712340000`):  
      - 你强制要求在这个地址分配。  
      - 场景: 恢复被挂起进程的入口点、或者 Process Hollowing（把原本在那里的合法 EXE 代码掏空，换成你的）。  
  - 输出 (OUT):  
    - 内核写入实际分配的基址。  
    - 注意: 哪怕你输入 `0x1001`，内核也会向下对齐到 `0x1000` (4KB边界)。

- **3. ZeroBits (零位掩码)**  
  - 类型: `usize`  
  - 含义: 这是一个为了兼容性存在的参数。它告诉内核：“这个地址的高位，必须有多少个 bit 是 0”。  
  - 作用: 控制分配地址的“高度”。  
  - 计算公式: 有效地址 < `(1 << (64 - ZeroBits))`  
  - 常见值:  
    - `0`: 默认值。如果是 64 位系统，就在 64 位空间随便找（0~128TB 范围）。  
    - `32` (在 64 位系统上): 强制分配在低 4GB 空间（`0xFFFFFFFF` 以下）。  
      - 场景: 当你的 Shellcode 是 32 位的（WoW64 模式），或者你用了一些只支持 32 位指针的古老汇编指令时，必须设这个值。

- **4. RegionSize (区域大小 - IN/OUT)**  
  - 类型: `*mut usize` (指向大小的指针)  
  - 含义: 既是“我想要多大”，也是“实际给了多大”。  
  - 输入 (IN):  
    - Shellcode 的字节数，比如 512 字节。  
  - 输出 (OUT):  
    - 内核总是按页 (Page) 分配。  
    - 在 x64 Windows 上，一页是 4096 字节 (`0x1000`)。  
    - 如果你输入 1，这里会被改写为 4096。  
    - 如果你输入 4097，这里会被改写为 8192 (2页)。

- **5. AllocationType (分配类型)**  
  - 类型: `u32` (位掩码标志位)  
  - 含义: 你想怎么操作这块地皮？是“先圈地”还是“马上盖房”？  
  - 常用标志:  
    - `MEM_COMMIT` (`0x00001000`): [核心]  
      - 分配物理内存（RAM）或页文件。不加这个，你访问内存会直接崩（Access Violation）。  
    - `MEM_RESERVE` (`0x00002000`): [核心]  
      - 在地址空间里“占坑”，防止被别人申请走，但还没给物理内存。  
    - 通常组合: `MEM_COMMIT | MEM_RESERVE` (`0x3000`)。一步到位，既占坑又给内存。  
  - 特殊/黑客标志:  
    - `MEM_TOP_DOWN` (`0x00100000`):  
      - 告诉内核：尽量从高地址往低地址分配。  
      - 对抗: 有些简陋的 EDR/Sandbox 监控可能只盯着低地址区域，用这个有时能绕过监控。

- **6. Protect (内存权限/页保护)**  
  - 类型: `u32`  
  - 含义: 这块内存允许做什么？（读 R、写 W、执行 X）  
  - 攻防焦点: 这是 EDR 报警的重灾区。  
  - 常见值:  
    - `PAGE_NOACCESS` (`0x01`):  
      - 不可访问。通常用于做“警戒页”或者堆喷射中的占位。  
    - `PAGE_READONLY` (`0x02`):  
      - 只读。放字符串常量。  
    - `PAGE_READWRITE` (`0x04`): [安全]  
      - 可读可写。存放数据。EDR 觉得这很正常。  
    - `PAGE_EXECUTE` (`0x10`):  
      - 只执行。极其罕见。  
    - `PAGE_EXECUTE_READ` (`0x20`): [半危险]  
      - 可读可执行。标准的代码段权限（`.text` 段）。  
    - `PAGE_EXECUTE_READWRITE` (`0x40`): [极度危险 - RWX]  
      - 红队: 最爱。因为 Shellcode 往往需要自己解密（需要写权限）然后运行（需要执行权限）。  
      - 蓝队: 绝杀。除了 JIT 编译器（如浏览器 JS 引擎、C# CLR），正常程序极少申请 RWX 内存。一旦发现，直接标黑。

- **总结一张表**

  | 参数           | 方向   | 核心作用     | 典型红队用法                        |
  |----------------|--------|--------------|-------------------------------------|
  | ProcessHandle  | IN     | 确定目标进程 | -1 (自身) 或 远程进程句柄 (注入)    |
  | BaseAddress    | IN/OUT | 确定内存位置 | NULL (随机) 或 指定地址 (Hollowing) |
  | ZeroBits       | IN     | 限制地址高度 | 0 (默认)                            |
  | RegionSize     | IN/OUT | 确定内存大小 | Shellcode 长度 (会被对齐到 4KB)     |
  | AllocationType | IN     | 分配模式     | 0x3000 (`COMMIT \| RESERVE`)        |
  | Protect        | IN     | 权限控制     | RWX (`0x40`) (最敏感，也是最需要的) |
  
## win32 api 和 导出函数(Native api)

win32 api(如 virtualalloc)和ntdll.dll的导出函数NtAllocateVirtualMemory有啥不同,win32 api在系统中是否真实存在?ntdll.dll在系统中什么地方存放着?

- 区别是什么？  
  Windows 的用户模式（User Mode） API 分为不同的层级：

  - Win32 API (Documented/公开):  
    - 代表: `VirtualAlloc`, `CreateFile`, `CreateThread`。  
    - 位置: 位于 `kernel32.dll`, `user32.dll`, `gdi32.dll` 等“子系统 DLL”中。  
    - 作用:  
         它们是微软提供给开发者的官方接口。它们负责参数检查、错误处理（设置  
         `GetLastError`），以及兼容性处理。  
    - 本质: 它们通常不直接干活，而是作为“中间人”，最终调用底层的 Native  
         API。例如，`VirtualAlloc` 内部做完检查后，会调用  
         `NtAllocateVirtualMemory`。

  - Native API (Undocumented/未完全公开):  
    - 代表: `NtAllocateVirtualMemory`, `NtCreateFile`, `NtCreateThreadEx`。  
    - 位置: 位于 `ntdll.dll` 中。  
    - 作用: 它们是用户模式进入内核模式（Kernel Mode）的最后一道大门。  
    - 本质: 它们负责设置系统调用号（SSN/System Service  
         Number），然后执行汇编指令（`syscall` 或 `sysenter`）跳转到 Ring 0  
         内核层。

- Win32 API 是否真实存在？  
  是的，真实存在。  
  物理上，它们存在于 `C:\Windows\System32\kernel32.dll`  
  等文件中。逻辑上，它们是真实存在的导出函数，你可以通过 `GetProcAddress`  
  获取它们的地址并调用。  

  但在操作系统内核的视角里，Win32 API 并不存在，内核只认识 Native API（如  
  `Nt...` 系列）。Win32 API 只是用户层的封装库。

- `ntdll.dll` 存放在哪里？  
  这取决于你的操作系统位数和程序位数（WoW64机制）：

  - 64位系统上的 64位程序:  
    - 路径: `C:\Windows\System32\ntdll.dll` (这是真正的64位核心库)  
  - 64位系统上的 32位程序:  
    - 路径: `C:\Windows\SysWOW64\ntdll.dll` (这是为了兼容32位程序提供的库)  
  - 32位系统:  
    - 路径: `C:\Windows\System32\ntdll.dll`
