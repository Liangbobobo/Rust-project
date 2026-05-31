// 此工具用于生成 samoa 重构时，config.rs 中所需的 API 和模块的哈希值。
// 算法严格遵循 puerto 的跨编码哈希对齐理念 (fnv1a_utf16_from_u8)

// 拷贝自 puerto/src/hash.rs 的哈希算法，以确保独立运行
pub fn fnv1a_utf16_from_u8(data: &[u8]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7;
    const FNV_PRIME: u32 = 0xAD37_79B9;

    let mut hash = FNV_OFFSET_BASIS;

    for &val in data {
        // Case Folding: a-z -> A-Z
        let chr = if val >= 97 && val <= 122 { val - 32 } else { val };

        // 模拟 UTF-16: 第一个字节是 ASCII，第二个字节是 0
        let bytes = [chr, 0u8];

        for &byte in &bytes {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn main() {
    let api_list = vec![
        "WaitForSingleObject",
        "BaseThreadInitThunk",
        "EnumSystemLocalesA", // 对应原始的 enum_date (即 EnumSystemLocalesA / EnumDateFormatsA 系列)
        "SystemFunction040",
        "SystemFunction041",
        "NtContinue",
        "NtSetEvent",
        "RtlUserThreadStart",
        "NtProtectVirtualMemory",
        "RtlExitUserThread",
        "NtSetContextThread",
        "NtGetContextThread",
        "NtTestAlert",
        "NtWaitForSingleObject",
        "RtlAcquireSRWLockExclusive", // 对应原始的 rtl_acquire_lock
        "TpReleaseCleanupGroupMembers", // 对应原始的 tp_release_cleanup
        "RtlCaptureContext",
        "ZwWaitForWorkViaWorkerFactory", // 对应原始的 zw_wait_for_worker
        "LoadLibraryA", // 动态加载 CryptBase 时依赖的函数
    ];

    println!("[+] 开始计算 Samoa (config.rs) 依赖的 API 哈希值 (fnv1a_utf16_from_u8):");
    println!("==========================================================================");

    for api in api_list {
        let hash = fnv1a_utf16_from_u8(api.as_bytes());
        println!("{:<30} -> {:<12} (0x{:08X})", api, format!("{}u32", hash), hash);
    }
    
    println!("==========================================================================\n");

    let modules = vec![
        "ntdll", 
        "kernel32", 
        "kernelbase", 
        "cryptbase"
    ];
    
    println!("[+] 模块名称哈希值计算:");
    println!("==========================================================================");
    for md in modules {
        let hash = fnv1a_utf16_from_u8(md.as_bytes());
        println!("{:<15} -> {:<12} (0x{:08X})", md, format!("{}u32", hash), hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modules_hashes() {
        assert_eq!(fnv1a_utf16_from_u8(b"ntdll"), 4143434745u32);
        assert_eq!(fnv1a_utf16_from_u8(b"kernel32"), 1303842461u32);
        assert_eq!(fnv1a_utf16_from_u8(b"kernelbase"), 3594687209u32);
        assert_eq!(fnv1a_utf16_from_u8(b"cryptbase"), 1145924862u32);
    }

    #[test]
    fn test_case_insensitivity() {
        // 算法应该对大小写不敏感
        assert_eq!(fnv1a_utf16_from_u8(b"NtDll"), fnv1a_utf16_from_u8(b"ntdll"));
        assert_eq!(fnv1a_utf16_from_u8(b"KERNEL32"), fnv1a_utf16_from_u8(b"kernel32"));
        assert_eq!(fnv1a_utf16_from_u8(b"NtProtectVirtualMemory"), fnv1a_utf16_from_u8(b"ntprotectvirtualmemory"));
    }

    #[test]
    fn test_apis_hashes() {
        assert_eq!(fnv1a_utf16_from_u8(b"WaitForSingleObject"), 474226840u32);
        assert_eq!(fnv1a_utf16_from_u8(b"BaseThreadInitThunk"), 4144453610u32);
        assert_eq!(fnv1a_utf16_from_u8(b"EnumSystemLocalesA"), 2305293355u32);
        assert_eq!(fnv1a_utf16_from_u8(b"SystemFunction040"), 4252924884u32);
        assert_eq!(fnv1a_utf16_from_u8(b"SystemFunction041"), 2396840837u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtContinue"), 2043420876u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtSetEvent"), 2314183347u32);
        assert_eq!(fnv1a_utf16_from_u8(b"RtlUserThreadStart"), 1924285810u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtProtectVirtualMemory"), 399609846u32);
        assert_eq!(fnv1a_utf16_from_u8(b"RtlExitUserThread"), 1491200690u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtSetContextThread"), 2907677246u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtGetContextThread"), 268078698u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtTestAlert"), 1663868085u32);
        assert_eq!(fnv1a_utf16_from_u8(b"NtWaitForSingleObject"), 1015357890u32);
        assert_eq!(fnv1a_utf16_from_u8(b"RtlAcquireSRWLockExclusive"), 105262326u32);
        assert_eq!(fnv1a_utf16_from_u8(b"TpReleaseCleanupGroupMembers"), 1421224806u32);
        assert_eq!(fnv1a_utf16_from_u8(b"RtlCaptureContext"), 1541026118u32);
        assert_eq!(fnv1a_utf16_from_u8(b"ZwWaitForWorkViaWorkerFactory"), 2438784615u32);
        assert_eq!(fnv1a_utf16_from_u8(b"LoadLibraryA"), 1290174399u32);
    }
}
