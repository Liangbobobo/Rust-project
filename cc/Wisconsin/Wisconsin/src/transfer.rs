//! # 通用传输与分片编排协议引擎 (Mythic 4.0 Transfer Engine Facade)
//!
//! ## 架构分层定位 (Tier 2: 传输协议编排层)
//!
//! 本模块专注于 Mythic 4.0 C2 应用层传输协议编排与断点续传状态机逻辑，与底层物理 I/O 流 ([`crate::stream`]) 正交解耦：
//! 1. **外发分片编排 ([`outbound::TransferSession`])**：
//!    - 消费多态流式输入 ([`TransferSource`])，驱动 0-based 字节偏移分片切分、Jitter 抖动与 Base64 纯栈封包；
//!    - 管理首次向 Mythic 申请 `file_id` 的注册握手与异常断点续传握手 (`resume: true`)。
//! 2. **下发流式接收 ([`inbound::TransferReceiveSession`])**：
//!    - 驱动分片请求生成 (`FileUploadRequest`)，Base64 解码并流式落盘写入目标 Writer。

pub mod inbound;
pub mod outbound;

pub use inbound::TransferReceiveSession;
pub use outbound::{TransferRegisterMeta, TransferSession};

use crate::stream::{MemorySliceReader, ZeroRead, ZeroSeek};

/// 多态数据输入源 (Polymorphic Transfer Source)
///
/// 统一抽象物理文件句柄与纯内存切片两类数据输入，杜绝两套冗余传输逻辑。
/// 默认泛型参数使用纯 `core` 的 [`MemorySliceReader`]，消除强制泛型标注。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub enum TransferSource<'src, R: ZeroRead + ZeroSeek = MemorySliceReader<'src>> {
    /// 物理文件句柄或流式 Reader (必须实现 ZeroRead + ZeroSeek 以支持随机 Offset 定位与断点续传)
    File(&'src mut R),
    /// 内存二进制数据切片 (零拷贝切片借用，如大回显、内存转储、截图切片)
    MemorySlice(&'src [u8]),
}

impl<'src, R: ZeroRead + ZeroSeek> TransferSource<'src, R> {
    /// 创建文件句柄传输源 (支持 std::fs::File 与 Windows NativeFileHandle)
    #[inline(always)]
    pub fn file(reader: &'src mut R) -> Self {
        Self::File(reader)
    }
}

impl<'src> TransferSource<'src, MemorySliceReader<'src>> {
    /// 创建内存切片传输源 (100% core 零分配，默认类型免除多余泛型标注)
    #[inline(always)]
    pub fn slice(slice: &'src [u8]) -> Self {
        Self::MemorySlice(slice)
    }
}
