//! 全局错误类型定义与状态码契约
//!
//! 统一收敛 Agent 各功能模块的错误代号，杜绝类型漂移并规避内存/字符串分析特征。

// ==========================================
// 1. 加密与安全物理帧错误码 (Crypto)
// ==========================================

// 仅在 debug 或 test 下生成 Debug 派生，Release 模式下剔除以消除 .rdata 字符串指纹
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
// 默认情况下,Rust拥有枚举内存布局的自主排布权.此处使用 #[repr(u8)] 强制其判别值(Discriminant)严格占用1字节,使整个错误枚举实例仅占1字节,对内存极限压缩友好
#[repr(u8)]
pub enum CryptoError {
    // === 10~19: 上下文初始化阶段 (Context / Init) ===
    /// 主密钥长度不符合 32 字节要求 (触发于 CryptoContext::new)
    MasterKeyInvalidLength = 10,

    // === 20~29: 封包加密阶段 (Seal / Outgoing) ===
    /// 传入的 Agent UUID 长度不为 36 字节
    SealUuidInvalidLength = 20,
    /// 待发送明文超过 MAX_PLAINTEXT_LEN 上限
    SealPlaintextTooLong = 21,
    /// 调用者提供的 raw_buf 二进制暂存区容量不足
    SealRawBufTooSmall = 22,
    /// 初始化 CBC 加密器失败 (密钥/IV 异常)
    SealCipherInitFailed = 23,
    /// AES-CBC 原地填充加密失败
    SealEncryptionFailed = 24,
    /// 初始化 HMAC 签名器失败
    SealHmacInitFailed = 25,
    /// 调用者提供的 out_b64 输出缓冲区不足以容纳 Base64 文本
    SealB64OutBufTooSmall = 26,

    // === 30~39: 解包验真阶段 (Open / Incoming) ===
    /// 服务端返回的数据不是合法的 Base64 编码
    OpenBase64DecodeFailed = 30,
    /// Base64 解码后二进制长度小于协议最小物理帧
    OpenPayloadTooShort = 31,
    /// 初始化 HMAC 验真器失败
    OpenHmacInitFailed = 32,
    /// HMAC-SHA256 签名校验失败 (数据被篡改或密钥不匹配)
    OpenHmacMismatch = 33,
    /// 初始化 CBC 解密器失败
    OpenCipherInitFailed = 34,
    /// AES-CBC 解密填充剔除失败 (PKCS#7 损坏或密文损坏)
    OpenDecryptionFailed = 35,
}

// 因为有Clone和Copy,这里并不会转移所有权,导致原变量失效
impl CryptoError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ==========================================
// 2. 文件传输与断点续传错误码密码本 (File Transfer / Resume)
// ==========================================

/// 文件传输状态机操作错误代号密码本 (File Transfer Error Codebook)
///
/// 严格限定为单字节 u8 判别值布局，无堆分配，Release 模式下杜绝任何明文字符串与符号信息。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum FileTransferError {
    // === 50~59: 状态机与服务端协议握手阶段 (Session & Handshake) ===
    /// 状态机处于不合法状态 (如尚未注册就尝试发数据或已传输完毕)
    InvalidState = 50,
    /// 缺少服务端分配的 File UUID
    MissingFileId = 51,
    /// 服务端返回非 success 状态
    ServerStatusError = 52,
    /// 断点续传回执中缺少 chunk_offset 字段
    ServerMissingOffset = 53,
    /// 偏移量超出文件总大小
    OffsetOutOfBounds = 54,
    /// 分片序号或字节偏移量不匹配 (如收到乱序分片)
    OffsetMismatch = 55,

    // === 60~69: 缓冲区与 I/O 编排阶段 (Buffer & I/O) ===
    /// 调用者提供的缓冲区容量过小
    BufferTooSmall = 60,
    /// Base64 编码或解码失败
    Base64Error = 61,
    /// 底层文件 I/O 读写失败
    IoError = 62,
}

impl FileTransferError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ==========================================
// 3. 协议解析与数据校验错误码 (Model / Protocol)
// ==========================================

/// Mythic 协议解析与数据校验错误代号密码本
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum ModelError {
    /// 接口动作标志匹配失败或为空
    InvalidAction = 40,
    /// C2 服务端返回 status 不为 "success"
    ResponseStatusError = 41,
    /// 必填参数缺失或格式长度校验失败 (如 UUID 长度不为 36)
    MissingRequiredField = 42,
    /// 文件分片序号、偏移或总片数参数非法
    InvalidChunkInfo = 43,
    /// 交互式终端消息类型或编码异常
    InvalidInteractiveType = 44,
}

impl ModelError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ==========================================
// 4. HTTP 网络通信与传输层错误码 (Http / Transport)
// ==========================================

/// Profile 网络通信与传输层错误代号密码本
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum HttpError {
    /// 封包阶段加密失败
    CryptoSealFailed = 70,
    /// 解包阶段解密或验签失败
    CryptoOpenFailed = 71,
    /// JSON 序列化失败
    JsonSerializeFailed = 72,
    /// JSON 反序列化失败
    JsonDeserializeFailed = 73,
    /// 网络传输或 Socket I/O 失败
    NetworkTransportFailed = 74,
    /// C2 服务端响应异常 (如 HTTP 状态码非 200 或业务状态 error)
    ServerResponseError = 75,
    /// 缓冲区容量溢出
    BufferOverflow = 76,
}

impl HttpError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ==========================================
// 5. Agent 核心状态机与任务调度错误码 (Agent Engine)
// ==========================================

/// Agent 核心运行状态机与调度错误代号密码本
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum AgentError {
    /// 尚未完成上线注册 (Checkin)
    NotCheckedIn = 80,
    /// 服务端下发的回调 Callback UUID 非法
    InvalidCallbackUuid = 81,
    /// 任务执行异常
    TaskExecutionFailed = 82,
    /// 未知或未实现的命令
    UnknownCommand = 83,
    /// 系统环境元数据采集失败
    SysinfoFailed = 84,
    /// 网络传输与 I/O 交互失败
    TransportFailed = 85,
    /// 状态机会话状态非法
    InvalidSessionState = 86,
}

impl AgentError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_error_code() {
        assert_eq!(CryptoError::MasterKeyInvalidLength.code(), 10);
        assert_eq!(CryptoError::SealUuidInvalidLength.code(), 20);
        assert_eq!(CryptoError::OpenBase64DecodeFailed.code(), 30);
    }

    #[test]
    fn test_file_transfer_error_code() {
        assert_eq!(FileTransferError::InvalidState.code(), 50);
        assert_eq!(FileTransferError::MissingFileId.code(), 51);
        assert_eq!(FileTransferError::ServerStatusError.code(), 52);
        assert_eq!(FileTransferError::BufferTooSmall.code(), 60);
        assert_eq!(FileTransferError::Base64Error.code(), 61);
        assert_eq!(FileTransferError::IoError.code(), 62);
    }

    #[test]
    fn test_model_and_http_error_code() {
        assert_eq!(ModelError::InvalidAction.code(), 40);
        assert_eq!(ModelError::InvalidInteractiveType.code(), 44);
        assert_eq!(HttpError::CryptoSealFailed.code(), 70);
        assert_eq!(HttpError::BufferOverflow.code(), 76);
        assert_eq!(AgentError::NotCheckedIn.code(), 80);
        assert_eq!(AgentError::UnknownCommand.code(), 83);
        assert_eq!(AgentError::SysinfoFailed.code(), 84);
        assert_eq!(AgentError::TransportFailed.code(), 85);
        assert_eq!(AgentError::InvalidSessionState.code(), 86);
    }
}

