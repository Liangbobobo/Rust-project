- [异常目录在发生异常的回溯机制](#异常目录在发生异常的回溯机制)
- [Struct StackSpoof](#struct-stackspoof)
- [申请第二个4k内存](#申请第二个4k内存)
- [RtlUserThreadStart](#rtluserthreadstart)
- [uwd::ignoring\_set\_fpreg](#uwdignoring_set_fpreg)
- [StackSpoof中用到的四个函数](#stackspoof中用到的四个函数)
- [\*(ctx\_spoof.Rsp as \*mut u64) = cfg.rtl\_acquire\_lock.as\_u64().add(0x17);](#ctx_spoofrsp-as-mut-u64--cfgrtl_acquire_lockas_u64add0x17)
- [cfg.base\_thread.as\_u64().add(0x14);](#cfgbase_threadas_u64add0x14)
- [let (add\_rsp\_addr, add\_rsp\_size)](#let-add_rsp_addr-add_rsp_size)
- [扩展rip rsp](#扩展rip-rsp)


## 异常目录在发生异常的回溯机制

win在loader指定程序exe及其dll时,将每个模块的内存范围和对应的异常表(IMAGE_RUNTIME_FUNCTION,在.pdata节)登记到内核级的 全局反向函数表（LdrpInvertedFunctionTable）中.该表明确划分了内存地址范围对应的模块.当异常发生/edr主动检查时时:
1. 定位:用当前崩溃的内存地址(rip指针)去全局花名册找该地址属于哪个模块
2. 去该模块专属的IMAGE_RUNTIME_FUNCTION异常表中查当前函数如何退栈
3. 提取上一层调用者返回地址.用新的返回地址再次回到第一步,直至回溯到线程最底层起点(如 RtlUserThreadStart)

**edr视角的异常:**
1. 正常软件调用栈一定是连贯的,如果edr回溯发现某个返回地址不再全局花名册(如 落在未注册的匿名内存).edr判定为无头shellcode
2. 对此,edr要么使用调用栈伪造call stack spoofing:把自己的木马伪装在合法连贯的系统函数调用链之下(spoof.rs使用的方式)
3. 要么用RtlAddFunctionTable,把自己伪造的异常表塞进系统全局花名册,让操作系统把木马当作合法的.这里需要继续展开

## Struct StackSpoof

```rust
/// Represents a reserved stack region for custom thread execution.
#[derive(Default, Debug, Clone, Copy)]
pub struct StackSpoof {
    /// Address of a `gadget_rbp`, which realigns the stack (`mov rsp, rbp; ret`).
    gadget_rbp: u64,

    /// Stack frame size for `BaseThreadInitThunk`.
    base_thread_size: u32,

    /// Stack frame size for `RtlUserThreadStart`.
    rtl_user_thread_size: u32,

    /// Stack frame size for `EnumResourcesW`.
    enum_date_size: u32,

    /// Stack frame size for `RtlAcquireSRWLockExclusive`.
    rlt_acquire_srw_size: u32,

    /// Type of gadget (`call [rbx]` or `jmp [rbx]`).
    gadget: GadgetKind,
}

```



## 申请第二个4k内存

```rust
  // Allocate pointer to gadget
        let mut gadget_ptr = null_mut();
        let mut ptr_size = 1 << 12;
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut gadget_ptr, 
            0, 
            &mut ptr_size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate gadget pointer page"));
        }
```

**为什么要再次申请4kb内存**
1. call rbx 与 call `[rbx]` 的区别:
* call rbx,cpu直接跳转到rbx中的内存位置.这种情况不需要第二块内存
* call `[rbx]`,cpu去rbx指向的内存中读取数据.如果cpu此时将该数据当作地址跳转,os会立即access violation.
  * 内存中不分指令和数据,指令层面才区分.call `[rbx]`时,cpu内部控制单元触发一次内存总线读取.去rbx指向的内存读取**8字节**数据.此时cpu读取的这8字节,在cpu视角下是不分数据/指令的
  * call指令除了读取,还会将读到的数据填入rip.且cpu会无条件的到rip指向的位置读取下一条要执行的指令
* 加上第二块内存的解决方案:
  * cpu执行call `[rbx]`.这里rbx中存入的是第二块内存的地址.
  * cpu读取8字节(这8字节被预先填入了第一块内存的地址)
  * cpu将该8字节填入rip
  * cpu执行压栈操作,把call指令下面那条指令地址压入当前栈,rsp-8,cpu把rip的当前值(即加上指令长度的rip,也叫返回地址)写入rsp中.cpu读取该8字节地址后,根据该该地址内部指令长度,加上rip的地址.让rip指向下一跳指令地址.
  * cpu之后通过rip找到第一块内存
  * cpu开始执行第一块内存中的指令


## RtlUserThreadStart

spoof.rs中通过
```rust
 let rtl_user = pe_ntdll
            .function_by_offset(cfg.rtl_user_thread.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32)
            .context(s!("missing unwind: RtlUserThreadStart"))?;
```

**背景知识**
win下,当创建一个线程时,cpu并不直接进入对应的代码,cpu首先进入ntdll!RtUserThreadStart.
* 该函数负责初始化thread的seh环境,并真正call业务代码.是所有线程的开始
* 物理位置: 是所有合法线程调用栈的物理最底层(栈底)
* EDR进行线程审计时,顺着当前线程的栈指针一路回溯,检查最后的终点是不是该函数.

**函数原型及特性**



## uwd::ignoring_set_fpreg

```rust
/// Computes the total stack frame size of a function while ignoring any `setfp` frames. 
/// Useful for identifying spoof-compatible RUNTIME_FUNCTION entries.
pub fn ignoring_set_fpreg(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<u32> {
    unsafe {
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;
        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;
        let flag = (*unwind_info).VersionFlags.Flags();

        let mut i = 0usize;
        let mut total_stack = 0u32;
        while i < (*unwind_info).CountOfCodes as usize {
            // Accessing `UNWIND_CODE` based on the index
            let unwind_code = unwind_code.add(i);

            // Information used in operation codes
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // Saves a non-volatile register on the stack.
                //
                // Example: push <reg>
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    total_stack += 8;
                    i += 1;
                }

                // Allocates small space in the stack.
                //
                // Example (OpInfo = 3): sub rsp, 0x20  ; Aloca 32 bytes (OpInfo + 1) * 8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as u32;
                    i += 1;
                }

                // Allocates large space on the stack.
                // - OpInfo == 0: The next slot contains the /8 size of the allocation (maximum 512 KB - 8).
                // - OpInfo == 1: The next two slots contain the full size of the allocation (up to 4 GB - 8).
                //
                // Example (OpInfo == 0): sub rsp, 0x100 ; Allocates 256 bytes
                // Example (OpInfo == 1): sub rsp, 0x10000 ; Allocates 65536 bytes (two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // Case 1: OpInfo == 0 (Size in 1 slot, divided by 8)
                        // Multiplies by 8 to the actual value

                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset as u32;

                        // Consumes 2 slots (1 for the instruction, 1 for the size divided by 8)
                        i += 2
                    } else {
                        // Case 2: OpInfo == 1 (Size in 2 slots, 32 bits)
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset as u32;

                        // Consumes 3 slots (1 for the instruction, 2 for the full size)
                        i += 3
                    }
                }

                // UWOP_SAVE_NONVOL: Saves the contents of a non-volatile register in a specific position on the stack.
                // - Reg: Name of the saved register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x40], rsi ; Saves the contents of RSI in RSP + 0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    i += 2;
                }

                // Saves a non-volatile register to a stack address with a long offset.
                // - Reg: Name of the saved register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x1040], rsi ; Saves the contents of RSI in RSP + 0x1040.
                Ok(UWOP_SAVE_NONVOL_BIG) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    i += 3;
                }

                // Saves the contents of a non-volatile XMM register on the stack.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x20], xmm6 ; Saves the contents of XMM6 in RSP + 0x20.
                Ok(UWOP_SAVE_XMM128) => i += 2,

                // UWOP_SAVE_XMM128BIG: Saves the contents of a non-volatile XMM register to a stack address with a long offset.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x1040], xmm6 ; Saves the contents of XMM6 in RSP + 0x1040.
                Ok(UWOP_SAVE_XMM128BIG) => i += 3,

                // Ignoring.
                Ok(UWOP_SET_FPREG) => i += 1,

                // Reserved code, not currently used.
                Ok(UWOP_EPILOG) | Ok(UWOP_SPARE_CODE) => i += 1,

                // Push a machine frame. This unwind code is used to record the effect of a hardware interrupt or exception.
                Ok(UWOP_PUSH_MACH_FRAME) => {
                    total_stack += if op_info == 0 { 0x40 } else { 0x48 };
                    i += 1
                }
                _ => {}
            }
        }

        // If there is a chain unwind structure, it too must be processed
        // recursively and included in the stack size calculation.
        if (flag & UNW_FLAG_CHAININFO) != 0 {
            let count = (*unwind_info).CountOfCodes as usize;
            let index = if count & 1 == 1 { count + 1 } else { count };
            let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
            if let Some(chained_stack) = ignoring_set_fpreg(module, &*runtime) {
                total_stack += chained_stack;
            } else {
                return None;
            }
        }

        Some(total_stack)
    }
}
```

```rust
// 关联的uwd中关于unwind结构体

/// Structure containing the unwind information of a function.
#[repr(C)]
pub struct UNWIND_INFO {
    /// Separate structure containing `Version` and `Flags`.
    pub VersionFlags: UNWIND_VERSION_FLAGS,

    /// Size of the function prologue in bytes.
    pub SizeOfProlog: u8,

    /// Number of non-array `UnwindCode` entries.
    pub CountOfCodes: u8,

    /// Separate structure containing `FrameRegister` and `FrameOffset`.
    pub FrameInfo: UNWIND_FRAME_INFO,

    /// Array of unwind codes describing specific operations.
    pub UnwindCode: UNWIND_CODE,

    /// Union containing `ExceptionHandler` or `FunctionEntry`.
    pub Anonymous: UNWIND_INFO_0,

    /// Optional exception data.
    pub ExceptionData: u32,
}


```

解复卷（Unwind）指令流

**调用了uwd::ignoring_set_fpreg,该函数在win64下,递归解析pe文件异常目录中的底层操作码(Unwind Codes),模拟函数序言对rsp指针的物理修改过程,并过滤掉不改变栈深的寄存器保存指令,从而在不依赖rbp链的前提下,实时计算出任意系统函数在内存中确切的栈帧厚度**


## StackSpoof中用到的四个函数

win下,几乎所有的用户态线程均遵循以下固定的启动流程:
1. RtUserThreadStrat
2. BaseThreadInitThunk
3. Usercode

EDR视角:如果edr发现一个线程的调用栈能一路回溯到RtUserThreadStart,会认为这是一个由系统标准api(如CreateThread)创建的正经线程

本项目中通过手写内存,强行让 BaseThreadInitThunk 认为是由 RtlUserThreadStart调用的

##  *(ctx_spoof.Rsp as *mut u64) = cfg.rtl_acquire_lock.as_u64().add(0x17);

cfg.rtl_acquire_lock：指向 ntdll!RtlAcquireSRWLockExclusive

大多数 64 位 Windows 版本的 ntdll.dll中，RtlAcquireSRWLockExclusive + 0x17 这个位置，恰好紧跟在一条 call指令之后

**背景:**
edr扫描伪造栈时,不仅看返回地址指向哪个函数,还会执行反向字节检查  
1. 如果栈上的返回地址是A,那么在地址A之前的几个字节必须有一条call指令
2. edr会读A-5字节(近跳转)或A-2字节(寄存器跳转)
3. 如果读到0xE8(call指令的字节码),则edr认为是一个正常的函数调用
4. 如果把返回地址设为函数开头(不是指定的0x17),edr会发现A-1处是上一个函数的垃圾数据,不是call.edr会报Invalid Stack Frame / Non-Call Return Address


**0x17的选择**
1. 在标准win10/11的ntdll.dll中,ntdll!RtlAcquireSRWLockExclusive的0x17处通常是上一条用来处理锁冲突的call指令结束后的第一个字节.
2. 通过将返回地址设为0x17,在内存中伪造了在ntdll!RtlAcquireSRWLockExclusive的第23字节处(0x17)执行完了一次调用,之后会返回
3. edr顺着上一个ZwWaitForWork 向上回溯时,会到RtlAcquireSRWLockExclusive + 0x17处.edr会检查这个地址的前几个字节,而因为0x17处之前在ntdll中确实是一个合法指令(call一个子函数),edr会认为这个调用链是系统调用系统
4. 这种正在处理同步锁的状态是os中常见的背景噪音,edr的启发式引擎会给这种栈分配极高的信誉信任,而跳过深层次的内存扫描
5. 此时RtlAcquireSRWLockExclusive的栈帧并没有载入当前栈中:EDR或系统栈回溯器(RtlvirtualUnwind)在扫描栈时,只关心返回地址,它不关心也不能检查函数内部局部变量
6. 这套伪造的栈,cpu根本不会执行.只是把伪造的函数地址放入栈上,根本没有执行指令.
7. 在线程被唤醒的时候,工作线程已经通过NtSetContextThread将主线程的rsp重新改回原始高地址.




##  cfg.base_thread.as_u64().add(0x14);

当cpu执行call rax指令进入子函数时,硬件会自动将call指令的下一条指令地址(BaseThreadInitThunk + 0x14)作为返回地址压入栈顶.因此,当线程再运行/挂起休眠时,栈上遗留的返回地址必须指向call的下一条指令.




现代edr在stack walking时,不仅检查返回地址是否属于合法dll,还检查该地址在函数内部合理性.如果返回地址与call无关的指令地址,edr告警.此处指向0x14处的mov ecx,eax.能欺骗edr和系统展开器RtlVirtualUnwind.当前线程之前是由BaseThreadInitThunk发起的函数调用.

```c
void __fastcall BaseThreadInitThunk( DWORD unknown, LPTHREAD_START_ROUTINE entry, void *arg )
{
    RtlExitUserThread( entry( arg ) );
}
```
1. __fastcall:win32下前两个参数通过ecx edx传递;win64下所有函数调用默认都是隐式的__fastcall
2. 以win64为例:该函数有3个参数,unknow对应rcx,代表系统内部标志位,用户态线程通常为1;entry对应rcx,函数指针,指向用CreatThread时传入的真正的线程函数(如恶意载荷入口/正常业务代码);arg对应r8,传给线程入口函数的自定义参数指针(CreatThread的最后一个参数lpParameter)
3. 函数体内 entry( arg ) ,执行到这里时,调用用户编写的线程函数,将参数arg传给它,此时控制权交给用户.
4. 用户写线程函数执行完毕/用户忘记在代码最后调用ExitThread:它的返回值(一个DWORD退出码)被传给RtlExitUserThread.由RtlExitUserThread发起ring 0系统调用NtTerminateThread,通知win内核回收堆栈\TEB等资源.


**Rsp存放的是一个内存地址(即当前栈顶在ram中的位置,相当于c的指针).  
返回地址是存放在rsp指向的内存中的内容,该内容是函数的地址**

以上,进入BaseThreadInitThunk时,它的三个参数分别在rcx rdx r8中.


BaseThreadInitThunk的反汇编代码:

| 偏移量 | 机器码 | 指令 | 注释 |
|--------|--------|------|------|
| 0x00 | 48 83 ec 28 | sub rsp, 28h | 申请栈空间 (32字节影子空间 + 8字节对齐) |
| 0x04 | 48 8b c2 | mov rax, rdx | 将线程参数放入RAX |
| 0x07 | 33 d2 | xor edx, edx | |
| 0x09 | 48 8b cb | mov rcx, rbx | |
| 0x0c | ff d0 | call rax | ◄── 调用真正的线程入口函数（执行我们的代码） |
| 0x0e | 90 | nop | （根据系统版本不同，可能有微小填充） |
| ... | ... | ... | |
| 0x14 | 8b c8 | mov ecx, eax | ◄── 偏移量 0x14 处的指令：保存线程返回的退出码 |
| 0x16 | ff 15 xx xx xx xx | call RtlExitUserThread | 调用线程退出函数 |

上述的汇编代码,是微软MSVC编译器,在编译win的kernel32.dll时,根据c语言的逻辑,自动决定使用哪些寄存器(如 rax暂存函数指针,rbx备份参数),并生成这些机器码指令.之后由cpu执行.

[扩展rip rsp](#扩展rip-rsp)

该反汇编代码包含函数序言 函数体 收尾:
1. 序言:0x00: sub rsp, 28h,其目的开辟空间,对齐内存.如果函数要修改非易失性寄存器(如 rbx),一般会把rbx压栈备份(push rbx),如果仅仅是读取rbx就不会压栈.
2. 例外,即使修改非易失性寄存器,现代MSVC也不一定压栈.因为,win64下,函数序言一开始通过sub rsp,28h分配足够多栈空间.编译器更倾向用mov将非易失性寄存器被分到已分配的栈帧中.使用结束后再还原这些使用的非易失性寄存器.这么做可以使rsp在整个函数体执行期间保持固定,便于系统进行异常展开SEH Unwind和调试
3. 当用户代码执行到最后一行ret,cpu: 1从当前栈顶读取压入的返回地址(BaseThreadInitThunk + 0x14),并送入rip.2将rsp + 8:弹出栈
4. BaseThreadInitThunk + 0x14处是BaseThreadInitThunk执行时的cpu的rip地址,此处对应的机器码mov ecx,eax.不是rsp这个总是指向栈顶的地址
5. 子函数执行完最后一行的ret时,cpu从栈顶读取返回地址BaseThreadInitThunk + 0x14,并写入rip.cpu执行此处mov指令后,cpu的硬件控制单元自动将rip加上当前指令长度2字节,新的rip=0x14+2 .rip自动指向0x16处,这里是call RtlExitUserThread .cpu执行这条call,才真正修改rip,cpu跳进 RtlExitUserThread  函数内部去销毁线程
6. *(ctx_spoof.Rsp.add((cfg.stack.rlt_acquire_srw_size + 8) as u64))是BaseThreadInitThunk的子函数RtlAcquireSRWLockExclusive在执行完,准备调用ret返回到BaseThreadInitThunk时rsp指向的内存地址




这里展示了当执行 call 到我们的线程，再到我们栈伪造休眠时，栈空间和 RSP 指针的真实物理状态：

    【 阶段 1：普通执行阶段 】                  【 阶段 2：发起 ROP 链栈伪造休眠 】

    内存地址 (高地址)                          内存地址 (高地址)
    ┌──────────────────────────────┐          ┌──────────────────────────────┐
    │ ...                          │          │ ...                          │
    ├──────────────────────────────┤          ├──────────────────────────────┤
    │ 原始栈数据                   │          │ 原始栈数据                   │
    ├──────────────────────────────┤          ├──────────────────────────────┤
    │ BaseThreadInitThunk 栈帧     │          │ BaseThreadInitThunk 栈帧     │
    ├──────────────────────────────┤          ├──────────────────────────────┤
    │ 返回地址: +0x14              │          │ 返回地址: +0x14              │
    ├──────────────────────────────┤          ├──────────────────────────────┤ ◄── 真实 RSP 斩断于此
    │ 我们的 ThreadProc 栈帧       │          │ (休眠期间这部分明文数据被加密) │
    ├──────────────────────────────┤          ├──────────────────────────────┤
    │                              │          │ ====== 20 KB Gap 安全区 =====│
    │                              │          │   (用于吸收 EDR 临时栈写入)   │
    │                              │          ├──────────────────────────────┤
    │    空闲栈空间 (向低地址增长)   │          │ 伪造返回地址: +0x14          │ ◄── 伪造的 BaseThreadInitThunk 帧
    │                              │          ├──────────────────────────────┤
    │                              │          │ 伪造影子空间 (32 字节)       │
    │                              │          ├──────────────────────────────┤ ◄── 伪造的虚拟 RSP (EDR 扫描起点)
    │                              │          │                              │
    ▼ 内存地址 (低地址)                          ▼ 内存地址 (低地址)




## let (add_rsp_addr, add_rsp_size) 

add rsp, 0x58 ; ret
1. 在hypnus的执行流中,当NtContinue被触发或线程池回调开始时,rsp所处位置可能残留系统函数局部变量/参数.
2. 执行add rsp, 0x58 ; ret后,rsp向上(高地址)跳过0x58字节(十进制88),在内存中把这88字节的数据全部作废
3. 这条add rsp, 0x58 ; ret通常位于kernelbase某个真实函数的结尾.即使edr扫描到此处,会认为是一个合法系统函数执行完后,正清理自己的栈并返回.
4. 栈指针指向预埋的第一个返回地址
5. ret:从这个位置弹出地址,开启伪造
6. ret:cpu读取当前rsp指向内存地址中的8字节,并载入rip,执行rsp+8.其实质等于pop rip
7. ret:本项目中在ROP下,第0字节写入add_rsp_addr;在0x58+8处写入gadget_add.ret后rsp会跳过88字节,到达gadget_add处,cpu取出其中8字节数据给rip,然后rsp+8.顺利的进入下一个函数处
8. 88字节+8字节的返回地址=96是16的倍数
9. ret时,cet会让cpu检查shadow stack.但在执行这个gadget前,hypnus已经通过Ntcontinue修好了系统状态,并利用内核特权同步了硬件影子栈.这里的ret是经过硬件背书的合法返回
10. 项目中将Ntcontinue通过config放入rax寄存器



## 扩展rip rsp

rip rsp最直接的联系发生在函数调用和返回的瞬间,他们通过栈内存进行数据交换.

核心纽带 call ret硬件指令

**Call**
1. call:rip写入rsp. cpu把rip的当前值即call的下一条指令(也就是返回地址)写入rsp.
2. rsp-8 栈顶下移
3. rip载入新函数入口地址,cpu跳转执行

**ret**
1. rsp写入rip:rsp +8 ,栈顶上移,回到返回地址所在位置
2. cpu从当前rsp指向的内存中读取8字节数据,并强行写入rip
3. cpu顺着新rip地址跳转,回到调用者函数中继续执行