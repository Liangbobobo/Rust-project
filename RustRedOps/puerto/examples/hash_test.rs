// 这是一个独立的 Example 文件，用于测试哈希计算和碰撞检测
// 它不依赖 src/ 下的其他代码，因此可以忽略其他模块的编译错误

// --- Core Hash Function Implementation ---
// 复制 src/hash.rs 的核心逻辑到这里

// 在 RustRedOps\puerto 路径下 执行
// rustc examples/hash_test.rs -o hash_test.exe; ./hash_test.exe

/// 直接传入&[u16]给hash函数
pub fn fnv1a_utf16(data: &[u16]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7; // 自定义种子
    const FNV_PRIME: u32 = 0xAD37_79B9;        // 自定义素数

    let mut hash = FNV_OFFSET_BASIS;

    for &val in data {
        // 免杀技巧：在哈希过程中直接将 a-z 转为 A-Z (Case Folding)
        // 这样不需要调用 .to_uppercase()，也就没有了内存分配
        let chr = if val >= 97 && val <= 122 { val - 32 } else { val };

        // 将 u16 拆分为两个字节进行哈希处理
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

/// 针对 &[u8] 的 fnv1a 哈希，模拟将其视为 UTF-16 字节流进行哈希
/// 这在处理 ASCII 字符串（如转发字符串中的模块名）并与 PEB 中的 UTF-16 哈希对比时非常有用
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

// --- Test Logic ---
// 直接在 main 函数中运行测试，不需要 #[cfg(test)]

use std::collections::HashMap;
use std::vec::Vec;

fn main() {
    // List of APIs and Modules to test
    let api_list = vec![
        // Modules
        "KERNEL32.DLL", "ntdll.dll", "advapi32.dll",
        "kernel32.dll", "NTDLL.DLL", "ADVAPI32.DLL",

        // Core APIs
        "NtAllocateVirtualMemory",
        "NtProtectVirtualMemory",
        "NtCreateThreadEx",
        "NtWriteVirtualMemory",
        "NtOpenProcess",
        "NtQueryInformationProcess",
        "NtGetContextThread",
        "NtSetContextThread",
        
        // Exception Handling
        "AddVectoredExceptionHandler",
        "RemoveVectoredExceptionHandler",

        // Standard Windows APIs
        "LoadLibraryA",
        "VirtualAlloc",
        "GetProcessHeap",
        "GetStdHandle",
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

        // Forwarded Exports (for module resolution tests)
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
    ];

    let mut hashes: HashMap<u32, &str> = HashMap::new();
    let mut collisions: Vec<(&str, &str, u32)> = Vec::new();

    println!("\n[+] Starting Hash Calculation & Collision Check (Standalone Example)...");
    println!("--------------------------------------------------");

    for api in api_list {
        // Convert to UTF-16 for the hash function
        let api_utf16: Vec<u16> = api.encode_utf16().collect();
        let hash = fnv1a_utf16(&api_utf16);

        // Print the calculated hash
        println!("API: {:<35} -> Hash: 0x{:08X}", api, hash);

        // Check for collisions
        if let Some(existing_api) = hashes.get(&hash) {
            // If hashes match, check if the source strings are actually different (ignoring case)
            // The hash function is case-insensitive, so "ntdll.dll" == "NTDLL.DLL" is NOT a collision.
            if !existing_api.eq_ignore_ascii_case(api) {
                collisions.push((api, *existing_api, hash));
            }
        } else {
            hashes.insert(hash, api);
        }
    }

    println!("--------------------------------------------------");

    if !collisions.is_empty() {
        println!("\n[!] COLLISIONS DETECTED:");
        for (api1, api2, hash) in &collisions {
            println!("    0x{:08X} -> '{}' matches '{}'", hash, api1, api2);
        }
        // Force exit with error code if collisions found
        std::process::exit(1);
    } else {
        println!("\n[+] SUCCESS: No hash collisions detected among {} unique hashes.", hashes.len());
    }
}
