# Wisconsin Agent 开发准则与 OPSEC 规范 (Project Rules)

1. **禁止裸写 `#[derive(Debug)]`**：
   - 必须使用 `#[cfg_attr(any(debug_assertions, test), derive(Debug))]`。
   - 确保 Release 模式下彻底剥离 Debug 格式化符号与结构体字段反射，杜绝在 `.rdata` 节中留下敏感特征字符串。

2. **密码本式错误控制 (Error Codebook Pattern)**：
   - 所有模块错误必须统一收敛在 `src/error.rs` 中（或专用的错误码枚举）。
   - 采用 `#[repr(u8)]` 紧凑内存布局，赋予每个变体唯一的数字错误码（如 `50`, `51`）。
   - **严禁**在 Release 下实现 `core::fmt::Display` 或嵌入任何明文错误描述字符串。
   - 对外仅导出 `code(self) -> u8`。文档注释充当开发与运营人员的“离线密码本”，通过数字代号定位排错。

3. **零堆分配与纯栈优先哲学 (Zero-Heap / Stack-First)**：
   - 核心通信与数据处理热点路径杜绝 `String` / `Vec`，优先使用切片借用 `&'a str` 或定长栈缓冲区（如 `[u8; 36]`、`[u8; 260]`），规避 Windows 10/11 Segment Heap 碎片与取证追踪。
