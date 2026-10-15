//! # 纯栈零堆底层流式 I/O 原语库 (Zero-Heap Stream I/O Primitives)
//!
//! ## 架构核心哲学与 100% 零堆纯栈契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **专注于硬件设备与数据源的底层 I/O 流抽象 (Tier 1: I/O 驱动层)**：
//!    - 彻底脱离标准库与外部 C2 业务协议，只专注底层字节流的移动、读写与寻道；
//!    - 定义纯栈零堆的 [`ZeroRead`]、[`ZeroWrite`] 与 [`ZeroSeek`] 抽象 Trait；
//!    - 提供纯内存切片数据流 ([`MemorySliceReader`]) 与 Windows 原生文件句柄 ([`NativeFileHandle`])。
//! 2. **未来流式协议扩充准备**：
//!    - 为未来引入 **Windows 命名管道流 (Named Pipe `\\.\pipe\...` 用于 SMB 级联穿透)**、
//!      **网络套接字流 (TCP/Socks5 代理)** 提供标准化原语接口，与上层传输协议完全正交解耦。
//! 3. **100% 零堆纯栈与错误收敛**：
//!    - 所有错误收敛至全局密码本 [`FileTransferError`]。

pub use crate::error::FileTransferError;

// ==========================================
// 1. 核心流式 I/O 抽象 (Zero-Heap Stream I/O Traits)
// ==========================================

/// 纯栈零堆流式读取抽象 (Zero-Heap Stream Reader Trait):这里只是一个trait,内部是一个函数签名,需要后续自己实现对应的功能.其对应的trait 原型是std::io::Read
///
/// 替代 `std::io::Read`，完全脱离标准库，所有错误收敛至密码本代号 [`FileTransferError`]。
pub trait ZeroRead {
    /// 从数据源尝试读取最多 `buf.len()` 个字节写入调用者提供的栈缓冲区
    ///
    /// * 若 `buf` 为空切片 (`buf.is_empty()`)，直接返回 `Ok(0)`，调用者不得将其视为 EOF
    /// * 若 `buf` 非空且返回 `Ok(0)`，代表到达真实 EOF (End Of File)
    /// * 返回 `Ok(n)` 代表实际读取的字节数 (0 < n <= buf.len())
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError>;
}

/// 纯栈零堆字节偏移定位抽象 (Zero-Heap Stream Seek Trait)
///
/// 替代 `std::io::Seek`。针对 Mythic 4.0 的 Byte-Offset 纯绝对偏移协议进行特化优化。
pub trait ZeroSeek {
    /// 跳转至流的绝对起始偏移量 (0-based)
    ///
    /// * `offset`: 目标绝对字节偏移量
    /// * 返回调整后的新偏移量
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError>;
}

/// 纯栈零堆流式写入抽象 (Zero-Heap Stream Writer Trait)
///
/// 替代 `std::io::Write`，供分片接收与落盘使用。
pub trait ZeroWrite {
    /// 将 `buf` 中的全量字节写入底层设备或内存
    fn zero_write_all(&mut self, buf: &[u8]) -> Result<(), FileTransferError>;

    /// 刷新缓冲区 (物理持久化)
    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        Ok(())
    }
}

// ----------------- 引用类型毯式实现 (Blanket Impl for &mut T) -----------------

// Rust 类型系统中，NativeFileHandle 和 &mut NativeFileHandle是不同类型.这里重新定义&mut R,方便后续使用process_stream(&mut file)处理文件句柄
// ?Sized 代表R可以是可以是动态尺寸类型
impl<R: ZeroRead + ?Sized> ZeroRead for &mut R {
    #[inline]
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError> {
        // 这里是调用者调用自己实现的zero_read函数.而不是在函数中调用其自身
        // 当前类型是&mut R 而zero_read参数是&mut self.那么传入参数类型实际上是&mut Self(即 &mut (&mut R))就是可变引用的可变引用，即两层指针.只有经过两层解引用** 才能得到真实的调用zero_read的结构体
        (**self).zero_read(buf)
    }
}

impl<R: ZeroSeek + ?Sized> ZeroSeek for &mut R {
    #[inline]
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError> {
        (**self).zero_seek_start(offset)
    }
}

impl<W: ZeroWrite + ?Sized> ZeroWrite for &mut W {
    #[inline]
    fn zero_write_all(&mut self, buf: &[u8]) -> Result<(), FileTransferError> {
        (**self).zero_write_all(buf)
    }

    #[inline]
    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        (**self).zero_flush()
    }
}

// ----------------- 标准库类型与内存切片适配实现 -----------------

// 多态适配.利用rust的std实现跨平台文件操作的抽象.针对win平台后面有专门的NativeFileHandle实现,会剔除std环境
impl ZeroRead for std::fs::File {
    #[inline]
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError> {
        // 标准库已经对std::io::Read实现了源码,所以这里只需要提供参数.自己不需要也不能实现Read
        // 这里只引入了std的Read trait并没有引入其他堆分配或者明文,是安全的.引入部分std并不代表不隐蔽
        use std::io::Read;
        // 1. 防御空切片: 避免无谓系统调用，防止触发下游假 EOF 连锁误判
        if buf.is_empty() {
            return Ok(0);
        }
        // 2. 循环吸收瞬态信号中断 (ErrorKind::Interrupted)，防止传输因 Windows APC 或调试中断异常中止
        loop {
            match self.read(buf) {
                Ok(n) => return Ok(n),
                // 通过ref来借用产生的错误,而不是move
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(FileTransferError::IoError),
            }
        }
    }
}

// std::io::seek为具有光标/游标定位能力的对象定义的通用接口.即负责把文件的读取光标瞬间挪到指定字节位置，专门用于断点续传时直接跳过已传输的部分
// 计算已经传输了多少数据在self.current_offset这里
// 这里抽象一层,并不是重复/冗余.避免了原生std::io::seek绑定的std::io::Error(可能触发堆分配和明文错误描述符号).
impl ZeroSeek for std::fs::File {
    #[inline]
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError> {
        use std::io::Seek;
        self.seek(std::io::SeekFrom::Start(offset))
            .map_err(|_| FileTransferError::IoError)
    }
}

// ZeroWrite代表一个写入器必须实现的trait.写入器可以是磁盘文件/内存缓冲区/标准输入管道句柄/TCP Socket 或 SOCKS5 代理流等
impl ZeroWrite for std::fs::File {
    #[inline]
    fn zero_write_all(&mut self, buf: &[u8]) -> Result<(), FileTransferError> {
        use std::io::Write;
        self.write_all(buf).map_err(|_| FileTransferError::IoError)
    }

    #[inline]
    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        use std::io::Write;
        self.flush().map_err(|_| FileTransferError::IoError)
    }
}

// Rust 标准库中，普通的内存切片（如 &[u8]、&mut [u8] 或Vec<u8>）只是内存里的一段固定连续字节，它本身没有“当前读写到哪一行/哪一个字节”的光标状态指针，也不具备随意向前、向后跳转（Seek）的能力
// std::io::Cursor<T> 的作用就是给这段纯内存数据加上一个内部游标（pos:u64），把它**“伪装/包装成一个虚拟文件”**.like Cursor<Vec<u8>> and Cursor<&[u8]>
impl<T> ZeroRead for std::io::Cursor<T>
where
    std::io::Cursor<T>: std::io::Read,
{
    #[inline]
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError> {
        use std::io::Read;
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            match self.read(buf) {
                Ok(n) => return Ok(n),
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(FileTransferError::IoError),
            }
        }
    }
}

impl<T> ZeroSeek for std::io::Cursor<T>
where
    std::io::Cursor<T>: std::io::Seek,
{
    #[inline]
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError> {
        use std::io::Seek;
        self.seek(std::io::SeekFrom::Start(offset))
            .map_err(|_| FileTransferError::IoError)
    }
}

#[cfg(test)]
impl ZeroWrite for std::vec::Vec<u8> {
    #[inline]
    fn zero_write_all(&mut self, buf: &[u8]) -> Result<(), FileTransferError> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        Ok(())
    }
}

/// 对&mut [u8]实现ZeroWrite trait
impl ZeroWrite for &mut [u8] {
    #[inline]
    fn zero_write_all(&mut self, buf: &[u8]) -> Result<(), FileTransferError> {
        if self.len() < buf.len() {
            return Err(FileTransferError::IoError);
        }
        self[..buf.len()].copy_from_slice(buf);
        let (_, rest) = core::mem::take(self).split_at_mut(buf.len());
        *self = rest;
        Ok(())
    }

    #[inline]
    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        Ok(())
    }
}

// ==========================================
// 2. 纯内存切片流式寻轨游标 (MemorySliceReader)
// ==========================================

/// 纯内存切片流式寻轨游标 (Pure-Core In-Memory Seekable Cursor)
///
/// 100% 运行于 core 核心库，零堆分配，纯栈操作，完全取代 `std::io::Cursor`。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct MemorySliceReader<'a> {
    slice: &'a [u8],
    offset: usize,
}

impl<'a> MemorySliceReader<'a> {
    /// 创建内存切片读取游标 (起始偏移量 0)
    #[inline(always)]
    pub const fn new(slice: &'a [u8]) -> Self {
        Self { slice, offset: 0 }
    }

    /// 创建带指定起始偏移量的内存切片游标
    #[inline(always)]
    pub const fn new_at(slice: &'a [u8], offset: usize) -> Self {
        Self { slice, offset }
    }

    /// 获取底层切片完整长度
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.slice.len()
    }

    /// 查询是否为空切片
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    /// 获取当前游标位置
    #[inline(always)]
    pub const fn position(&self) -> usize {
        self.offset
    }
}

impl<'a> ZeroRead for MemorySliceReader<'a> {
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError> {
        if self.offset >= self.slice.len() {
            // 是不是带一个错误提示更好?
            return Ok(0);
        }
        let available = &self.slice[self.offset..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.offset += to_read;
        Ok(to_read)
    }
}

impl<'a> ZeroSeek for MemorySliceReader<'a> {
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError> {
        self.offset = offset as usize;
        Ok(self.offset as u64)
    }
}

// ==========================================
// 3. Windows 原生底层文件句柄 (NativeFileHandle)
// ==========================================

#[cfg(windows)]
unsafe extern "system" {
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut core::ffi::c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_void;

    fn ReadFile(
        hFile: *mut core::ffi::c_void,
        lpBuffer: *mut core::ffi::c_void,
        nNumberOfBytesToRead: u32,
        lpNumberOfBytesRead: *mut u32,
        lpOverlapped: *mut core::ffi::c_void,
    ) -> i32;

    fn WriteFile(
        hFile: *mut core::ffi::c_void,
        lpBuffer: *const core::ffi::c_void,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut core::ffi::c_void,
    ) -> i32;

    fn SetFilePointerEx(
        hFile: *mut core::ffi::c_void,
        liDistanceToMove: i64,
        lpNewFilePointer: *mut i64,
        dwMoveMethod: u32,
    ) -> i32;

    fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;

    fn FlushFileBuffers(hFile: *mut core::ffi::c_void) -> i32;

    fn GetFileSizeEx(hFile: *mut core::ffi::c_void, lpFileSize: *mut i64) -> i32;
}

#[cfg(windows)]
const GENERIC_READ: u32 = 0x80000000;
#[cfg(windows)]
const GENERIC_WRITE: u32 = 0x40000000;
#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x00000001;
#[cfg(windows)]
const CREATE_ALWAYS: u32 = 2;
#[cfg(windows)]
const OPEN_EXISTING: u32 = 3;
#[cfg(windows)]
const OPEN_ALWAYS: u32 = 4;
#[cfg(windows)]
const FILE_ATTRIBUTE_NORMAL: u32 = 0x00000080;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: *mut core::ffi::c_void = -1isize as *mut core::ffi::c_void;
#[cfg(windows)]
const FILE_BEGIN: u32 = 0;

#[cfg(windows)]
fn encode_path_utf16(src: &str, dst: &mut [u16]) -> Result<(), FileTransferError> {
    let mut i = 0;
    for c in src.encode_utf16() {
        if i + 1 >= dst.len() {
            return Err(FileTransferError::IoError);
        }
        dst[i] = c;
        i += 1;
    }
    dst[i] = 0;
    Ok(())
}

/// Windows 原生 Win32 物理文件句柄 (100% 剥离 Rust std::fs 运行时符号与错误字符串)
#[cfg(windows)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct NativeFileHandle {
    handle: *mut core::ffi::c_void,
    owns_handle: bool,
}

#[cfg(windows)]
impl NativeFileHandle {
    /// 从原始 Win32 HANDLE 创建包装对象
    ///
    /// # Safety
    /// 调用者必须保证 handle 有效，且在 owns_handle 为 true 时独占该句柄生命周期。
    pub const unsafe fn from_raw(handle: *mut core::ffi::c_void, owns_handle: bool) -> Self {
        Self {
            handle,
            owns_handle,
        }
    }

    /// 获取底层 Win32 HANDLE
    pub const fn as_raw(&self) -> *mut core::ffi::c_void {
        self.handle
    }

    /// 使用纯栈 UTF-16 转换以只读模式打开本地文件 (零堆分配，直调 CreateFileW)
    pub fn open_read(path: &str) -> Result<Self, FileTransferError> {
        let mut path_w = [0u16; 320];
        encode_path_utf16(path, &mut path_w)?;

        let handle = unsafe {
            CreateFileW(
                path_w.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ,
                core::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                core::ptr::null_mut(),
            )
        };

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(FileTransferError::IoError);
        }

        Ok(Self {
            handle,
            owns_handle: true,
        })
    }

    /// 使用纯栈 UTF-16 转换以追加写入模式打开或创建本地文件 (零堆分配，直调 CreateFileW)
    pub fn open_write_append(path: &str) -> Result<Self, FileTransferError> {
        let mut path_w = [0u16; 320];
        encode_path_utf16(path, &mut path_w)?;

        let handle = unsafe {
            CreateFileW(
                path_w.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ,
                core::ptr::null_mut(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                core::ptr::null_mut(),
            )
        };

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(FileTransferError::IoError);
        }

        Ok(Self {
            handle,
            owns_handle: true,
        })
    }

    /// 使用纯栈 UTF-16 转换以覆盖新建/截断写入模式打开本地文件 (零堆分配，直调 CreateFileW CREATE_ALWAYS)
    pub fn open_write_truncate(path: &str) -> Result<Self, FileTransferError> {
        let mut path_w = [0u16; 320];
        encode_path_utf16(path, &mut path_w)?;

        let handle = unsafe {
            CreateFileW(
                path_w.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ,
                core::ptr::null_mut(),
                CREATE_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                core::ptr::null_mut(),
            )
        };

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(FileTransferError::IoError);
        }

        Ok(Self {
            handle,
            owns_handle: true,
        })
    }

    /// 获取底层物理文件的真实大小 (零堆分配，直调 Win32 GetFileSizeEx)
    pub fn file_size(&self) -> Result<u64, FileTransferError> {
        let mut size = 0i64;
        let ok = unsafe { GetFileSizeEx(self.handle, &mut size) };
        if ok == 0 || size < 0 {
            return Err(FileTransferError::IoError);
        }
        Ok(size as u64)
    }
}

#[cfg(windows)]
impl Drop for NativeFileHandle {
    fn drop(&mut self) {
        if self.owns_handle && !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.handle);
            }
            self.handle = INVALID_HANDLE_VALUE;
        }
    }
}

#[cfg(windows)]
impl ZeroRead for NativeFileHandle {
    fn zero_read(&mut self, buf: &mut [u8]) -> Result<usize, FileTransferError> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut bytes_read = 0u32;
        let to_read = (buf.len() as u64).min(u32::MAX as u64) as u32;
        let ok = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                to_read,
                &mut bytes_read,
                core::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(FileTransferError::IoError);
        }
        Ok(bytes_read as usize)
    }
}

#[cfg(windows)]
impl ZeroSeek for NativeFileHandle {
    fn zero_seek_start(&mut self, offset: u64) -> Result<u64, FileTransferError> {
        let mut new_pos = 0i64;
        let ok = unsafe {
            SetFilePointerEx(
                self.handle,
                offset as i64,
                &mut new_pos,
                FILE_BEGIN,
            )
        };
        if ok == 0 {
            return Err(FileTransferError::IoError);
        }
        Ok(new_pos as u64)
    }
}

#[cfg(windows)]
impl ZeroWrite for NativeFileHandle {
    fn zero_write_all(&mut self, mut buf: &[u8]) -> Result<(), FileTransferError> {
        while !buf.is_empty() {
            let to_write = (buf.len() as u64).min(u32::MAX as u64) as u32;
            let mut bytes_written = 0u32;
            let ok = unsafe {
                WriteFile(
                    self.handle,
                    buf.as_ptr() as *const core::ffi::c_void,
                    to_write,
                    &mut bytes_written,
                    core::ptr::null_mut(),
                )
            };
            if ok == 0 || bytes_written == 0 {
                return Err(FileTransferError::IoError);
            }
            buf = &buf[bytes_written as usize..];
        }
        Ok(())
    }

    fn zero_flush(&mut self) -> Result<(), FileTransferError> {
        let ok = unsafe { FlushFileBuffers(self.handle) };
        if ok == 0 {
            return Err(FileTransferError::IoError);
        }
        Ok(())
    }
}


// ==========================================
// 4. 单元测试 (纯 I/O 流原语验证)
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_slice_reader_seek_and_read() {
        let sample = b"0123456789ABCDEF";
        let mut reader = MemorySliceReader::new(sample);
        assert_eq!(reader.len(), 16);
        assert_eq!(reader.position(), 0);

        let mut buf = [0u8; 4];
        let n = reader.zero_read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"0123");
        assert_eq!(reader.position(), 4);

        // Seek forward
        let new_pos = reader.zero_seek_start(10).unwrap();
        assert_eq!(new_pos, 10);
        let n = reader.zero_read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"ABCD");

        // Read to end
        let n = reader.zero_read(&mut buf).unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf[..2], b"EF");

        // EOF
        let n = reader.zero_read(&mut buf).unwrap();
        assert_eq!(n, 0);

        // Seek back to start
        reader.zero_seek_start(0).unwrap();
        let n = reader.zero_read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"0123");
    }

    #[cfg(windows)]
    #[test]
    fn test_native_file_handle_roundtrip() {
        let test_file = "target\\test_native_handle.bin";
        let _ = std::fs::remove_file(test_file);

        // 1. 原生写入文件
        let test_payload = b"Wisconsin Native Win32 Handle File I/O Test Payload 2026";
        {
            let mut writer = NativeFileHandle::open_write_append(test_file).expect("open native file write");
            writer.zero_write_all(test_payload).expect("write native file");
            writer.zero_flush().expect("flush native file");
        }

        // 2. 原生读取与 Seek 测试
        {
            let mut reader = NativeFileHandle::open_read(test_file).expect("open native file read");
            let mut buf = [0u8; 9];
            let read_bytes = reader.zero_read(&mut buf).expect("read native file");
            assert_eq!(read_bytes, 9);
            assert_eq!(&buf, b"Wisconsin");

            // Seek 到 10 字节处读取 "Native"
            let new_pos = reader.zero_seek_start(10).expect("seek native file");
            assert_eq!(new_pos, 10);
            let mut buf_native = [0u8; 6];
            let read_native = reader.zero_read(&mut buf_native).expect("read native after seek");
            assert_eq!(read_native, 6);
            assert_eq!(&buf_native, b"Native");
        }

        // 3. 接入 TransferSession 泵送测试
        {
            let mut reader = NativeFileHandle::open_read(test_file).expect("open native file read for session");
            let uuid = [0x77u8; 36];
            let mut session = crate::transfer::TransferSession::new_with_transfer_id(
                "task-native-dl",
                test_payload.len() as i64,
                uuid,
                0,
            );
            let mut ws = crate::protocol::ActiveStackWorkspace::new();
            let (chunk, pumped) = session.pump_file_chunk(&mut reader, &mut ws, false).expect("pump native chunk");
            assert_eq!(pumped, test_payload.len());
            assert_eq!(chunk.chunk_offset, Some(0));
            assert!(session.is_complete);
        }

        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_zero_read_empty_buffer_and_cursor() {
        let mut cursor = std::io::Cursor::new(b"Hello Wisconsin".to_vec());
        let mut empty_buf = [0u8; 0];
        // 传入空切片必须返回 Ok(0)，且游标不能移动
        let n = cursor.zero_read(&mut empty_buf).unwrap();
        assert_eq!(n, 0);

        let mut read_buf = [0u8; 5];
        let n = cursor.zero_read(&mut read_buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&read_buf, b"Hello");
    }

    #[test]
    fn test_zero_read_file_empty_buffer() {
        let temp_path = "target\\test_zero_read_empty.tmp";
        std::fs::write(temp_path, b"Test Content").unwrap();
        let mut file = std::fs::File::open(temp_path).unwrap();

        // 1. 传入空切片返回 Ok(0)
        let mut empty_buf = [0u8; 0];
        assert_eq!(file.zero_read(&mut empty_buf).unwrap(), 0);

        // 2. 正常读取不会被空读取影响
        let mut data_buf = [0u8; 4];
        assert_eq!(file.zero_read(&mut data_buf).unwrap(), 4);
        assert_eq!(&data_buf, b"Test");

        drop(file);
        let _ = std::fs::remove_file(temp_path);
    }
}
