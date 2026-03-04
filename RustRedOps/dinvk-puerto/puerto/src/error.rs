

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    // 只需初始化第一个字段,其余会自己递增
    Success = 0,
    Fail,
    HashFuncNotFound,
    ModuleNameIsEmpty,
    ModuleAddrIsEmpty,
    FuncAddNotFound,
    SymbolNotFound,
    HashFunctionMissing,
    SyscallResolutionFailed,
    ApiSetResolutionFailed,
    ForwardingFailed,
    InvalidPeFormat,
}

// impl From<DinvkError> for crate::types::NTSTATUS {
//     fn from(err: DinvkError) -> Self {
//         match err {
//             DinvkError::Success => 0,
//             DinvkError::ModuleNotFound => 0xC0000135u32 as i32,    // STATUS_DLL_NOT_FOUND
//             DinvkError::SymbolNotFound => 0xC0000139u32 as i32,    // STATUS_ENTRYPOINT_NOT_FOUND
//             _ => 0xC0000001u32 as i32,                            // STATUS_UNSUCCESSFUL
//         }
//     }
// }