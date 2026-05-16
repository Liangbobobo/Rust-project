#![allow(unused)]

// 本文件(mod)在项目中作用:

// 问题:

// 需注意:

use core::{ffi::c_void,mem::transmute,ptr::null_mut};
use uwd::syscall;
use crate::{debug_log,stealth_bail};
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use puerto::types::{EVENT_TYPE,HANDLE,LARGE_INTEGER,NTSTATUS,STATUS_UNSUCCESSFUL};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};

use crate::types::*;

/// Structure containing all function pointers resolved only once.
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