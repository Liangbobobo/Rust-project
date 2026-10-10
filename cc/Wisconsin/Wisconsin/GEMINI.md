# Wisconsin Agent 开发准则与 Mythic 4.0 核心架构规范 (Project Rules & Memory)

## 一、 Wisconsin Agent 核心开发准则 (OPSEC & Architecture)



1. **禁止裸写 `#[derive(Debug)]`**：
   - 必须使用 `#[cfg_attr(any(debug_assertions, test), derive(Debug))]`。
   - 自定义实现 `core::fmt::Debug` 同样必须包裹 `#[cfg(any(debug_assertions, test))]` 门控。
   - 确保 Release 模式下彻底剥离 Debug 格式化符号与结构体字段反射，杜绝在 `.rdata` 节中留下敏感特征字符串。

2. **密码本式错误控制 (Error Codebook Pattern)**：
   - 所有模块错误必须统一收敛在 `src/error.rs` 中。
   - 采用 `#[repr(u8)]` 紧凑内存布局，赋予每个变体唯一的数字错误码（如 Crypto 10~35, Model 40~49, Transfer 50~69, Http 70~79）。
   - **严禁**在 Release 下实现 `core::fmt::Display` 或嵌入任何明文错误描述字符串。
   - 对外仅导出 `code(self) -> u8`。文档注释充当开发与运营人员的“离线密码本”，通过数字代号定位排错。

3. **零堆分配与纯栈优先哲学 (Zero-Heap / Stack-First)**：
   - 核心通信与数据处理热点路径杜绝 `String` / `Vec`，优先使用切片借用 `&'a str`、定长栈缓冲区（如 `[u8; 36]`、`[u8; 260]`）或纯栈容器 `StackVec<T, N>`，规避 Windows 10/11 Segment Heap 碎片与 ETW-Ti 遥测追踪。

4. **统一以 Rust 1.94 现代标准开发 (Rust 1.94 Modern Target)**：
   - 永久严格锁定 Rust 1.94 (2024 edition) 作为基准版本，全面采用常量求值 `const context`（如 `[const { MaybeUninit::uninit() }; N]`）、无 `unsafe` 零开销原语及标准库最新优化特性。

5. **编译期全量静态字符串混淆 (obfstr)**：
   - 所有敏感网络与协议字符串（如 Action 动作、路径、协议参数、默认配置）必须使用 `obfstr` 宏在编译期混淆并在栈上就地解密，严禁以明文字面量暴露在 `.rdata` 节中。

---

## 二、 Mythic 4.0 官方规范与通信协议全景备忘 (Mythic 4.0 Protocol Specification)

1. **物理帧格式 (Agent Message Wire Format)**：
   - 外部封包格式：`Base64( UUID (36 字节 ASCII) + EncryptedBlob )`。
   - 密文帧结构：`IV (16 字节随机) + Ciphertext (AES-256-CBC, PKCS#7) + Tag (32 字节 HMAC-SHA256)`。
   - **密码学规则（原生规范）**：在标准 `aes256_hmac` 模式下，Mythic 服务端直接使用 32 字节的 `AESPSK` 作为 AES 密钥和 HMAC 密钥（`HMAC is SHA256 with the same AES key over (IV + Ciphertext)`），**禁止使用 HKDF 派生不同子密钥**，否则会导致官方 C2 服务端解密及验签失败。
   - 协商与置换（Staging）：通过 `staging_rsa`（4096-bit RSA PKCS#1 OAEP SHA1）传递客户端公钥换取 `session_key`，使用 `tempUUID` 暂存状态，最终换取 `callbackUUID`。

2. **上线注册协议 (Checkin)**：
   - 请求动作：`action: "checkin"`。
   - 字段规范：网络 IP 必须使用复数切片/数组 `"ips": ["..."]`（不得使用单数 `ip`）；核心字段包含 `uuid`, `os`, `user`, `host`, `pid`, `architecture`, `domain`, `integrity_level` (1: Low, 2: Med, 3: High, 4: SYSTEM), `process_name` 等。
   - 响应格式：`{"action": "checkin", "id": "<new_callback_uuid>", "status": "success"}`。

3. **任务轮询与载荷捎带 (GetTasking & Piggybacking)**：
   - 请求动作：`action: "get_tasking"`，包含 `tasking_size`（通常为 1 或 -1）。
   - **顺风车捎带 (Piggybacking)**：`get_tasking` 必须支持可选的 `responses` 数组切片以及 `socks`, `rpfwd`, `delegates`, `edges`, `alerts`, `interactive`。
   - 攻防价值：允许 Agent 在一次 HTTP 心跳轮询中同时完成任务回执/文件分片回传与新任务拉取，避免触发高频连续连接异常，破坏 NDR/网络流量分析特征。



4. **Mythic 4.0 纯字节偏移文件传输规范 (Byte-Offset Transfer)**：
   - 摒弃传统基于 `total_chunks` 和 `chunk_num` 的序号分片，全面采用以 0 为起始基准的字节偏移量 `chunk_offset` 和总大小 `total_size`。
   - **外发/下载 (Agent -> Mythic)**：
     - 阶段 1（注册）：发送 `total_size`、`filename`、`full_path`、`is_screenshot`，服务端返回 `file_id`。
     - 阶段 2（推流）：循环发送 `file_id` + `chunk_offset` + Base64 编码的 `chunk_data`。
     - 断点续传（Resume）：发送 `file_id` + `resume: true`，服务端返回当前首个缺失字节偏移量 `chunk_offset`。
   - **下发/上传 (Mythic -> Agent)**：
     - Agent 收到下发任务后，向服务端请求 `{"upload": {"chunk_size": ..., "file_id": ..., "chunk_num": ...}}`，服务端下发 Base64 `chunk_data`，Agent 接收解码后落盘。

5. **统一 Hooking 响应结构演进**：
   - **Process Browser**：Mythic 4.0 规范为对象嵌套格式：
     `{"processes": {"host": "...", "os": "...", "update_deleted": bool, "processes": [...]}}`。
   - **File Browser**：目录枚举上报 `{"file_browser": {"host": "...", "parent_path": "...", "files": [...]}}`；文件删除上报 `{"removed_files": [{"host": "...", "path": "..."}]}`。
   - **Agent RPC**：Mythic 4.0 异步容器调用：`agent_rpc: {"name": "...", "arguments": "..."}`。


