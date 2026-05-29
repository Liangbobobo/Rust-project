- [CFG（控制流防护，Control Flow Guard）](#cfg控制流防护control-flow-guard)
  - [cfg.rs的解决方案](#cfgrs的解决方案)
  - [NtQueryInformationProcess](#ntqueryinformationprocess)


# CFG（控制流防护，Control Flow Guard）

CFG（控制流防护）是微软在 Windows 8.1 引入的一项极其强悍的底层安全防御机制:防御ROP/JOP等内存漏洞的利用
1. 原理：操作系统在内存里维护了一张巨大的Bitmap（位图白名单）。在程序执行任何间接调用或跳转（比如 call rax 或 jmp rdx或函数指针回调）之前，CPU 必须先去查这个CFG验证位图。只有被编译器提前登记为合法入口点的内存地址,才允许跳过去执行.如果跳到了未登记地址,进程立刻触发 0xC0000005 崩溃.
2. hypnus中,的大量使用了线程池回调劫持(把原生api/NtContinue的地址塞给系统当作回调执行).这就是一种非法间接调用.如果宿主进程开启CFG.当系统尝试回调NtContinue来恢复假线程上下文时,系统会发现NtContinue并不在当前线程池的合法调用位图.

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