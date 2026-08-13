// [+] Starting Hash Calculation & Collision Check (puerto / mariana / samoa)...

// // =========================================================================
// // 1. Modules (包含大小写，验证 fnv1a_utf16 大小写折叠特征)
// // =========================================================================
// API/Module: kernel32.dll                        -> Hash: 0x6BEFCBB7 (dec: 1810877367)
// API/Module: KERNEL32.DLL                        -> Hash: 0x6BEFCBB7 (dec: 1810877367)
// API/Module: ntdll.dll                           -> Hash: 0xB3383153 (dec: 3006804307)
// API/Module: NTDLL.DLL                           -> Hash: 0xB3383153 (dec: 3006804307)
// API/Module: kernelbase.dll                      -> Hash: 0x31B113C3 (dec: 833688515)
// API/Module: KERNELBASE.DLL                      -> Hash: 0x31B113C3 (dec: 833688515)
// API/Module: advapi32.dll                        -> Hash: 0x37019FB7 (dec: 922853303)
// API/Module: ADVAPI32.DLL                        -> Hash: 0x37019FB7 (dec: 922853303)
// API/Module: cryptbase.dll                       -> Hash: 0xF6316394 (dec: 4130431892)
// API/Module: CryptBase                           -> Hash: 0x444D6CFE (dec: 1145924862)
// API/Module: CRYPTBASE.DLL                       -> Hash: 0xF6316394 (dec: 4130431892)

// // =========================================================================
// // 2. Stack Spoofing Core APIs (uwd / mariana)
// // =========================================================================
// API/Module: RtlUserThreadStart                  -> Hash: 0x72B24572 (dec: 1924285810)
// API/Module: BaseThreadInitThunk                 -> Hash: 0xF70757EA (dec: 4144453610)

// // =========================================================================
// // 3. Dynamic Syscalls & Native APIs (uwd / mariana / samoa)
// // =========================================================================
// API/Module: NtAllocateVirtualMemory             -> Hash: 0x803BA0E0 (dec: 2151391456)
// API/Module: NtProtectVirtualMemory              -> Hash: 0x17D18FF6 (dec: 399609846)
// API/Module: NtCreateThreadEx                    -> Hash: 0xC5C9DC2A (dec: 3318340650)
// API/Module: NtWriteVirtualMemory                -> Hash: 0x4ABFD310 (dec: 1254085392)
// API/Module: NtOpenProcess                       -> Hash: 0xED482F32 (dec: 3980930866)
// API/Module: NtQueryInformationProcess           -> Hash: 0xC43D7E80 (dec: 3292364416)
// API/Module: NtGetContextThread                  -> Hash: 0x0FFA8E6A (dec: 268078698)
// API/Module: NtSetContextThread                  -> Hash: 0xAD4FA23E (dec: 2907677246)
// API/Module: NtSignalAndWaitForSingleObject      -> Hash: 0x3C139717 (dec: 1007916823)
// API/Module: NtQueueApcThread                    -> Hash: 0x75C2EBF0 (dec: 1975708656)
// API/Module: NtAlertResumeThread                 -> Hash: 0x56B9A9A4 (dec: 1455008164)
// API/Module: NtLockVirtualMemory                 -> Hash: 0x2DCBE5E6 (dec: 768337382)
// API/Module: NtDuplicateObject                   -> Hash: 0x89D8E19F (dec: 2312692127)
// API/Module: NtCreateEvent                       -> Hash: 0xE7CD1155 (dec: 3888976213)
// API/Module: NtWaitForSingleObject               -> Hash: 0x3C8521C2 (dec: 1015357890)
// API/Module: NtClose                             -> Hash: 0x83C4DABB (dec: 2210716347)
// API/Module: NtSetEvent                          -> Hash: 0x89EFA2B3 (dec: 2314183347)
// API/Module: NtContinue                          -> Hash: 0x79CC20CC (dec: 2043420876)
// API/Module: NtTestAlert                         -> Hash: 0x632C9CB5 (dec: 1663868085)
// API/Module: RtlExitUserThread                   -> Hash: 0x58E1EAB2 (dec: 1491200690)
// API/Module: ZwWaitForWorkViaWorkerFactory       -> Hash: 0x915CE667 (dec: 2438784615)

// // =========================================================================
// // 4. ThreadPool (TP) APIs (samoa / hypnus)
// // =========================================================================
// API/Module: TpAllocPool                         -> Hash: 0xCD8EE2C2 (dec: 3448693442)
// API/Module: TpSetPoolStackInformation           -> Hash: 0x2AB0519F (dec: 716198303)
// API/Module: TpSetPoolMinThreads                 -> Hash: 0xC407247A (dec: 3288802426)
// API/Module: TpSetPoolMaxThreads                 -> Hash: 0x1449B364 (dec: 340374372)
// API/Module: TpAllocTimer                        -> Hash: 0x8415A9F9 (dec: 2216012281)
// API/Module: TpSetTimer                          -> Hash: 0x41FB11F6 (dec: 1106973174)
// API/Module: TpAllocWait                         -> Hash: 0x9F7F7CB5 (dec: 2675932341)
// API/Module: TpSetWait                           -> Hash: 0xD7F339CA (dec: 3623041482)
// API/Module: TpReleaseCleanupGroupMembers        -> Hash: 0x54B62B66 (dec: 1421224806)
// API/Module: CloseThreadpool                     -> Hash: 0x48D98313 (dec: 1222214419)

// // =========================================================================
// // 5. Memory & Fiber & System Helper APIs (samoa / hypnus)
// // =========================================================================
// API/Module: RtlWalkHeap                         -> Hash: 0xFB6908F0 (dec: 4217964784)
// API/Module: RtlCaptureContext                   -> Hash: 0x5BDA3146 (dec: 1541026118)
// API/Module: RtlAcquireSRWLockExclusive          -> Hash: 0x06462CF6 (dec: 105262326)
// API/Module: SetProcessValidCallTargets          -> Hash: 0x815DE4D8 (dec: 2170414296)
// API/Module: ConvertFiberToThread                -> Hash: 0x3A0DEF5F (dec: 973991775)
// API/Module: ConvertThreadToFiber                -> Hash: 0x08CDC22F (dec: 147702319)
// API/Module: CreateFiber                         -> Hash: 0x74A55DD9 (dec: 1956994521)
// API/Module: DeleteFiber                         -> Hash: 0x7DBAA104 (dec: 2109382916)
// API/Module: SwitchToFiber                       -> Hash: 0x564CD584 (dec: 1447875972)
// API/Module: SystemFunction040                   -> Hash: 0xFD7E7BD4 (dec: 4252924884)
// API/Module: SystemFunction041                   -> Hash: 0x8EDCE385 (dec: 2396840837)
// API/Module: EnumDateFormatsA                    -> Hash: 0x7717AF01 (dec: 1998040833)

// // =========================================================================
// // 6. Exception Handling & Win32 APIs (puerto / dinvk)
// // =========================================================================
// API/Module: AddVectoredExceptionHandler         -> Hash: 0x86429FB1 (dec: 2252513201)
// API/Module: RemoveVectoredExceptionHandler      -> Hash: 0xED2A1F66 (dec: 3978960742)
// API/Module: LoadLibraryA                        -> Hash: 0x4CE67FBF (dec: 1290174399)
// API/Module: VirtualAlloc                        -> Hash: 0x63E4D69B (dec: 1675941531)
// API/Module: GetProcessHeap                      -> Hash: 0x4E861A86 (dec: 1317411462)
// API/Module: GetStdHandle                        -> Hash: 0x5E6A00C8 (dec: 1584005320)
// API/Module: GetProcAddress                      -> Hash: 0x5EF0E069 (dec: 1592844393)
// API/Module: GetModuleHandleA                    -> Hash: 0x74BD07C0 (dec: 1958545344)
// API/Module: ExitProcess                         -> Hash: 0xDEC8009C (dec: 3737649308)
// API/Module: WaitForSingleObject                 -> Hash: 0x1C442098 (dec: 474226840)
// API/Module: CreateFileW                         -> Hash: 0xDA6054C2 (dec: 3663746242)
// API/Module: ReadFile                            -> Hash: 0x219168D3 (dec: 563177683)
// API/Module: WriteFile                           -> Hash: 0x9C01094C (dec: 2617313612)
// API/Module: VirtualFree                         -> Hash: 0xE028A812 (dec: 3760760850)
// API/Module: VirtualProtect                      -> Hash: 0x03D3511D (dec: 64180509)
// API/Module: CreateRemoteThread                  -> Hash: 0x53E210B9 (dec: 1407324345)
// API/Module: OpenProcess                         -> Hash: 0x87967048 (dec: 2274783304)
// API/Module: RtlMoveMemory                       -> Hash: 0x6A5B898D (dec: 1784383885)
// API/Module: RtlZeroMemory                       -> Hash: 0xD4782C9E (dec: 3564645534)

// // =========================================================================
// // 7. Forwarded Exports (for module resolution tests)
// // =========================================================================
// API/Module: SetIoRingCompletionEvent            -> Hash: 0xB01A7639 (dec: 2954524217)
// API/Module: SetProtectedPolicy                  -> Hash: 0x50D8DA3F (dec: 1356388927)
// API/Module: SetProcessDefaultCpuSetMasks        -> Hash: 0x85F1DB36 (dec: 2247220022)
// API/Module: SetDefaultDllDirectories            -> Hash: 0x180FE675 (dec: 403695221)
// API/Module: SetProcessDefaultCpuSets            -> Hash: 0x1920E702 (dec: 421586690)
// API/Module: InitializeProcThreadAttributeList   -> Hash: 0x5443D271 (dec: 1413730929)
// API/Module: SystemFunction028                   -> Hash: 0x3C13D4DA (dec: 1007932634)
// API/Module: PerfIncrementULongCounterValue      -> Hash: 0x2B4982D7 (dec: 726237911)
// API/Module: PerfSetCounterRefValue              -> Hash: 0xB904EFFA (dec: 3104108538)
// API/Module: I_QueryTagInformation               -> Hash: 0xE4F1FD65 (dec: 3841064293)
// API/Module: TraceQueryInformation               -> Hash: 0xB9945020 (dec: 3113504800)
// API/Module: TraceMessage                        -> Hash: 0xF60D376D (dec: 4128061293)

// --------------------------------------------------

// [+] SUCCESS: No hash collisions detected among 82 unique hashes.


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
    // List of APIs and Modules grouped by category across puerto / mariana / samoa
    let api_groups: &[(&str, &[&str])] = &[
        (
            "1. Modules (包含大小写，验证 fnv1a_utf16 大小写折叠特征)",
            &[
                "kernel32.dll", "KERNEL32.DLL",
                "ntdll.dll", "NTDLL.DLL",
                "kernelbase.dll", "KERNELBASE.DLL",
                "advapi32.dll", "ADVAPI32.DLL",
                "cryptbase.dll", "CryptBase", "CRYPTBASE.DLL",
            ],
        ),
        (
            "2. Stack Spoofing Core APIs (uwd / mariana)",
            &[
                "RtlUserThreadStart",
                "BaseThreadInitThunk",
            ],
        ),
        (
            "3. Dynamic Syscalls & Native APIs (uwd / mariana / samoa)",
            &[
                "NtAllocateVirtualMemory",
                "NtProtectVirtualMemory",
                "NtCreateThreadEx",
                "NtWriteVirtualMemory",
                "NtOpenProcess",
                "NtQueryInformationProcess",
                "NtGetContextThread",
                "NtSetContextThread",
                "NtSignalAndWaitForSingleObject",
                "NtQueueApcThread",
                "NtAlertResumeThread",
                "NtLockVirtualMemory",
                "NtDuplicateObject",
                "NtCreateEvent",
                "NtWaitForSingleObject",
                "NtClose",
                "NtSetEvent",
                "NtContinue",
                "NtTestAlert",
                "RtlExitUserThread",
                "ZwWaitForWorkViaWorkerFactory",
            ],
        ),
        (
            "4. ThreadPool (TP) APIs (samoa / hypnus)",
            &[
                "TpAllocPool",
                "TpSetPoolStackInformation",
                "TpSetPoolMinThreads",
                "TpSetPoolMaxThreads",
                "TpAllocTimer",
                "TpSetTimer",
                "TpAllocWait",
                "TpSetWait",
                "TpReleaseCleanupGroupMembers",
                "CloseThreadpool",
            ],
        ),
        (
            "5. Memory & Fiber & System Helper APIs (samoa / hypnus)",
            &[
                "RtlWalkHeap",
                "RtlCaptureContext",
                "RtlAcquireSRWLockExclusive",
                "SetProcessValidCallTargets",
                "ConvertFiberToThread",
                "ConvertThreadToFiber",
                "CreateFiber",
                "DeleteFiber",
                "SwitchToFiber",
                "SystemFunction040",
                "SystemFunction041",
                "EnumDateFormatsA",
            ],
        ),
        (
            "6. Exception Handling & Win32 APIs (puerto / dinvk)",
            &[
                "AddVectoredExceptionHandler",
                "RemoveVectoredExceptionHandler",
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
            ],
        ),
        (
            "7. Forwarded Exports (for module resolution tests)",
            &[
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
            ],
        ),
    ];

    let mut hashes: HashMap<u32, &str> = HashMap::new();
    let mut collisions: Vec<(&str, &str, u32)> = Vec::new();

    println!("\n[+] Starting Hash Calculation & Collision Check (puerto / mariana / samoa)...");

    for (category, apis) in api_groups {
        println!("\n// =========================================================================");
        println!("// {}", category);
        println!("// =========================================================================");

        for api in *apis {
            // Convert to UTF-16 for the hash function
            let api_utf16: Vec<u16> = api.encode_utf16().collect();
            let hash = fnv1a_utf16(&api_utf16);

            // Print the calculated hash
            println!("API/Module: {:<35} -> Hash: 0x{:08X} (dec: {})", api, hash, hash);

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
    }

    println!("\n--------------------------------------------------");

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
