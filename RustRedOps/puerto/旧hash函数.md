// 传入名称的hash算法,及test代码
// fn main(){

//   pub fn fnv1a(string: &str) -> u32 {

//     /// 需要修改的种子
//     const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7;

//     /// 需要修改的素数(避免出现hash撞库)
//     const FNV_PRIME: u32 = 0xAD37_79B9;

//     let mut hash = FNV_OFFSET_BASIS;
//     for &byte in string.as_bytes() {
//         hash ^= byte as u32;
//         hash = hash.wrapping_mul(FNV_PRIME);
//     }

//     hash
// }

// println!("{}",fnv1a("liabry"))
// }

// #[cfg(test)]
// mod tests {
//     use std::collections::HashMap;

//     // Modified FNV1a algorithm as requested
//     // This uses a custom offset basis to avoid signature detection
//     fn fnv1a_modified(string: &str) -> u32 {
//         const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7;
//         const FNV_PRIME: u32 = 0xAD37_79B9; // Custom random odd constant to avoid signatures

//         let mut hash = FNV_OFFSET_BASIS;
//         for &byte in string.as_bytes() {
//             hash ^= byte as u32;
//             hash = hash.wrapping_mul(FNV_PRIME);
//         }
//         hash
//     }

//     #[test]
//     fn test_hash_collision() {
//         // 1. Collect all APIs and module names involved in the project
//         // List source: Based on src/winapis.rs, src/module.rs, README.md etc.
//         let api_list = vec![
//             // Module names (Case sensitivity depends on usage, testing both variants recommended)
//             "KERNEL32.DLL", "ntdll.dll", "advapi32.dll",
//             "kernel32.dll", "NTDLL.DLL", "ADVAPI32.DLL",

//             // APIs explicitly used in the project
//             "NtAllocateVirtualMemory",
//             "NtProtectVirtualMemory",
//             "NtCreateThreadEx",
//             "NtWriteVirtualMemory",
//             "NtOpenProcess",
//             "NtQueryInformationProcess", // Appears in macro comments
//             "LoadLibraryA",
//             "VirtualAlloc",

//             // Forwarding tests APIs (from module.rs tests)
//             "SetIoRingCompletionEvent",
//             "SetProtectedPolicy",
//             "SetProcessDefaultCpuSetMasks",
//             "SetDefaultDllDirectories",
//             "SetProcessDefaultCpuSets",
//             "InitializeProcThreadAttributeList",
//             "SystemFunction028",
//             "PerfIncrementULongCounterValue",
//             "PerfSetCounterRefValue",
//             "I_QueryTagInformation",
//             "TraceQueryInformation",
//             "TraceMessage",

//             // Common Red Team / Shellcode APIs (for safety)
//             "GetProcAddress",
//             "GetModuleHandleA",
//             "ExitProcess",
//             "WaitForSingleObject",
//             "CreateFileW",
//             "ReadFile",
//             "WriteFile",
//             "VirtualFree",
//             "VirtualProtect",
//             "CreateRemoteThread",
//             "OpenProcess",
//             "RtlMoveMemory",
//             "RtlZeroMemory",
//         ];

//         let mut hashes = HashMap::new();
//         let mut collisions = Vec::new();

//         println!("\n[+] Starting Hash Collision Test for FNV1a_Modified...");
//         println!("--------------------------------------------------");

//         for api in api_list {
//             let hash = fnv1a_modified(api);

//             // Check if hash already exists
//             if let Some(existing_api) = hashes.get(&hash) {
//                 collisions.push((api, *existing_api, hash));
//             } else {
//                 hashes.insert(hash, api);
//                 // Print Hash value for easy copying
//                 println!("API: {:<35} | 十六进制Hash: 0x{:08X} (十进制:{})", api, hash, hash);
//             }
//         }

//         println!("--------------------------------------------------");

//         if !collisions.is_empty() {
//             println!("\n[!] ⚠️ FATAL ERROR: Hash Collisions Detected!");
//             for (api1, api2, hash) in collisions {
//                 println!("    Collision: '{}' and '{}' both hash to 0x{:08X}", api1, api2, hash);
//             }
//             panic!("Hash algorithm modification failed due to collisions.");
//         } else {
//             println!("\n[+] ✅ Success: No collisions detected in {} APIs.", hashes.len());
//         }
//     }
// }