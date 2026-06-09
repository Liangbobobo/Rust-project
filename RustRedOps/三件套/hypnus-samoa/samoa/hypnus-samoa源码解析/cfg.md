- [CFG（控制流防护，Control Flow Guard）](#cfg控制流防护control-flow-guard)
  - [cfg.rs的解决方案](#cfgrs的解决方案)
  - [NtQueryInformationProcess](#ntqueryinformationprocess)


# CFG（控制流防护，Control Flow Guard）

CFG（控制流防护）是微软在 Windows 8.1 引入的一项极其强悍的底层安全防御机制:防御ROP/JOP等内存损坏漏洞的利用.内存损坏漏洞:cfg出来之前,黑客绕过杀软的方式主要是ROP,之后又演变为劫持函数指针:篡改内存中函数指针(如利用use-after-free漏洞,覆盖对象在内存中虚函数表),然程序通过间接调用(如 call rax/jmp r8)跳到黑客预置的恶意代码(shellcode/ROP链)上.为了封杀这种攻击,在win8.1之后引入CFG.相比CET半壶程序的返回路径,CFG是保护程序前进路径的.
1. 原理：由编译器MSVC和内核内存管理器memory manager共同实现.在编译时扫描源码,找出所有可能作为函数指针调用的函数地址,将这些地址记录在pe文件的load configuration directory加载配置目录中,作为白名单.之后对每次间接调用(call rax)强行插桩(调用rax前,先调用cfg检验).当程序启动时,内核ntoskrnl.exe将程序加载内存,(如果不用msvc编译器,在调用ntdll/kernel32/kernelbase(这些dll中的api都是msvc编译且开启cfg的)时,会直接暴露)它会读取pe头里的白名单,并在进程的虚拟地址空间的高位,开辟一块庞大的只读内存区域即CFG bitmap.在64位系统中,内存中每8字节在cfg bitmap中对应1个bit位.如果这8个字节的起始地址是一个合法的函数入口,内核就把这1个bit置为1,否则为0.这张巨大的bitmap覆盖进程的整个用户态地址空间.
2. 之前提到的cfg检验,实质是一个指向ntdll.dll!LdrpValidateUserCallTarget函数指针.该函数拿到即将跳转的目标地址(存放在rcx中),通过高效的位移运算,算出这个内存地址在cfg bitmap中对应的具体bit位置,然后读取bit的值,如果bit=1是合法入口,函数直接ret返回随后执行后面的rax.rax作为函数跳转的专用寄存器吗?不是,这里这是举例 如果bit=0,该函数不会抛出可以捕获的常规异常seh,而是直接触发int 29h中断.这是一个内核级中断指令,一旦触发os内核会下场以最高优先级瞬间强杀当前进程.

**对抗cfg**
1. cfg bitmap锁死了只读,对于一些需要在运行时动态生成汇编代码并执行的需求,微软留了一个后门api SetProcessValidCallTargets.samoa就是调用这个api给敏感跳转地址(如,NtContinue/特定gadget)在bitmap中打上合法标记(置为1)
2. cfg的盲区在于:只检查间接调用(call rax)和间接跳转(jmp rax),绝不检查返回指令ret.因此,经过的通过覆盖栈空间来控制ret跳转的rop链,cfg是防不住的.后来微软出了cet专防ret.这是微软十年来修补内存漏洞的抗争
3. 为了绕过cfg,被迫使用了冷门的SetProcessValidCallTargets.这本身就是一个明显的特征,除了带有JIT编译器的合法程序(chrome/.net/c#等),普通的业务进程(notepad.exe/svchost.exe)不应调用这个api来修改函数的执行属性.解决方案:
   3.1 在调用SetProcessValidCallTargets之前,先致盲edr:如果直接调用SetProcessValidCallTargets,edr在这个函数上的hook会波安静.hook是什么?但在本项目中samoa底层依赖dinvk/puerto和其中的硬件断点HWBP技术.在调用这个敏感api前,dinvk/puerto通过hwbp接管执行流,或者从硬盘上读取一份未被edr污染的干净dll并手动映射.
   3.2 SetProcessValidCallTargets只是微软提供给开发者的一个包装函数,存在于kernelbase.dll中.其底层是向内核发起一个系统调用,通常是NtSetInformationVirtualMemory.那么就可以不去碰kernelbase.dll中的高位特征函数.而是直接提取底层syscall ssn(系统调用号),利用内联汇编直接从应用层跳入内核.
   3.3 如果木马跑在notepad.exe中,edr会报警;如果将木马注入到一个正在运行的.net程序或浏览器子进程(如 msedge.exe)中.调用SetProcessValidCallTargets就变得正常了.
4. hypnus中,的大量使用了线程池回调劫持(把原生api/NtContinue的地址塞给系统当作回调执行).这就是一种非法间接调用.如果宿主进程开启CFG.当系统尝试回调NtContinue来恢复假线程上下文时,系统会发现NtContinue并不在当前线程池的合法调用位图.
5. hypnus的解决中,假设edr为了性能妥协,不敢直接对SetProcessValidCallTargets下hook,因为这个敏感函数调用及其频繁,下hook会卡死电脑:
   5.1 依赖dinvk/puerto的动态解析,pe文件中没有kernelbase.dll和SetProcessValidCallTargets.
   5.2 对SetProcessValidCallTargets的地址使用transmute将一个原始数据指针强转为一个可执行的函数指针.避开了对IAT的检测,直接发起调用
   

## cfg.rs的解决方案

1. is_cfg_enforced(),利用NtQueryInformationProcess查询当前木马注入的宿主进程,是否开启了CFG
2. add_cfg(),利用SetProcessValidCallTargets:这个api原本是给JIT编译器(如浏览器的JS引擎)动态生成代码时向系统申请CFG豁免用的.这里使用这个api,直接修改宿主进程的CFG验证位图,把需要利用的Gadget/系统函数标记为CFG_CALL_TARGET_VALID从而获取合法身份.最容易被拦截的时NtContinue
3. register_cfg_targets():


## NtQueryInformationProcess

```cpp

//#define NtCurrentLogonId() (NtCurrentPeb()->LogonId)

/**
 * The NtQueryInformationProcess routine retrieves information about the specified process.
 *
 * \param ProcessHandle A handle to the process.
 * \param ProcessInformationClass The type of process information to be retrieved.
 * \param ProcessInformation A pointer to a buffer that receives the process information.
 * \param ProcessInformationLength The size of the buffer pointed to by the ProcessInformation parameter.
 * \param ReturnLength An optional pointer to a variable that receives the size of the data returned.
 * \return NTSTATUS Successful or errant status.
 */
_Kernel_entry_
NTSYSCALLAPI
NTSTATUS
NTAPI
NtQueryInformationProcess(
    _In_ HANDLE ProcessHandle, // 查询的进程
    _In_ PROCESSINFOCLASS ProcessInformationClass,  // 查询的信息类别
    _Out_writes_bytes_(ProcessInformationLength) PVOID ProcessInformation,  // 查询结构填入什么地方
    _In_ ULONG ProcessInformationLength,  // 查询结果的缓冲区
    _Out_opt_ PULONG ReturnLength // 实际返回字节数
    );

```

1. Queries various information about the specified process. This function is partially documented in Windows SDK
2. 