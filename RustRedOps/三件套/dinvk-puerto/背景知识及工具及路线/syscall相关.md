- [syscall](#syscall)
  - [Windows 系统调用的完整生命周期](#windows-系统调用的完整生命周期)
  - [关于 Stub](#关于-stub)
  - [IMAGE\_EXPORT\_DIRECTORY-\>AddressOfFunctions](#image_export_directory-addressoffunctions)
  - [Syscall 函数体 (Stub)相关](#syscall-函数体-stub相关)
  - [关于ssn](#关于ssn)
  - [扩展-rust代码哪些情况会进入内核态?](#扩展-rust代码哪些情况会进入内核态)


# syscall

**发生系统调用的原因**:  
* 用户态 (Ring 3)：你的程序运行在这里。你没有权限直接读写硬盘、修改其他进程内存或操作硬件。  
* 内核态 (Ring 0)：Windows内核（ntoskrnl.exe）运行在这里。它拥有至高无上的权限。  
* 系统调用(Syscall)：这就是那扇“传送门”。当你需要申请内存（NtAllocateVirtualMemory）时，你必须调用内核提供的功能

## Windows 系统调用的完整生命周期

1. 第一步：你的程序发起调用  
在 Rust 代码使用 Rust 标准库或 windows crate 时  
* Rust 代码: std::fs::File::create("test.txt")
* ↓ 调用: kernel32.dll!CreateFileW（这是 Win32 API，负责对参数进行高级封装和检查）
* ↓ 调用: ntdll.dll!NtCreateFile（这是 Native API，是进入内核前的最后一站）
* ↓ 结果: 进入 ntdll 的 Stub（那段 32 字节的汇编）。

什么情况下会进入kernel32.dll?

**根据 Windows x64 调用约定，参数这样放：**  
* RCX: 第 1 个参数 (ProcessHandle)
* RDX: 第 2 个参数 (DesiredAccess)
* R8: 第 3 个参数 (ObjectAttributes)
* R9: 第 4 个参数 (ClientId)
* 栈 (Stack): 如果有第 5 个及以后的参数，放在栈上。

当涉及到
2. 第二步：进入 ntdll.dll（这就是 Stub 所在地）  
程序会跳转到 ntdll.dll 里的 NtOpenProcess 函数地址。  
这个地址就是 `AddressOfFunctions` 数组里存的那个 RVA 加上基址后的位置

在这个地址上，你会看到那段 32 字节的 Stub 代码：
```asm
mov r10, rcx       ; 1. 把第一个参数从 RCX 挪到 R10 (内核要求)
mov eax, 0x26      ; 2. 把 NtOpenProcess 的系统调用号 (SSN) 放入EAX
syscall            ; 3. 触发传送门！CPU 瞬间切换到内核态
ret                ; 4. 内核回来后，返回到你的程序
```

* 执行地址存放在 Stub 中吗？ 不，Stub 本身就是 执行代码。AddressOfFunctions指向的就是这段代码的开头。
* Stub 里的跳转：Stub内部通常没有复杂的跳转，它的任务非常纯粹：报上编号（SSN），执行 `syscall`。

3. 第三步：内核接管
当 CPU 执行到 syscall 指令时：  
   1. CPU 查阅一个特殊的寄存器（LSTAR），里面存着内核里“系统调用处理中心”的地址。
   2. CPU 跳转到内核代码。
   3. 内核看一眼 EAX 寄存器：哦，是 0x26。
   4. 内核在自己的表格里查：0x26 对应的是 NtOpenProcess 的内核实现函数。
   5. 内核执行真正的操作，完成后通过 sysret 指令回到用户态。

## 关于 Stub

1. Stub 是入口：AddressOfFunctions 指向的是这段汇编代码的起始。
2. Stub 是中转站：它不含业务逻辑，只负责“翻译参数”和“发起调用”。
3. 32 字节是布局：Windows 将这些中转站函数（Stubs）整齐地排列在 ntdll 的 .text段。
       * NtFunction1 [32字节]
       * NtFunction2 [32字节]
       * NtFunction3 [32字节]
4. 红队的骚操作：
* Hell's Gate：既然 AddressOfFunctions 告诉了我 NtFunction2的地址，我直接去那个地址读出 mov eax, SSN 里的 SSN。
* Halo's Gate：如果 NtFunction2 的这 32 字节被 EDR 改成了 JMPEDR_Check，我就去读 NtFunction1 的地址（也就是 NtFunction2 地址减去 32字节），读出它的 SSN，然后加 1。

## IMAGE_EXPORT_DIRECTORY->AddressOfFunctions

1. 在 functions[] 数组中（存储时）
   * 占用字节：4 字节
   * 类型：u32 (即 RVA)
   * 解释：无论是在 x86 还是 x64 的 PE 文件结构中，导出表里的 AddressOfFunctions数组中存储的元素永远是 32 位的 RVA。

 2. 计算出绝对地址后（即 Rust 中的指针本身）
   * 占用字节：8 字节
   * 类型：*const u8 / *const i8 (在 x64 下等同于 u64 或 usize)
   * 解释：当你执行 module_base + rva 时，结果是一个 64 位的虚拟地址。**在 Rust中，任何裸指针（*const T）在 64 位系统上都占用 8 个字节。**

3. 指针指向的内容（机器码 Opcode）
   * 占用字节：1 字节（按单位计）/ 约 32 字节（按功能块计）
   * 解释：
    * 因为类型是 *const u8，当你解引用（read()）它时，你一次读取的是1 字节的数据。
    * 但在红队开发（Hell's Gate）的语境下，一个完整的 Syscall Stub（从 mov r10,rcx 到 ret）通常占用 32 字节 (0x20)。这就是为什么我们在 Halo's Gate中搜索邻居时，步进值通常设为 32。

| 描述对象                         | 数据类型  | 占用空间 |
|----------------------------------|-----------|----------|
| 导出表数组里的元素 (RVA)         | u32       | 4 字节   |
| Rust 指针变量本身 (VA)           | *const u8 | 8 字节   |
| 解引用指针读取的单个指令字节     | u8        | 1 字节   |
| 一个标准的 Syscall 函数体 (Stub) | [u8; 32]  | 32 字节  |

## Syscall 函数体 (Stub)相关

在 Windows 的 ntdll.dll 中，系统调用函数（我们称之为Stub）不是乱排的
,是整齐地按块排列(每个都是32字节)

**什么是 Syscall Stub？**  
当你调用 NtOpenProcess 时，你实际上是进入了 ntdll里的这一小段汇编代码。它的唯一作用就是把系统调用号（SSN）放进 EAX 寄存器，然后执行syscall 进入内核。这块小代码就叫 Stub（存根）

**为什么是 32 字节？**  
为了内存对齐和执行效率，Windows 编译器会将这些 Stub 按照固定的间距进行排列.  
但在x86中,这个值是5或15字节.  

同时存在非标准stub, 并不是 ntdll 里所有的导出函数都是 32 字节。  
* 一些复杂的导出函数（非系统调用，如 LdrLoadDll）可能有几百甚至上千字节长。
* 但 `Nt`系列系统调用（即你关心的那些）是由同一个代码生成模板产生的，所以它们之间保持了高度的 32 字节一致性。

1. 32 字节是 “模板化生成的机器码” + “内存对齐要求” 的共同产物  
2. 在目前的 winx64环境下它是绝对的标准

在 64 位 Windows 中，绝大多数 Syscall Stub 的结构如下（以 NtReadFile 为例）,其中机器码 Hex(十六进制)是根据Intel和AMD的对照表由汇编器转为二进制的机器码的,机器码是cpu直接识别的命令：

| 偏移  | 机器码 (Hex)            | 汇编指令                    | 长度     |
|-------|-------------------------|-----------------------------|----------|
| +0x00 | 4C 8B D1                | mov r10, rcx                | 3 字节   |
| +0x03 | B8 06 00 00 00          | mov eax, 0x6 (SSN)          | 5 字节   |
| +0x08 | F6 04 25 08 03 FE 7F 01 | test byte ptr [7FFE0308], 1 | 9 字节   |
| +0x10 | 75 03                   | jne short near_ptr          | 2 字节   |
| +0x12 | 0F 05                   | syscall                     | 2 字节   |
| +0x14 | C3                      | ret                         | 1 字节   |
| +0x15 | `0F 1F 44 00 00` ...      | NOP (填充/对齐)             | 剩余字节 |


虽然实际指令加起来只有 21 字节左右，但 Windows 会用 NOP指令（无作用指令，如 0F 1F ...）将每一个函数补齐到 32 字节 (`0x20`) 的边界上

## 关于ssn

如前所述,mov eax, ssn 代表在stub中将ssn传入eax中,以进行系统调用.但是在三个门中解析出ssn时,是以4个字节32位进行分析ssn的.那么  
**ssn到底是16位还是32位?**

即指令(mov eax, ssn)是 32 位的，而我们在代码里却把ssn当成 u16来处理?

1. 硬件层面：指令要求 32 位  
在 x64 汇编中，mov eax, <常量> 这条指令的格式是固定的。操作码 0xB8 后面必须紧跟一个4 字节（32 位）的立即数
* 即使你的系统调用号只是 0x18，机器码也必须写成：B8 18 00 00 00（补齐 4个字节的数据位）
* 如果你只给 2 个字节，CPU 会解错指令，导致程序崩溃。

2. 操作系统层面：SSN 的实际大小(16位)  
虽然硬件预留了 32 位的空间，但 Windows 历史上所有的系统调用号（SSN）其实都很小：
* 目前 Windows 10/11 的 SSN 通常在 0x0000 到 0x1000（即 0 到 4096）之间
* `u16` 的最大值是 65535。这意味着对于目前的 Windows 来说，u16 已经绰绰有余了，根本用不到 32 位





















## 扩展-rust代码哪些情况会进入内核态?

 不需要进入内核的情况（纯用户态）
  绝大多数逻辑运算、内存操作和控制流都在用户态完成，速度极快：
   * 基础运算：加减乘除、位运算、逻辑判断（if/else）。
   * 栈内存分配：let x = 10; 或定义结构体。
   * 堆内存分配（部分）：虽然 Box::new 或 Vec::push 最终可能调用
     malloc，但现代内存分配器（如 jemalloc 或
     mimalloc）维护着用户态的内存池。只有当内存池耗尽需要向操作系统申请新页（Page）时
     ，才会进入内核。
   * 函数调用：普通的函数调用（fn）和闭包。
   * 字符串处理：String 的拼接、格式化、查找。
   * 标准库中的纯算法：Vec::sort、HashMap::insert、迭代器操作。


  必须进入内核的情况（系统调用）
  任何涉及 硬件资源、全局系统状态 或 进程间交互 的操作，都必须通过 syscall 进入内核：
   * 文件操作：File::open、read、write（需要磁盘 I/O）。
   * 网络通信：TcpStream::connect、UdpSocket::send（需要网卡驱动）。
   * 线程/进程管理：std::thread::spawn、Command::new（需要内核调度器）。
   * 时间获取：SystemTime::now（需要读取硬件时钟）。
   * 控制台输出：println!（最终调用 WriteFile 到 stdout，涉及屏幕/终端驱动）。
   * 同步原语（部分）：Mutex、RwLock
     在无竞争时可能在用户态自旋（Spin），但一旦发生竞争需要挂起线程（Park），就必须调
     用内核的 NtWaitForSingleObject。