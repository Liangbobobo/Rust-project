//! Pre-computed API & Module Hashes using FNV-1a (puerto compatible)

// ============================================================================
// 1. Modules (系统 DLL 模块哈希)
// ============================================================================
pub const HASH_KERNEL32_DLL: u32     = 0x6BEFCBB7;
pub const HASH_NTDLL_DLL: u32        = 0xB3383153;
pub const HASH_KERNELBASE_DLL: u32   = 0x31B113C3;
pub const HASH_ADVAPI32_DLL: u32     = 0x37019FB7;
pub const HASH_CRYPTBASE_DLL: u32    = 0xF6316394;
pub const HASH_CRYPTBASE: u32        = 0x444D6CFE;

// ============================================================================
// 2. Stack Spoofing Core APIs (栈伪造核心起点)
// ============================================================================
pub const HASH_RTL_USER_THREAD_START: u32  = 0x72B24572;
pub const HASH_BASE_THREAD_INIT_THUNK: u32 = 0xF70757EA;

// ============================================================================
// 3. Dynamic Syscalls & Native APIs (底层系统调用)
// ============================================================================
pub const HASH_NT_ALLOCATE_VIRTUAL_MEMORY: u32               = 0x803BA0E0;
pub const HASH_NT_PROTECT_VIRTUAL_MEMORY: u32                = 0x17D18FF6;
pub const HASH_NT_CREATE_THREAD_EX: u32                      = 0xC5C9DC2A;
pub const HASH_NT_WRITE_VIRTUAL_MEMORY: u32                  = 0x4ABFD310;
pub const HASH_NT_OPEN_PROCESS: u32                          = 0xED482F32;
pub const HASH_NT_QUERY_INFORMATION_PROCESS: u32             = 0xC43D7E80;
pub const HASH_NT_GET_CONTEXT_THREAD: u32                    = 0x0FFA8E6A;
pub const HASH_NT_SET_CONTEXT_THREAD: u32                    = 0xAD4FA23E;
pub const HASH_NT_SIGNAL_AND_WAIT_FOR_SINGLE_OBJECT: u32     = 0x3C139717;
pub const HASH_NT_QUEUE_APC_THREAD: u32                      = 0x75C2EBF0;
pub const HASH_NT_ALERT_RESUME_THREAD: u32                   = 0x56B9A9A4;
pub const HASH_NT_LOCK_VIRTUAL_MEMORY: u32                   = 0x2DCBE5E6;
pub const HASH_NT_DUPLICATE_OBJECT: u32                      = 0x89D8E19F;
pub const HASH_NT_CREATE_EVENT: u32                          = 0xE7CD1155;
pub const HASH_NT_WAIT_FOR_SINGLE_OBJECT: u32                = 0x3C8521C2;
pub const HASH_NT_CLOSE: u32                                 = 0x83C4DABB;
pub const HASH_NT_SET_EVENT: u32                             = 0x89EFA2B3;
pub const HASH_NT_CONTINUE: u32                              = 0x79CC20CC;
pub const HASH_NT_TEST_ALERT: u32                            = 0x632C9CB5;
pub const HASH_RTL_EXIT_USER_THREAD: u32                     = 0x58E1EAB2;
pub const HASH_ZW_WAIT_FOR_WORK_VIA_WORKER_FACTORY: u32     = 0x915CE667;

// ============================================================================
// 4. ThreadPool (TP) APIs (线程池相关)
// ============================================================================
pub const HASH_TP_ALLOC_POOL: u32                           = 0xCD8EE2C2;
pub const HASH_TP_SET_POOL_STACK_INFORMATION: u32           = 0x2AB0519F;
pub const HASH_TP_SET_POOL_MIN_THREADS: u32                 = 0xC407247A;
pub const HASH_TP_SET_POOL_MAX_THREADS: u32                 = 0x1449B364;
pub const HASH_TP_ALLOC_TIMER: u32                          = 0x8415A9F9;
pub const HASH_TP_SET_TIMER: u32                            = 0x41FB11F6;
pub const HASH_TP_ALLOC_WAIT: u32                           = 0x9F7F7CB5;
pub const HASH_TP_SET_WAIT: u32                             = 0xD7F339CA;
pub const HASH_TP_RELEASE_CLEANUP_GROUP_MEMBERS: u32        = 0x54B62B66;
pub const HASH_CLOSE_THREADPOOL: u32                        = 0x48D98313;

// ============================================================================
// 5. Memory & Fiber & System Helper APIs (内存/纤程辅助函数)
// ============================================================================
pub const HASH_RTL_WALK_HEAP: u32                           = 0xFB6908F0;
pub const HASH_RTL_CAPTURE_CONTEXT: u32                     = 0x5BDA3146;
pub const HASH_RTL_ACQUIRE_SRW_LOCK_EXCLUSIVE: u32          = 0x06462CF6;
pub const HASH_SET_PROCESS_VALID_CALL_TARGETS: u32          = 0x815DE4D8;
pub const HASH_CONVERT_FIBER_TO_THREAD: u32                 = 0x3A0DEF5F;
pub const HASH_CONVERT_THREAD_TO_FIBER: u32                 = 0x08CDC22F;
pub const HASH_CREATE_FIBER: u32                            = 0x74A55DD9;
pub const HASH_DELETE_FIBER: u32                            = 0x7DBAA104;
pub const HASH_SWITCH_TO_FIBER: u32                         = 0x564CD584;
pub const HASH_SYSTEM_FUNCTION_040: u32                     = 0xFD7E7BD4;
pub const HASH_SYSTEM_FUNCTION_041: u32                     = 0x8EDCE385;
pub const HASH_ENUM_DATE_FORMATS_A: u32                     = 0x7717AF01;

// ============================================================================
// 6. Exception Handling & Win32 APIs (通用 Win32 API)
// ============================================================================
pub const HASH_ADD_VECTORED_EXCEPTION_HANDLER: u32          = 0x86429FB1;
pub const HASH_REMOVE_VECTORED_EXCEPTION_HANDLER: u32       = 0xED2A1F66;
pub const HASH_LOAD_LIBRARY_A: u32                          = 0x4CE67FBF;
pub const HASH_VIRTUAL_ALLOC: u32                           = 0x63E4D69B;
pub const HASH_GET_PROCESS_HEAP: u32                        = 0x4E861A86;
pub const HASH_GET_STD_HANDLE: u32                          = 0x5E6A00C8;
pub const HASH_GET_PROC_ADDRESS: u32                        = 0x5EF0E069;
pub const HASH_GET_MODULE_HANDLE_A: u32                     = 0x74BD07C0;
pub const HASH_EXIT_PROCESS: u32                            = 0xDEC8009C;
pub const HASH_WAIT_FOR_SINGLE_OBJECT: u32                  = 0x1C442098;
pub const HASH_CREATE_FILE_W: u32                           = 0xDA6054C2;
pub const HASH_READ_FILE: u32                               = 0x219168D3;
pub const HASH_WRITE_FILE: u32                              = 0x9C01094C;
pub const HASH_VIRTUAL_FREE: u32                            = 0xE028A812;
pub const HASH_VIRTUAL_PROTECT: u32                         = 0x03D3511D;
pub const HASH_CREATE_REMOTE_THREAD: u32                    = 0x53E210B9;
pub const HASH_OPEN_PROCESS: u32                            = 0x87967048;
pub const HASH_RTL_MOVE_MEMORY: u32                         = 0x6A5B898D;
pub const HASH_RTL_ZERO_MEMORY: u32                         = 0xD4782C9E;

// ============================================================================
// 7. Forwarded Exports (转发导出表测试 API)
// ============================================================================
pub const HASH_SET_IO_RING_COMPLETION_EVENT: u32            = 0xB01A7639;
pub const HASH_SET_PROTECTED_POLICY: u32                    = 0x50D8DA3F;
pub const HASH_SET_PROCESS_DEFAULT_CPU_SET_MASKS: u32       = 0x85F1DB36;
pub const HASH_SET_DEFAULT_DLL_DIRECTORIES: u32             = 0x180FE675;
pub const HASH_SET_PROCESS_DEFAULT_CPU_SETS: u32            = 0x1920E702;
pub const HASH_INITIALIZE_PROC_THREAD_ATTRIBUTE_LIST: u32   = 0x5443D271;
pub const HASH_SYSTEM_FUNCTION_028: u32                     = 0x3C13D4DA;
pub const HASH_PERF_INCREMENT_U_LONG_COUNTER_VALUE: u32     = 0x2B4982D7;
pub const HASH_PERF_SET_COUNTER_REF_VALUE: u32              = 0xB904EFFA;
pub const HASH_I_QUERY_TAG_INFORMATION: u32                 = 0xE4F1FD65;
pub const HASH_TRACE_QUERY_INFORMATION: u32                 = 0xB9945020;
pub const HASH_TRACE_MESSAGE: u32                           = 0xF60D376D;