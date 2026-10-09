针对 Wisconsin 项目，重构后的目录规范如下：

```text
C:\Users\liang\Desktop\Rust-project\cc\Wisconsin\Wisconsin\
├── Cargo.toml
├── src/
│   ├── lib.rs                  # 库根：声明 pub mod common, mythic, profiles, tasking
│   ├── main.rs                 # 二进制执行入口：引用 lib 中的核心组件
│   │
│   ├── common.rs               # 【底层基础设施与 OPSEC 模块声明】
│   ├── common/
│   │   ├── mem.rs              # 物理内存安全擦除：zeroize_string, zeroize_slice
│   │   ├── stack_alloc.rs      # 零堆分配栈容器：StackVec<T, N>, StackArena<CAP>
│   │   └── obf.rs              # 编译期混淆：ObfBuffer 状态机与 obfstr! 宏
│   │
│   ├── mythic.rs               # 【Mythic 官方协议规范模块声明与统一导出】
│   ├── mythic/
│   │   ├── error.rs            # ModelError：单字节 u8 错误代号
│   │   └── messages.rs         # ★ 纯净的 Mythic 官方固化契约 (Checkin, Response 等)
│   │
│   ├── profiles.rs             # 【C2 通信信道模块声明】
│   ├── profiles/
│   │   ├── crypto.rs           # AES-256-CBC + HMAC-SHA256 密码管线与常数预算
│   │   └── http.rs             # HTTP/HTTPS Profile 网络传输与明文暂存区装配
│   │
│   ├── tasking.rs              # 【Agent 本地任务分发与业务执行模块声明】
│   └── tasking/
│       ├── dispatch.rs         # 业务调度：is_long_running 预判、Worker 线程池派发
│       └── context.rs          # 传输状态上下文：ChunkTaskContext (流式分片校验与断点跟踪)
```

| 层级 | 路径 | 职责说明 |
|---|---|---|
| 根 | `Cargo.toml` | 项目清单与依赖声明 |
| 根 | `src/lib.rs` | 库根：声明 `pub mod common, mythic, profiles, tasking` |
| 根 | `src/main.rs` | 二进制执行入口：引用 lib 中的核心组件 |
| 模块声明 | `src/common.rs` | 底层基础设施与 OPSEC 模块声明 |
| 实现 | `src/common/mem.rs` | 物理内存安全擦除：`zeroize_string`, `zeroize_slice` |
| 实现 | `src/common/stack_alloc.rs` | 零堆分配栈容器：`StackVec<T, N>`, `StackArena<CAP>` |
| 实现 | `src/common/obf.rs` | 编译期混淆：`ObfBuffer` 状态机与 `obfstr!` 宏 |
| 模块声明 | `src/mythic.rs` | Mythic 官方协议规范模块声明与统一导出 |
| 实现 | `src/mythic/error.rs` | `ModelError`：单字节 `u8` 错误代号 |
| 实现 | `src/mythic/messages.rs` | ★ 纯净的 Mythic 官方固化契约（Checkin、Response 等） |
| 模块声明 | `src/profiles.rs` | C2 通信信道模块声明 |
| 实现 | `src/profiles/crypto.rs` | AES-256-CBC + HMAC-SHA256 密码管线与常数预算 |
| 实现 | `src/profiles/http.rs` | HTTP/HTTPS Profile 网络传输与明文暂存区装配 |
| 模块声明 | `src/tasking.rs` | Agent 本地任务分发与业务执行模块声明 |
| 实现 | `src/tasking/dispatch.rs` | 业务调度：`is_long_running` 预判、Worker 线程池派发 |
| 实现 | `src/tasking/context.rs` | 传输状态上下文：`ChunkTaskContext`（流式分片校验与断点跟踪） |