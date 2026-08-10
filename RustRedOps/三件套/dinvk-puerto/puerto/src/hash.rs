 



//  1. 传什么字符串？
//   根据 dinvk 的惯例以及 Windows加载器的行为，windows不区分大小写,所以应该将string 全部转为大写.你应该传递：大写的模块名，且通常不带后缀。

//    *目标字符串："NTDLL"
//    *理由：
//        * 大写：Windows
//          模块名是不区分大小写的，将所有输入转为大写（Canonicalization）是确保哈希一致性的标准做法。
//        * 去掉后缀：dinvk 的 canonicalize_module 函数会去掉 .DLL 后缀。如果你在
//          puerto 中也沿用了这个逻辑，那么哈希的对象就是 "NTDLL"。

//   2. 怎么计算（逻辑步骤）？
//   由于你的哈希函数现在接受的是 &[u16]，计算步骤如下：

//    1. 取字符串："NTDLL"
//    2. 转为 UTF-16 字节序列：
//        * 'N' -> 0x004E (78)
//        * 'T' -> 0x0054 (84)
//        * 'D' -> 0x0044 (68)
//        * 'L' -> 0x004C (76)
//        * 'L' -> 0x004C (76)
//    3. 输入哈希函数：将 &[0x004E, 0x0054, 0x0044, 0x004C, 0x004C] 传给你的
//       fnv1a_utf16。

// 为了兼容 Windows的不区分大小写特性，我们在哈希过程中直接进行“位运算转换（Case Folding）”，而不产生新字符串。


// 在 hash.rs 中增加一个处理模块名的逻辑，使其在哈希时自动忽略 .DLL后缀（类似于你做的大小写折叠）
/// PEB-Ldr(InMemoryOrderModuleList)链表中,模块名(BaseDllName)是UNICODE_STRING  结构体.且Windows NT 内核默认使用 UTF-16LE 编码处理底层字符串.即BaseDllName是每个字符占用两个字节的宽字符数组=&[u16](Rust)
/// 直接传入&[u16](PEB里面的字符串格式)给hash函数
/// 
/// 接收&`[u16]`作为参数,用于查找模块(get_module_address)和查找函数(get_proc_address),遍历PEB的InMemoryOrderModuleList双向链表时,模块名BaseDllName在内核中强制是utf-16LE的
pub  fn fnv1a_utf16(data: &[u16]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7; // 你自定义的种子
    const FNV_PRIME: u32 = 0xAD37_79B9;        // 你自定义的素数

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
            // 如果是 ASCII 字符，第二个字节通常是 0，可以根据需求决定是否忽略
            // 这里为了通用性，对两个字节都进行哈希
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// 针对 &[u8] 的 fnv1a 哈希，模拟将其视为 UTF-16 字节流进行哈希
/// 这在处理 ASCII 字符串（如转发字符串中的模块名）并与 PEB 中的 UTF-16 哈希对比时非常有用
/// 处理转发函数与原生ascii字符串(&u8),解析的是PE文件的导出表的AddressOfNames  数组(导出表中的函数名强制规定必须是 ASCII 字符串=&[u8])
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



// [+] Starting Hash Calculation & Collision Check (Standalone Example)...
// --------------------------------------------------
// API: KERNEL32.DLL                        -> Hash: 0x6BEFCBB7
// API: ntdll.dll                           -> Hash: 0xB3383153
// API: advapi32.dll                        -> Hash: 0x37019FB7
// API: kernel32.dll                        -> Hash: 0x6BEFCBB7
// API: NTDLL.DLL                           -> Hash: 0xB3383153
// API: ADVAPI32.DLL                        -> Hash: 0x37019FB7
// API: NtAllocateVirtualMemory             -> Hash: 0x803BA0E0
// API: NtProtectVirtualMemory              -> Hash: 0x17D18FF6
// API: NtCreateThreadEx                    -> Hash: 0xC5C9DC2A
// API: NtWriteVirtualMemory                -> Hash: 0x4ABFD310
// API: NtOpenProcess                       -> Hash: 0xED482F32
// API: NtQueryInformationProcess           -> Hash: 0xC43D7E80
// API: NtGetContextThread                  -> Hash: 0x0FFA8E6A
// API: NtSetContextThread                  -> Hash: 0xAD4FA23E
// API: AddVectoredExceptionHandler         -> Hash: 0x86429FB1
// API: RemoveVectoredExceptionHandler      -> Hash: 0xED2A1F66
// API: LoadLibraryA                        -> Hash: 0x4CE67FBF
// API: VirtualAlloc                        -> Hash: 0x63E4D69B
// API: GetProcessHeap                      -> Hash: 0x4E861A86
// API: GetStdHandle                        -> Hash: 0x5E6A00C8
// API: GetProcAddress                      -> Hash: 0x5EF0E069
// API: GetModuleHandleA                    -> Hash: 0x74BD07C0
// API: ExitProcess                         -> Hash: 0xDEC8009C
// API: WaitForSingleObject                 -> Hash: 0x1C442098
// API: CreateFileW                         -> Hash: 0xDA6054C2
// API: ReadFile                            -> Hash: 0x219168D3
// API: WriteFile                           -> Hash: 0x9C01094C
// API: VirtualFree                         -> Hash: 0xE028A812
// API: VirtualProtect                      -> Hash: 0x03D3511D
// API: CreateRemoteThread                  -> Hash: 0x53E210B9
// API: OpenProcess                         -> Hash: 0x87967048
// API: RtlMoveMemory                       -> Hash: 0x6A5B898D
// API: RtlZeroMemory                       -> Hash: 0xD4782C9E
// API: SetIoRingCompletionEvent            -> Hash: 0xB01A7639
// API: SetProtectedPolicy                  -> Hash: 0x50D8DA3F
// API: SetProcessDefaultCpuSetMasks        -> Hash: 0x85F1DB36
// API: SetDefaultDllDirectories            -> Hash: 0x180FE675
// API: SetProcessDefaultCpuSets            -> Hash: 0x1920E702
// API: InitializeProcThreadAttributeList   -> Hash: 0x5443D271
// API: SystemFunction028                   -> Hash: 0x3C13D4DA
// API: PerfIncrementULongCounterValue      -> Hash: 0x2B4982D7
// API: PerfSetCounterRefValue              -> Hash: 0xB904EFFA
// API: I_QueryTagInformation               -> Hash: 0xE4F1FD65
// API: TraceQueryInformation               -> Hash: 0xB9945020
// API: TraceMessage                        -> Hash: 0xF60D376D
// --------------------------------------------------

// [+] SUCCESS: No hash collisions detected among 42 unique hashes