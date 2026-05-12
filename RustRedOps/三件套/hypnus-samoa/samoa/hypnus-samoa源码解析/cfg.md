

## CFG（控制流防护，Control Flow Guard）

CFG（控制流防护）是微软在 Windows 8.1 引入的一项极其强悍的底层安全防御机制
1. 原理：操作系统在内存里维护了一张巨大的Bitmap（位图白名单）。每当程序执行间接调用或跳转（比如 call rax 或 jmprdx）时，CPU 必须先去查这张白名单。如果 rax指向的地址不在白名单里，说明执行流被劫持了，进程立刻触发 0xC0000005 崩溃
2. hypnus.rs中的timer/foliage等函数中,有大量jmp和NtContinue的上下文劫持.如果目标进程开启CFG,会直接0xC0000005 崩溃
3. 目标进程开启了CFG,必须调用SetProcessValidCallTargets 这个系统api,把自己的trampoline悄悄放入白名单(CFG Bypass)


### NtQueryInformationProcess

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