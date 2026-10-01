- [models.rs中各结构体之间的联系和数据流向](#modelsrs中各结构体之间的联系和数据流向)
- [各结构体之间的核心联系](#各结构体之间的核心联系)
- [Ref\<'a\> 零拷贝结构体与 Owned 结构体的区别与设计意图](#refa-零拷贝结构体与-owned-结构体的区别与设计意图)



## models.rs中各结构体之间的联系和数据流向

| 分类 | 结构体 / 元素 | 类型 | 数据流向 | 说明 |
|---|---|---|---|---|
| 错误控制 | `ModelError` | 单字节 `u8` 判别值 | ← 各结构体返回错误 | 统一错误类型 |
| 外部暂存区 | `raw_buf: &mut [u8]` | 解密明文暂存区 | ──借用 `&'a str`──→ `ServerCheckinResponse<'a>` / `ServerTaskingResponseRef<'a>` / `MythicTaskRef<'a>` | 借用来源 |
| 请求阶段 | `CheckinMessage<'a>` | 零拷贝借用 | ──返回错误──→ `ModelError` | 上线注册请求 |
| 请求阶段 | `GetTaskingMessage<'a>` | 栈构造 / Default | — | 心跳轮询请求 |
| 请求阶段 | `PostResponseMessage<'a>` | 批量回执外壳 | ──包含引用切片 `&'a [TaskResponseItem]`──→ `TaskResponseItem<'a>` | 任务回执请求 |
| 请求阶段 | `TaskResponseItem<'a>` | 单条输出 / 文件分片 | ──返回错误──→ `ModelError` | 单条任务结果 |
| 请求阶段 | `FileChunkPayload<'a>` | 大文件专属载荷 | ──返回错误──→ `ModelError` | 大文件分片传输 |
| 响应解析 | `ServerCheckinResponse<'a>` | 零拷贝引用 | `raw_buf` ──借用──→ 本结构体；本结构体 ──`copy_id_to()`──→ `PersistentID [u8; 36]`；本结构体 ──返回错误──→ `ModelError` | 上线响应 |
| 响应解析 | `ServerTaskingResponseRef<'a>` | 零拷贝引用 | `raw_buf` ──借用──→ 本结构体；本结构体 ──包含 `Vec`──→ `MythicTaskRef<'a>` | 轮询响应（推荐） |
| 响应解析 | `MythicTaskRef<'a>` | 零拷贝引用 | `raw_buf` ──借用──→ 本结构体；本结构体 ──`to_owned()`──→ `MythicTask`；本结构体 ──返回错误──→ `ModelError` | 零拷贝任务项 |
| 响应解析 | `ServerTaskingResponse` | 所有权 | 本结构体 ──包含 `Vec`──→ `MythicTask`；本结构体 ──`Drop`──→ `zeroize_string` | 轮询响应（异步） |
| 响应解析 | `MythicTask` | 所有权 | 本结构体 ──`Drop`──→ `zeroize_string` 内存刷零 | 所有权异步任务 |
| 转换 | `MythicTaskRef` → `MythicTask` | `to_owned()` | `MythicTaskRef` ──`to_owned()`──→ `MythicTask` | 跨线程派发时转换 |
| 转换 | `ServerCheckinResponse` → `PersistentID` | `copy_id_to()` | `ServerCheckinResponse` ──`copy_id_to()`──→ `PersistentID [u8; 36]` | 拷贝 Callback UUID 至静态缓冲区 |
| RAII 洗白 | `MythicTask` | `Drop` Trait | `MythicTask` ──`Drop`──→ `zeroize_string` | 内存刷零 |
| RAII 洗白 | `ServerTaskingResponse` | `Drop` Trait | `ServerTaskingResponse` ──`Drop`──→ `zeroize_string` | 内存刷零 |


## 各结构体之间的核心联系

- 生命周期锚定（Lifetime Invariance）：CheckinMessage<'a>、ServerCheckinResponse<'a>、ServerTaskingResponseRef<'a> 和 TaskResponseItem<'a> 都带有生命周期参数'a。它们直接借用了网络层在栈上解密出的 raw_buf 明文切片
- 数据结构组合（Aggregation）：
      • PostResponseMessage<'a> 并不是直接装载数据，而是通过 &'a `[TaskResponseItem<'a>]` 切片引用装载多个回显条目。
      • ServerTaskingResponseRef<'a> 内部包含零拷贝的`Vec<MythicTaskRef<'a>>`
-  状态与转型（Transformation）：
      • ServerCheckinResponse<'a> 解析后，通过 copy_id_to 函数将 UUID提取至外部静态数组 [u8; 36] 中，使 raw_buf 能被立刻洗白
      • MythicTaskRef<'a> 通过 .to_owned() 转为拥有所有权的MythicTask，完成从**同步短生命周期向异步长生命周期**的跨越

## Ref<'a> 零拷贝结构体与 Owned 结构体的区别与设计意图