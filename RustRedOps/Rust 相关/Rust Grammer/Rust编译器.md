为了生成一个标准的 Windows 可执行文件，Rust 的编译链路分为三步：
1. 前端解析 (rustc)：解析 Rust 源码，进行类型检查，将其转换为中间代码（LLVMIR）
2. 后端代码生成 (LLVM)：Rust 使用 LLVM 作为后端（与 Clang编译器相同）。LLVM 负责将中间代码转换为 x64 汇编和机器码（即生成 .obj目标文件）
3. 链接阶段 (MSVC link.exe)：在 Windows 上，默认的 Rust 编译目标（Target）是 x86_64-pc-windows-msvc。
    * 在这个目标下，rustc 在生成 .obj 文件后，必须调用微软官方的链接器link.exe（来自 MSVC / VS Build Tools），将这些目标文件链接成最终的 .exe或 .dll


| 编译目标 (Target) | 异常处理机制 (SEH) | 链接器 (Linker) | 特点 |
| :--- | :--- | :--- | :--- |
| x86_64-pc-windows-msvc(默认，强烈推荐) | 原生 Windows SEH | 微软官方 link.exe | 产生的 .pdata 表与 C/C++ 完全一致，与操作系统和 EDR 兼容性最好。 |
| x86_64-pc-windows-gnu | MinGW DWARF / SEH | GNU ld.exe | 使用 GCC/MinGW 链，不需要安装 Visual Studio，但体积和兼容性略逊 |

**在红队免杀和win底层开发时,必须强制使用msvc链,这能保证二进制程序在os和edr眼里,和官方c++程序没有区别**

在Rust中,如果发生panic!,rust会默认进行栈展开stack unwinding,来依次执行析构函数(Drop).当处于msvc工具链下,rust编译器(LLVM后端):
1. 自动写入.pdata节:对每个rust函数,llvm会自动生成对应的IMAGE_RUNTIME_FUNCTION 表项。
2. 自动追加 UNWIND_INFO_0：LLVM 会自动将 UNWIND_INFO_0 的 ExceptionHandler字段指向 Rust 专用的异常处理器：rust_eh_personality（类似于 C++ 的_CxxFrameHandler3）。
3. 自动追加 ExceptionData：LLVM 会将 Rust 的 drop 清理链数据（类似于 C++ 的Scope Table）物理追加在内存的最尾部。

结果：
1. 编译出来的 Rust 程序，其内核异常表布局与 C/C++ 完全一致
2. 正因为 Rust 完美支持并兼容这套 Windows x64 应用程序二进制接口 (ABI)：
3. 我们的 uwd 引擎可以用完全相同的算法去解析 ntdll.dll（微软用 C写的）和我们自己的 Rust 木马
4. 当 EDR 的驱动（用 C 写的）对我们的 Rust木马进行栈回溯时，它也能用同样的宏去正常解码我们的 Rust栈帧。这使得我们的栈欺骗技术在跨语言环境下拥有了 100% 的通用性