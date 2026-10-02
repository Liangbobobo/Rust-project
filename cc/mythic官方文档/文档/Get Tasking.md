- [Mythic 4.0 核心笔记：Get Tasking（任务拉取与调度）](#mythic-40-核心笔记get-tasking任务拉取与调度)
  - [一、 任务请求报文（Message Request）](#一-任务请求报文message-request)
  - [二、 任务响应报文（Message Response）](#二-任务响应报文message-response)


# Mythic 4.0 核心笔记：Get Tasking（任务拉取与调度）

---

## 一、 任务请求报文（Message Request）

受控端向 Mythic 服务端请求拉取任务时的线缆报文格式：

```text
Base64( CallbackUUID + JSON(
{
    "action": "get_tasking",
    "tasking_size": 1, // 指定期望拉取的最大任务数量
    // 若代其他隔离受控端转发报文，包含以下字段：
    "delegates": [
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"},
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"}
    ],
    "get_delegate_tasks": true // 可选，默认为 true
}
)
)
```

---

**1. `tasking_size` 参数与异步调度机制**

该参数默认值为 `1`，允许 Agent 自行声明单次最多获取的任务数量。若 Agent 指定该值为 `-1`，则 Mythic 将一次性返回该 Callback 当前所有的待处理任务。

* **服务端机制（Mythic Core Go 源码与 PostgreSQL 交互层）**：
  * 通过 SQL 查询一次性取出该 Callback 当前排队的所有处于 `submitted` 状态的任务。
  * 服务端在将这些任务装配进响应报文的同时，通过数据库事务原子性地将这些任务的 `status` 字段从 `submitted`（已提交）更新为 `processing`（处理中），防止下一次轮询时被重复下发。
  * 服务端在单次加密响应的 `tasks` 数组中，将所有任务按时间戳顺序并列填入：
    ```json
    {
        "action": "get_tasking",
        "tasks": [
            { "id": "task-uuid-1", "command": "whoami", "parameters": "", "timestamp": 1578706611.1 },
            { "id": "task-uuid-2", "command": "ps",     "parameters": "", "timestamp": 1578706611.2 },
            { "id": "task-uuid-3", "command": "download", "parameters": "{\"file\":\"/etc/passwd\"}", "timestamp": 1578706611.3 }
        ]
    }
    ```
  * Mythic 服务端不提供、也不干涉受控端内部的线程或协程调度模型。协议保持运行时中立（不论 Agent 是用 Rust、Go、C、Python 还是 JXA 编写），服务端只负责将任务队列从数据库推送到线缆上。

* **受控端（Agent）内部的任务调度器实现**：
  * **模式 A：单线程 / 同步阻塞模型（无并发调度）**：
    * 若 Agent 没有并发设计，即使拿到了 3 个任务，也只能在主循环里通过顺序遍历串行执行：执行完任务 1，再执行任务 2，最后执行任务 3。
    * **缺陷**：若任务 2 是一个长期驻留任务（如 SOCKS 代理开启、大文件下载或端口扫描），整个 Agent 的心跳轮询主循环将被彻底阻塞（Hang），导致无法再向服务端发送任何心跳。
  * **模式 B：异步 / 多线程并发调度器（成熟 Agent 的标配）**：
    * 当受控端支持高并发（例如使用 Rust 编写并基于异步运行时环境时）：
    * **主心跳循环（Event Loop）只负责 I/O 调度**：心跳协程向服务端拉取到包含多个任务的 `Vec<Task>`。
    * **任务解耦与分发（Task Spawning）**：调度器遍历该数组，针对每一个 Task 派发（Spawn）一个独立的轻量级异步任务（或工作线程）去后台执行。
    * **非阻塞执行**：长周期任务与瞬时任务并行运行，主心跳循环完全不受影响，可以在下一个 Sleep 间隔继续正常进行心跳和任务拉取。

* **Mythic 天生异步友好**：
  * **强标识解耦（Task UUID Binding）**：每个下发的任务都具有独立的 `"id"`（Task UUID）。这使得 Agent 在汇报输出时，回传结构体（`post_response`）完全不需要按任务下发的顺序返回。
  * **乱序与增量回传（Out-of-Order Reporting）**：
    * 假设 Agent 同时启动了长任务 A（下载 10GB 文件）和短任务 B（`whoami`）：
    * 任务 B 可以优先在下一个心跳中标记 `completed: true` 并返回回执；
    * 任务 A 则可以在后续数十次心跳中，持续以相同 `task_id` 分块上报进度，两者互不干扰。

---

**2. `delegates` 参数与 P2P 覆盖网络机制**

> `delegates` 专为多跳中继（Pivoting & P2P Mesh）设计，它本质上就是 C2 在应用层自行构建的覆盖网络（Overlay Network）数据中继通道。所以 `delegates` 与“转发（Forwarding / Relaying）”深度绑定，正是因为内网隔离设备在网络拓扑与路由上根本不具备直接出网的能力，它们与 C2 之间的所有通信必然且只能依赖出网节点进行中继转发。

* **核心概念辨析：“位于内网” vs “无法出网”**：
  * **设备位于内网（Internal）**：
    * 设备分配的是 RFC 1918 私有 IP 地址（如 192.168.x.x、10.x.x.x、172.16.x.x），隐藏在 NAT（网络地址转换）或企业防火墙之后。
    * **通信特性**：入向（Inbound）受阻（外部 C2 服务器无法主动发起连接访问内网设备），但**出向（Outbound/Egress）允许**。绝大多数企业内网允许员工电脑或服务器访问互联网（如网页浏览 HTTP/HTTPS、DNS 查询）。当内网设备主动发起连接时，防火墙和 NAT 会自动建立并维护连接追踪表项（Conntrack），响应数据包即可顺畅返回。
  * **设备无法出网（No Egress / Air-gapped）**：
    * 设备处于极高安全级别的物理隔离网段、核心数据库区或严格限制出网规则的区域。
    * **通信特性**：防火墙在网络层/传输层彻底切断了该网段向互联网 IP 的任何出向连接。此类设备既无法被外界访问，也无法主动访问外界。

* **为什么绝大多数内网设备不需要 P2P 转发？**
  * 在实际企业网络环境下，绝大多数常规员工终端和业务服务器都属于“位于内网，但具备出网能力”。
  * 传统的 C2 通信模式全部采用**反向连接（Reverse Connection）**模型：不是 C2 服务端主动连 Agent，而是 Agent 定时主动轮询 C2。请求是由内网发起的标准出向 HTTPS 流量（走 443 端口），企业防火墙视为普通浏览流量放行。
  * 现代企业网络对出向流量高度依赖（SaaS、API、更新源），且 Agent 可以直接继承使用系统代理配置（如 Windows WinHTTP/WinINET 或 `HTTP_PROXY` 环境变量）。
  ```text
  【内网 Agent】 ──(主动出向 HTTPS 请求 443)──> 【NAT/防火墙】 ───> 【公网 C2 服务器】
      │                                                                   │
      └───────────────────(接收任务响应与回传结果)─────────────────────────┘
  ```

* **什么时候才真正需要 P2P 转发？**
  * 只有遇到真正的“无出网权限设备”时，P2P 转发才成为刚需：
    1. 核心隔离网段（DMZ 隔离区、数据库集群、工控 ICS 内部网段），禁止访问互联网任何 IP；
    2. 横向移动（Lateral Movement）场景：已获得一台可出网的双网卡/跳板机 A（运行 HTTP Agent），需要进一步控制不可出网的内网机器 B。机器 B 上的 Agent 无法连公网 C2，必须通过 SMB 命名管道或内网 TCP 建立与机器 A 的局域网连接，由机器 A 代理转发其流量。
  ```text
  【公网 C2】 <──(HTTP/HTTPS)──> 【跳板机 A (出网 Agent)】 <──(SMB 管道 / TCP)──> 【隔离机 B (无出网 Agent)】
                                    [P2P 中继/Delegate 节点]
  ```

* **建立节点间局域网信道（Inter-Agent P2P Link）与转发时序**：
  * **前置条件**：出网节点（Agent A）与内网隔离节点（Agent B）之间已通过横向移动命令（如 `link`）建立本地传输信道（Windows 下为 SMB 命名管道 `\\192.168.1.50\pipe\mythic_mesh`；Linux 下为内网原始 TCP Socket）。Agent A 维护常驻读写流句柄。
  ```text
    ┌──────────────────┐  ┌──────────────────┐     ┌──────────────────┐
    │ 内网隔离节点 (Agent B) │  │ 出网跳板节点 (Agent A) │     │ 外部 Mythic C2 服务端 │
    └──────────────────┘  └──────────────────┘     └──────────────────┘
              │                     │                        │
              │─1. 本地生成加密报文 agentMessage_B                    │
              │─2. 通过 SMB 管道写入字节流───►                        │
              │                     │                        │
              │                     │─3. 数据入队 (Buffer 暂存，不解密)│
              │                     │─4. 定期出网心跳触发            │
              │                     │─5. 发送外层用 Key_A 加密的报文──►
              │                     │   (含 B 的 delegates 报文)     │
              │                     │                        │
              │                     ◄9. 外部 HTTPS 响应 (Key_A 加密， │
              │                     │   含针对 B 的 delegates 回包)   │
              │                     │                        │
              ◄11. 将数据块通过 SMB 管道写回给 B                      │
              │   (B 用 Key_B 解密任务)                               │
  ```
  * **阶段 1：内网节点报文生成与本地传输（Local Ingestion）**：Agent B 生成标准报文 $\text{agentMessage\_B} = \text{Base64}(\text{CallbackUUID\_B} + \text{AES256\_KeyB}(\text{JSON\_B}))$，写入与 Agent A 相连的 SMB 管道或 TCP Socket。
  * **阶段 2：跳板节点的数据入队（Transparent Buffering）**：Agent A 后台线程读取管道输入。遵循**透明性原则（Zero-Knowledge / Opaque）**，Agent A 既没有 Agent B 的密钥，也不对数据解密，仅将其作为切片追加到内存的待转发代理队列：
    ```rust
    // 逻辑结构示意
    struct DelegateMessage {
        message: String,     // 原封不动的 agentMessage_B
        c2_profile: String,  // 如 "smb"
        uuid: String,        // Agent B 的 CallbackUUID
    }
    ```
  * **阶段 3：外网报文装配与出网（Piggybacking Outbound）**：Agent A 定期出网心跳触发时，将待转发队列数据填入 `delegates` 字段，用自身 `Key_A` 对整个 JSON 加密，拼接 `CallbackUUID_A` 发送给 Mythic。
    *(注：修正官方规范字段键名为 `"message"` 而非 `"agent_message"`，且包含 `"uuid"`)*：
    ```json
    {
      "action": "get_tasking",
      "tasking_size": 1,
      "delegates": [
        {
          "c2_profile": "smb",
          "message": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
          "uuid": "<CallbackUUID_B>"
        }
      ]
    }
    ```
  * **阶段 4：服务端递归解包与分发（Recursive Dispatching）**：Mythic 用 `Key_A` 解密外层报文，处理 Agent A 自身的任务；随后遍历 `delegates` 数组，根据 `uuid: CallbackUUID_B` 取出 `Key_B` 解密，生成针对 B 的回包数据：
    $$\text{response\_B} = \text{Base64}(\text{CallbackUUID\_B} + \text{AES256\_KeyB}(\text{Tasking\_for\_B}))$$
  * **阶段 5：反向路由下发（Reverse Relay Delivery）**：服务端在响应报文中将 `response_B` 填充进 `delegates` 数组，用 `Key_A` 加密发回给 Agent A。Agent A 解密后通过 SMB 管道写回给 Agent B，Agent B 用 `Key_B` 解密取出任务。
  * **技术总结**：出网节点充当了局域网内部 IPC 信道（命名管道/TCP）与外部 WAN 出网信道（HTTPS/DNS）之间的应用层透明网桥（Transparent Protocol Bridge）。

* **受控端实现边界：这些都需要自己手动在 Agent 实现吗？**
  * **不需要全部实现！** 如果目标是让受控端能够上线、执行命令、回传结果，只需要实现单一直连逻辑：
    * 编写一个基于 HTTP/HTTPS/WebSocket 的网络请求模块；
    * 发送 `get_tasking` 时，JSON 里**完全不写 `delegates` 和 `get_delegate_tasks` 字段**（字段是可选的，不填完全不影响）；
    * 将 `tasking_size` 写死为 `1`。每次心跳只拉取 1 个任务，执行完毕后在下一次通信中回传；
    * 结构极简：$\text{Sleep} \longrightarrow \text{HTTP POST 拉任务} \longrightarrow \text{本地执行} \longrightarrow \text{HTTP POST 报结果}$。
  * **完全可选项（P2P 级联转发）**：`delegates` 转发、本地命名管道/TCP 监听器、待转发缓冲区等全部属于深水区高级功能。常规出网 Agent 完全可直接无视。
  * **SDK 现状**：服务端有官方 `MythicContainer`（Go/Python）；受控端（客户端二进制）因平台差异及防病毒查杀考量，无统一下发的二进制运行时库，需由开发者独立编写或基于模板编写。

* **`get_delegate_tasks` 参数作用**：
  * 可选参数，默认为 `true`。指示当前 `get_tasking` 请求是否同时检查那些通过当前 Callback 路由可达的其他 Callback 的待处理任务。
  * **设计意义**：当内网级联的子节点自己会定期发送 `get_tasking`（主动 Pull 模型）时，若将其设为 `false`，可防止父节点在轮询时意外提前消费并丢弃子节点的任务，确保子节点的任务必须等待子节点自身的心跳到达 Mythic 时才被拉取。*(属于 P2P 进阶范畴，暂时无需深入)*

---

## 二、 任务响应报文（Message Response）

Mythic 服务端对 `get_tasking` 请求返回如下格式响应：

```text
Base64( CallbackUUID + JSON(
{
    "action": "get_tasking",
    "tasks": [
        {
            "command": "command name",
            "parameters": "command param string",
            "timestamp": 1578706611.324671, // 时间戳，用于受控端本地时序排序
            "id": "task uuid"
        }
    ],
    // 若中继了其他 Agent 的报文，返回对应的响应数据
    "delegates": [
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"},
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"}
    ]
}
)
)
```

* **`tasks` 数组**：始终为列表类型，包含 `0` 到 `tasking_size` 个任务条目。若无待执行任务，返回空列表 `[]`（充当标准心跳 Keepalive）。
* **`parameters` 字段**：封装命令的具体执行参数。如果命令包含复杂的结构化对象（例如 `{"remote_path": "/users/desktop/test.png", "file_id": "uuid_here"}`），在线缆传输中该字段**始终以转义后的 JSON 字符串（String 标量）**形式存在（即 task 对象中的 `parameters` 字段），受控端命令逻辑负责对其进行本地二次反序列化解析。
* **`delegates` 字段**：包含针对请求中所中继转发的子节点报文的服务端响应数据。

---

**核心战术：基于单次请求的捎带机制（Piggybacking）与消息多路复用（Multiplexing）**

> 这正是 Mythic 在半双工协议（如 HTTP/HTTPS）上实现“逻辑上的全双工（Pseudo Full-Duplex）”通信的核心机制。

* **概念辨析（纠错说明）**：
  * 原笔记中提出的 *“三次握手变成 2 次？”* 属于概念混淆。
  * 此处**并非指 TCP 传输层的三次握手（SYN, SYN-ACK, ACK）**，而是指 **应用层 C2 交互动作与网络往返（Round-Trip）的合并与精简**：
    * 传统 C2 模式通常划分 3 类独立动作：上线注册（`checkin`）、心跳拉任务（`get_tasking`）、回传输出（`post_response`）。每上报一次数据再拉取新任务需要发起两次独立的 HTTP POST 往返。
    * Mythic 允许在 `get_tasking` 请求中**捎带回传所有业务数据**。受控端在代码层面因此**只需实现 `checkin` 和 `get_tasking` 两类消息通道**，将“上报执行结果”与“拉取新指令”合并进单次 HTTP 往返中。无需在发送任务回执后连续紧接着再发一次拉取任务的请求，避免了因频繁回传大量任务响应而导致无法及时拉取到新命令。

* **单次请求的物理 JSON 结构（合并形态）**：
  ```json
  {
    "action": "get_tasking",
    "tasking_size": 1,

    // 1. 捎带回传之前命令的执行结果 (responses)
    "responses": [
      {
        "task_id": "uuid-1234",
        "user_output": "root\n",
        "completed": true
      }
    ],

    // 2. 捎带内网 SOCKS 代理流量 (socks)
    "socks": [
      {
        "server_id": 1,
        "data": "Base64EncryptedSocksData..."
      }
    ],

    // 3. 捎带交互式 PTY / 终端回显 (interactive)
    "interactive": [
      {
        "task_id": "uuid-5678",
        "data": "Base64ShellOutput..."
      }
    ]
    // 此外还可捎带 rpfwd(反向端口转发流量), edges(P2P路由拓扑), alerts(告警信息)
  }
  ```

* **服务端单次返回包（包含下行任务与下行代理流）**：
  ```json
  {
    "action": "get_tasking",
    "tasks": [
      {
        "id": "uuid-9999",
        "command": "ps",
        "parameters": "",
        "timestamp": 1578706611.5
      }
    ],
    "socks": [ ... ]
  }
  ```

* **对 Rust / Agent 开发者的工程实操意义**：
  1. **极大简化网络层代码与状态机**：
     * 根本不需要分别写 `fn send_response()` 和 `fn fetch_task()` 两个网络模块；
     * 只需抽象一个统一的 `fn do_heartbeat(responses: Option<Vec<Response>>)` 函数驱动心跳。
  2. **极佳的性能与免杀隐蔽性**：
     * 网络请求次数减少了 50%。在每一个 Sleep 周期到期时：
       * 检查本地是否有待提交的命令结果 / SOCKS 流量；
       * 有，就塞进 `get_tasking` 请求包中；没有，就只发空参数的 `get_tasking` 心跳请求；
       * 一次 HTTP POST，完成双向数据对调。
