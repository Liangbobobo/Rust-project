 
/// 这里是否可以修改为返回一个十六进制的数?
 pub fn fnv1a(string: &str) -> u32 {

    /// 需要修改的种子
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7;

    /// 需要修改的素数(避免出现hash撞库)
    const FNV_PRIME: u32 = 0xAD37_79B9;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in string.as_bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}