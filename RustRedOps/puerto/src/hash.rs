 



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
/// 直接传入&[u16]给hash函数
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
