#![allow(unused)]

// 本文件(mod)在项目中作用:
// 提供其他模块用到的windows api/dll的rust定义:找到地址后通过transmute转为rust函数指针(主动调用native api的ffi安全wrapper)

// 问题:

// 需注意:

use core::{ffi::c_void,mem::transmute,ptr::null_mut};
use uwd::syscall;
use crate::{debug_log,stealth_bail};
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use puerto::types::{EVENT_TYPE,HANDLE,LARGE_INTEGER,NTSTATUS,STATUS_UNSUCCESSFUL};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};
use spin::Once;

// obfstr!底层加密/解密都在stack/数据段完成,无内存分配
use obfstr::{obfstr as s};

use crate::types::*;

/// Structure containing all function pointers resolved only once.用于动态计算项目使用的win api地址
pub struct Winapis {
    pub NtSignalAndWaitForSingleObject: NtSignalAndWaitForSingleObjectFn,
    pub NtQueueApcThread: NtQueueApcThreadFn,
    pub NtAlertResumeThread: NtAlertResumeThreadFn,
    pub NtQueryInformationProcess: NtQueryInformationProcessFn,
    pub NtLockVirtualMemory: NtLockVirtualMemoryFn,
    pub NtDuplicateObject: NtDuplicateObjectFn,
    pub NtCreateEvent: NtCreateEventFn,
    pub NtWaitForSingleObject: NtWaitForSingleObjectFn,
    pub NtClose: NtCloseFn,
    pub TpAllocPool: TpAllocPoolFn,
    pub TpSetPoolStackInformation: TpSetPoolStackInformationFn,
    pub TpSetPoolMinThreads: TpSetPoolMinThreadsFn,
    pub TpSetPoolMaxThreads: TpSetPoolMaxThreadsFn,
    pub TpAllocTimer: TpAllocFn,
    pub TpSetTimer: TpSetTimerFn,
    pub TpAllocWait: TpAllocFn,
    pub TpSetWait: TpSetWaitFn,
    pub NtSetEvent: NtSetEventFn,
    pub CloseThreadpool: CloseThreadpoolFn,
    pub RtlWalkHeap: RtlWalkHeapFn,
    pub SetProcessValidCallTargets: SetProcessValidCallTargetsFn,
    pub ConvertFiberToThread: ConvertFiberToThreadFn,
    pub ConvertThreadToFiber: ConvertThreadToFiberFn,
    pub CreateFiber: CreateFiberFn,
    pub DeleteFiber: DeleteFiberFn,
    pub SwitchToFiber: SwitchToFiberFn,
}

/// one-time lazy initialization of the structure with resolved pointer.在调用winapis()时通过WINAPIS.call_once()时才真正初始化
static WINAPIS:Once<Winapis>=Once::new();

/// Returns a reference to the resolved winapis structure.同时实现Winapis结构体的初始化
#[inline]
pub fn winapis() -> &'static Winapis {
    WINAPIS.call_once(|| {
        let ntdll = get_ntdll_address();
        let kernelbase = get_module_address(Some(3594687209u32), Some(fnv1a_utf16));
        let kernel32 = get_module_address(Some(1303842461u32), Some(fnv1a_utf16));
        unsafe {
            Winapis {
                NtSignalAndWaitForSingleObject: transmute(get_proc_address(Some(ntdll), Some(1007916823u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtQueueApcThread: transmute(get_proc_address(Some(ntdll), Some(1975708656u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtAlertResumeThread: transmute(get_proc_address(Some(ntdll), Some(1455008164u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtQueryInformationProcess: transmute(get_proc_address(Some(ntdll), Some(3292364416u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtLockVirtualMemory: transmute(get_proc_address(Some(ntdll), Some(768337382u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtDuplicateObject: transmute(get_proc_address(Some(ntdll), Some(2312692127u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtCreateEvent: transmute(get_proc_address(Some(ntdll), Some(3888976213u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtWaitForSingleObject: transmute(get_proc_address(Some(ntdll), Some(1015357890u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtClose: transmute(get_proc_address(Some(ntdll), Some(2210716347u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpAllocPool: transmute(get_proc_address(Some(ntdll), Some(3448693442u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpSetPoolStackInformation: transmute(get_proc_address(Some(ntdll), Some(716198303u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpSetPoolMinThreads: transmute(get_proc_address(Some(ntdll), Some(3288802426u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpSetPoolMaxThreads: transmute(get_proc_address(Some(ntdll), Some(340374372u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpAllocTimer: transmute(get_proc_address(Some(ntdll), Some(2216012281u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpSetTimer: transmute(get_proc_address(Some(ntdll), Some(1106973174u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpAllocWait: transmute(get_proc_address(Some(ntdll), Some(2675932341u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                TpSetWait: transmute(get_proc_address(Some(ntdll), Some(3623041482u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                NtSetEvent: transmute(get_proc_address(Some(ntdll), Some(2314183347u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                CloseThreadpool: transmute(get_proc_address(kernel32, Some(1222214419u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                RtlWalkHeap: transmute(get_proc_address(Some(ntdll), Some(4217964784u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                SetProcessValidCallTargets: transmute(get_proc_address(kernelbase, Some(2170414296u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                ConvertFiberToThread: transmute(get_proc_address(kernelbase, Some(973991775u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                ConvertThreadToFiber: transmute(get_proc_address(kernelbase, Some(147702319u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                CreateFiber: transmute(get_proc_address(kernelbase, Some(1956994521u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                DeleteFiber: transmute(get_proc_address(kernelbase, Some(2109382916u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
                SwitchToFiber: transmute(get_proc_address(kernelbase, Some(1447875972u32), Some(fnv1a_utf16)).unwrap_or(null_mut())),
            }
        }
    })
}

// 以上已经动态解析了win api的地址并转为rust的函数指针.但仍然需要进行一层wrapper:
// 1. unsafe集中隔离 2.强制触发laze load(WINAPIS.call_once) .避免调用时尚未初始化 3. 方便扩展,比如插入debug_log!
/// Wrapper for the `NtClose` API.
#[inline]
pub fn NtClose(Handle: HANDLE) -> NTSTATUS {
    unsafe { (winapis().NtClose)(Handle) }
}

/// Wrapper for the `NtSetEvent` API.
#[inline]
pub fn NtSetEvent(hEvent: *mut c_void, PreviousState: *mut i32) -> NTSTATUS {
    unsafe { (winapis().NtSetEvent)(hEvent, PreviousState) }
}

/// Wrapper for the `NtWaitForSingleObject` API.
#[inline]
pub fn NtWaitForSingleObject(Handle: HANDLE, Alertable: u8, Timeout: *mut i32) -> NTSTATUS {
    unsafe { (winapis().NtWaitForSingleObject)(Handle, Alertable, Timeout) }
}

/// Wrapper for the `NtCreateEvent` API.
#[inline]
pub fn NtCreateEvent(
    EventHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut c_void,
    EventType: EVENT_TYPE,
    InitialState: u8,
) -> NTSTATUS {
    unsafe { 
        (winapis().NtCreateEvent)(
            EventHandle, 
            DesiredAccess, 
            ObjectAttributes, 
            EventType, 
            InitialState
        ) 
    }
}

/// Wrapper for the `NtDuplicateObject` API.
#[inline]
pub fn NtDuplicateObject(
    SourceProcessHandle: HANDLE,
    SourceHandle: HANDLE,
    TargetProcessHandle: HANDLE,
    TargetHandle: *mut HANDLE,
    DesiredAccess: u32,
    HandleAttributes: u32,
    Options: u32,
) -> NTSTATUS {
    unsafe {
        (winapis().NtDuplicateObject)(
            SourceProcessHandle,
            SourceHandle,
            TargetProcessHandle,
            TargetHandle,
            DesiredAccess,
            HandleAttributes,
            Options,
        )
    }
}

/// Wrapper for the `NtLockVirtualMemory` API.
#[inline]
pub fn NtLockVirtualMemory(
    ProcessHandle: HANDLE, 
    BaseAddress: *mut *mut c_void, 
    RegionSize: *mut usize, 
    MapType: u32
) -> NTSTATUS {
    unsafe { 
        (winapis().NtLockVirtualMemory)(
            ProcessHandle, 
            BaseAddress, 
            RegionSize, 
            MapType
        ) 
    }
}

/// Wrapper for the `NtAllocateVirtualMemory` API.
pub fn NtAllocateVirtualMemory(
    ProcessHandle: HANDLE,
    BaseAddress: *mut *mut c_void,
    ZeroBits: usize,
    RegionSize: *mut usize,
    AllocationType: u32,
    Protect: u32,
) -> NTSTATUS {
    match syscall!(
        s!("NtAllocateVirtualMemory"),
        ProcessHandle,
        BaseAddress,
        ZeroBits,
        RegionSize,
        AllocationType,
        Protect
    ) {
        Ok(ret) => ret as NTSTATUS,
        Err(_) => STATUS_UNSUCCESSFUL,
    }
}

/// Wrapper for the `NtProtectVirtualMemory` API.
pub fn NtProtectVirtualMemory(
    ProcessHandle: *mut c_void,
    BaseAddress: *mut *mut c_void,
    RegionSize: *mut usize,
    NewProtect: u32,
    OldProtect: *mut u32,
) -> NTSTATUS {
    match syscall!(
        s!("NtProtectVirtualMemory"), 
        ProcessHandle, 
        BaseAddress, 
        RegionSize, 
        NewProtect, 
        OldProtect
    ) {
        Ok(ret) => ret as NTSTATUS,
        Err(_) => STATUS_UNSUCCESSFUL,
    }
}

/// Wrapper for the `NtQueryInformationProcess` API.
#[inline]
pub fn NtQueryInformationProcess(
    ProcessHandle: HANDLE,
    ProcessInformationClass: u32,
    ProcessInformation: *mut c_void,
    ProcessInformationLength: u32,
    ReturnLength: *mut u32,
) -> NTSTATUS {
    unsafe {
        (winapis().NtQueryInformationProcess)(
            ProcessHandle, 
            ProcessInformationClass, 
            ProcessInformation, 
            ProcessInformationLength, 
            ReturnLength
        )
    }
}

/// Wrapper for the `NtAlertResumeThread` API.
#[inline]
pub fn NtAlertResumeThread(ThreadHandle: HANDLE, PreviousSuspendCount: *mut u32) -> NTSTATUS {
    unsafe { (winapis().NtAlertResumeThread)(ThreadHandle, PreviousSuspendCount) }
}

/// Wrapper for the `NtQueueApcThread` API.
#[inline]
pub fn NtQueueApcThread(
    ThreadHandle: HANDLE,
    ApcRoutine: *mut c_void,
    ApcArgument1: *mut c_void,
    ApcArgument2: *mut c_void,
    ApcArgument3: *mut c_void,
) -> NTSTATUS {
    unsafe { 
        (winapis().NtQueueApcThread)(
            ThreadHandle, 
            ApcRoutine, 
            ApcArgument1, 
            ApcArgument2, 
            ApcArgument3
        ) 
    }
}

/// Wrapper for the `NtSignalAndWaitForSingleObject` API.
#[inline]
pub fn NtSignalAndWaitForSingleObject(
    SignalHandle: HANDLE, 
    WaitHandle: HANDLE, 
    Alertable: u8, 
    Timeout: *mut LARGE_INTEGER
) -> NTSTATUS {
    unsafe { 
        (winapis().NtSignalAndWaitForSingleObject)(
            SignalHandle, 
            WaitHandle, 
            Alertable, 
            Timeout
        ) 
    }
}

/// Wrapper for the `TpAllocPool` API.
#[inline]
pub fn TpAllocPool(PoolReturn: *mut *mut c_void, Reserved: *mut c_void) -> NTSTATUS {
    unsafe { (winapis().TpAllocPool)(PoolReturn, Reserved) }
}

/// Wrapper for the `TpSetPoolStackInformation` API.
#[inline]
pub fn TpSetPoolStackInformation(
    Pool: *mut c_void, 
    PoolStackInformation: *mut TP_POOL_STACK_INFORMATION
) -> NTSTATUS {
    unsafe { (winapis().TpSetPoolStackInformation)(Pool, PoolStackInformation) }
}

/// Wrapper for the `TpSetPoolMinThreads` API.
#[inline]
pub fn TpSetPoolMinThreads(Pool: *mut c_void, MinThreads: u32) -> NTSTATUS {
    unsafe { (winapis().TpSetPoolMinThreads)(Pool, MinThreads) }
}

/// Wrapper for the `TpSetPoolMaxThreads` API.
#[inline]
pub fn TpSetPoolMaxThreads(Pool: *mut c_void, MaxThreads: u32) {
    unsafe { (winapis().TpSetPoolMaxThreads)(Pool, MaxThreads) }
}

/// Wrapper for the `TpAllocTimer` API.
/// 
/// 
#[inline]
pub fn TpAllocTimer(
    Timer: *mut *mut c_void, 
    Callback: *mut c_void, 
    Context: *mut c_void, 
    CallbackEnviron: *mut TP_CALLBACK_ENVIRON_V3
) -> NTSTATUS {
    unsafe { (winapis().TpAllocTimer)(Timer, Callback, Context, CallbackEnviron) }
}

/// Wrapper for the `TpSetTimer` API.
#[inline]
pub fn TpSetTimer(
    Timer: *mut c_void, 
    DueTime: *mut LARGE_INTEGER, 
    Period: u32, 
    WindowLength: u32
) {
    unsafe { 
        (winapis().TpSetTimer)(Timer, DueTime, Period, WindowLength) 
    }
}

/// Wrapper for the `TpAllocWait` API.
#[inline]
pub fn TpAllocWait(
    WaitReturn: *mut *mut c_void,
    Callback: *mut c_void,
    Context: *mut c_void,
    CallbackEnviron: *mut TP_CALLBACK_ENVIRON_V3,
) -> NTSTATUS {
    unsafe { (winapis().TpAllocWait)(WaitReturn, Callback, Context, CallbackEnviron) }
}

/// Wrapper for the `TpSetWait` API.
#[inline]
pub fn TpSetWait(Wait: *mut c_void, Handle: *mut c_void, Timeout: *mut LARGE_INTEGER) {
    unsafe { (winapis().TpSetWait)(Wait, Handle, Timeout) }
}

/// Wrapper for the `CloseThreadpool` API.
#[inline]
pub fn CloseThreadpool(Pool: *mut c_void) -> NTSTATUS {
    unsafe { (winapis().CloseThreadpool)(Pool) }
}

/// Wrapper for the `RtlWalkHeap` API.
#[inline]
pub fn RtlWalkHeap(HeapHandle: *mut c_void, Entry: *mut RTL_HEAP_WALK_ENTRY) -> NTSTATUS {
    unsafe { (winapis().RtlWalkHeap)(HeapHandle, Entry) }
}

/// Wrapper for the `SetProcessValidCallTargets` API.
#[inline]
pub fn SetProcessValidCallTargets(
    hProcess: HANDLE,
    VirtualAddress: *mut c_void,
    RegionSize: usize,
    NumberOfOffsets: u32,
    OffsetInformation: *mut CFG_CALL_TARGET_INFO,
) -> u8 {
    unsafe { 
        (winapis().SetProcessValidCallTargets)(
            hProcess, 
            VirtualAddress, 
            RegionSize, 
            NumberOfOffsets, 
            OffsetInformation
        ) 
    }
}

/// Wrapper for the `ConvertFiberToThread` API.
#[inline]
pub fn ConvertFiberToThread() -> i32 {
    unsafe { (winapis().ConvertFiberToThread)() }
}

/// Wrapper for the `ConvertThreadToFiber` API.
#[inline]
pub fn ConvertThreadToFiber(lpParameter: *mut c_void) -> *mut c_void {
    unsafe { (winapis().ConvertThreadToFiber)(lpParameter) }
}

/// Wrapper for the `CreateFiber` API.
#[inline]
pub fn CreateFiber(
    dwStackSize: usize, 
    lpStartAddress: LPFIBER_START_ROUTINE, 
    lpParameter: *const c_void
) -> *mut c_void {
    unsafe { (winapis().CreateFiber)(dwStackSize, lpStartAddress, lpParameter) }
}

/// Wrapper for the `DeleteFiber` API.
#[inline]
pub fn DeleteFiber(lpFiber: *mut c_void) {
    unsafe { (winapis().DeleteFiber)(lpFiber) }
}

/// Wrapper for the `SwitchToFiber` API.
#[inline]
pub fn SwitchToFiber(lpFiber: *mut c_void) {
    unsafe { (winapis().SwitchToFiber)(lpFiber) }
}

/// Lightweight wrapper for NtSetEvent,used in a thread callback context
/// 
/// 本质是一个rust的函数,但遵循win64 c语言调用约定;
/// 
/// 以hypnus.rs中status=TpAllocTimer( &mut timer_event,NtSetEvent2 as *mut c_void,events[0],&mut env);为例
/// 
/// NtSetEvent2  被当作回调函数，它必须严格符合  PTP_WAIT_CALLBACK（一个函数指针类型定义）的签名规范。在初始化阶段，我们将  NtSetEvent2及其目标事件句柄（作为 Context）传递给  TpAllocWait进行登记。在定时器到期时，操作系统底层的线程池会自动执行它，而它会在执行时接住系统传来的4 个参数，丢弃无用项，精准提取出 Context 中的句柄，进而触发真正的NtSetEvent
/// 
/// PTP_WAIT_CALLBACK定义四个参数.而NtSetEvent只有两个参数.所以这里wrapper了一下
pub extern "C" fn NtSetEvent2(_: *mut c_void, event: *mut c_void, _: *mut c_void, _: u32) {
    NtSetEvent(event, null_mut());
}