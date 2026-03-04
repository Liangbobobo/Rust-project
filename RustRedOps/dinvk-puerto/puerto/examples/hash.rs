//  use std::collections::HashMap;

/// 直接传入&[u16]给hash函数
/// 
/// 该函数实现了 FNV-1a 哈希算法，专门用于处理 UTF-16 编码的字符串（Windows API 默认格式）。
/// 它包含了一个“原地大小写转换”的技巧，可以在计算哈希时忽略大小写，且不产生任何内存分配。
pub fn fnv1a_utf16(data: &[u16]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7; // 自定义种子
    const FNV_PRIME: u32 = 0xAD37_79B9;        // 自定义素数

    let mut hash = FNV_OFFSET_BASIS;

    for &val in data {
        // 免杀技巧：在哈希过程中直接将 a-z 转为 A-Z (Case Folding)
        // 这样不需要调用 .to_uppercase()，也就没有了内存分配
        let chr = if val >= 97 && val <= 122 { val - 32 } else { val };

        // 将 u16 拆分为两个字节进行哈希处理
        // FNV算法通常是按字节工作的
        let bytes = [
            (chr & 0xFF) as u8,
            (chr >> 8) as u8,
        ];

        for &byte in &bytes {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn main() {
    let example_api = "NtAllocateVirtualMemory";
    // 将字符串转换为 UTF-16 向量
    let utf16_api: Vec<u16> = example_api.encode_utf16().collect();
    
    let hash = fnv1a_utf16(&utf16_api);
    println!("API: {} -> Hash: 0x{:08X}", example_api, hash);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_hash_collision() {
        // 1. 收集项目中涉及的所有 API 和模块名称
        // 列表来源：基于 src/winapis.rs, src/module.rs, README.md 等
        let api_list = vec![
            // 模块名称 (大小写敏感性取决于用法，建议测试两种变体)
            // 注意：fnv1a_utf16 内部已经做了转大写处理，所以理论上 "ntdll.dll" 和 "NTDLL.DLL" 哈希值应该一样
            "KERNEL32.DLL", "ntdll.dll", "advapi32.dll",
            "kernel32.dll", "NTDLL.DLL", "ADVAPI32.DLL",

            // 项目中显式使用的 API
            "NtAllocateVirtualMemory",
            "NtProtectVirtualMemory",
            "NtCreateThreadEx",
            "NtWriteVirtualMemory",
            "NtOpenProcess",
            "NtQueryInformationProcess", // 出现在宏注释中
            "LoadLibraryA",
            "VirtualAlloc",
            "GetProcessHeap",
            "NtGetContextThread",
            "NtSetContextThread",
            "AddVectoredExceptionHandler",
            "RemoveVectoredExceptionHandler",
            "GetStdHandle",

            // 转发测试 API (来自 module.rs 测试)
            "SetIoRingCompletionEvent",
            "SetProtectedPolicy",
            "SetProcessDefaultCpuSetMasks",
            "SetDefaultDllDirectories",
            "SetProcessDefaultCpuSets",
            "InitializeProcThreadAttributeList",
            "SystemFunction028",
            "PerfIncrementULongCounterValue",
            "PerfSetCounterRefValue",
            "I_QueryTagInformation",
            "TraceQueryInformation",
            "TraceMessage",

            // 常见的红队 / Shellcode API (为了安全起见)
            "GetProcAddress",
            "GetModuleHandleA",
            "ExitProcess",
            "WaitForSingleObject",
            "CreateFileW",
            "ReadFile",
            "WriteFile",
            "VirtualFree",
            "VirtualProtect",
            "CreateRemoteThread",
            "OpenProcess",
            "RtlMoveMemory",
            "RtlZeroMemory",
        ];

        let mut hashes: HashMap<u32, &str> = HashMap::new();
        let mut collisions = Vec::new();

        println!("\n[+] Starting Hash Collision Test for FNV1a_UTF16 (Modified) ...");
        println!("--------------------------------------------------");

        for api in api_list {
            // 关键修改：将字符串转为 UTF-16 数组 [u16]
            let api_utf16: Vec<u16> = api.encode_utf16().collect();
            let hash = fnv1a_utf16(&api_utf16);

            // 检查哈希是否已存在
            // 注意：我们允许 "ntdll.dll" 和 "NTDLL.DLL" 产生相同的哈希（因为算法内部忽略大小写）
            // 只有当两个 *不同* 的字符串（忽略大小写后仍然不同）产生相同哈希时，才算真正的碰撞
            if let Some(existing_api) = hashes.get(&hash) {
                // 如果现有的 API 和当前的 API 在忽略大小写后不相等，则视为碰撞
                if !existing_api.eq_ignore_ascii_case(api) {
                    collisions.push((api, *existing_api, hash));
                } else {
                     // 这是一个预期的“碰撞”（因为算法忽略大小写），我们不需要报错，但可以记录一下
                     // println!("    [Info] equivalent strings: '{}' == '{}' -> 0x{:08X}", api, existing_api, hash);
                }
            } else {
                hashes.insert(hash, api);
                // 打印哈希值方便复制
                println!("API: {:<35} | 十六进制Hash: 0x{:08X} (十进制:{})", api, hash, hash);
            }
        }

        println!("--------------------------------------------------");

        if !collisions.is_empty() {
            println!("\n[!] ⚠️ FATAL ERROR: Hash Collisions Detected!");
            for (api1, api2, hash) in collisions {
                println!("    Collision: '{}' and '{}' both hash to 0x{:08X}", api1, api2, hash);
            }
            panic!("Hash algorithm modification failed due to collisions.");
        } else {
            println!("\n[+] ✅ Success: No collisions detected in {} APIs.", hashes.len());
        }
    }
}