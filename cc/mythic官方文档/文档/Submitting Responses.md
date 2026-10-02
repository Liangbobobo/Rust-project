
The main difference between submitting a response with a post_response and submitting responses with get_tasking is that in a get_tasking message with a responses key, you’ll also get back additional tasking that’s available同时还会获取到当前可用的新任务. With a post_response message and a responses key, you won’t get back additional tasking that’s ready for your agent. You can still get socks, rpfwd, interact, and delegates messages as part of your message back from Mythic, but you won’t have a tasks key. tasks（任务列表）键

两类 action 全部都是由客户端（受控端 Agent）主动发起、发送给服务端（Mythic
  Server）的上行请求动作

Displayed as code block. Widen terminal to view inline.

    flowchart TD
        subgraph 模式 A: action: post_response
            Agent1["Agent (发送已完成结果)"] -->|POST: 包含 responses| Server1["Mythic
  服务端"]
            Server1 -->|回包: 确认写入状态, 无 tasks 字段| Agent1
            style Agent1 fill:#f9f,stroke:#333,stroke-width:1px
        end

        subgraph 模式 B: action: get_tasking
            Agent2["Agent (拉任务并顺带报结果)"] -->|POST: 包含 responses 且
  action=get_tasking| Server2["Mythic 服务端"]
            Server2 -->|回包: 处理结果 + 下发新 tasks 数组| Agent2
            style Agent2 fill:#bbf,stroke:#333,stroke-width:1px
        end

特性维度         | action: "post_response"          | action: "get_tasking" (携带 resp…
  ------------------|----------------------------------|-----------------------------------
   发起方           | 客户端（Agent）                  | 客户端（Agent）
   客户端意图       | 纯上报模式：仅将本地已执行完毕的 | 多路复用模式：既上报之前的结果，
                    | 结果交付服务端。                 | 又索取后续的新任务。
   服务端处理逻辑   | 解析 responses                   | 解析 responses
                    | 写入数据库，不检索任务队列。     | 写入数据库，同时检索待执行任务队
                    |                                  | 列。
   回包内容         | 仅包含每个 task                  | 包含新拉取到的待执行命令数组（含
                    | 的接收状态（success/error），无  | tasks 键）。
                    | tasks 键。                       |
   双向代理流量支持 | 支持携带 socks / rpfwd /         | 支持携带 socks / rpfwd /
                    | delegates                        | delegates




 get_tasking 能够捎带回传，为什么协议不直接废弃
  post_response？这是为了满足**受控端流式输出与并发背压控制（Backpressure & Concurrency
  Control）**的需求：

  1. 分块增量输出（Chunked Streaming Output）：
      • 假设 Agent 正在执行一个长耗时命令（例如扫描整段 C 段端口，耗时 5
      分钟，产生大量屏幕日志）。
      • Agent 希望每扫描完 10
      台主机就向操作台实时刷新输出一部分日志，但此时该命令尚未执行结束。
      • 如果此时使用 get_tasking 回传部分日志，服务端会顺带塞给它 2 个新命令；而如果 Agent
      采用单线程串行架构，它就不得不中途打断当前扫描去处理新命令，导致状态机混乱。
      • 此时使用 post_response：Agent
      可以持续向外回传阶段性输出，且明确告知服务端**“我现在还在忙当前任务，不要给我下发任何
      新任务”**。
  2. 轻量错误即时上报：
      • 任务在初始化阶段发生致命崩溃或环境不兼容时，Agent 可通过单次轻量级 post_response
      直接终结该任务并报错，无需触发新一轮复杂的调度循环。



## Message Request

The contents of the JSON message from the agent to Mythic when posting tasking responses is as follows:
Base64( CallbackUUID + JSON(
{
    "action": "post_response",
    "responses": [
        {
            "task_id": "uuid of task",
            ... response message (see below)
        },
        {
            "task_id": "uuid of task",
            ... response message (see below)
        }
    ], //if we were passing messages on behalf of other agents
    "delegates": [
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"},
        {"message": agentMessage, "c2_profile": "ProfileName", "uuid": "uuid here"}
        ]
}
)
)


responses - This parameter is a list of all the responses for each tasking.
For each element in the responses array, we have a dictionary of information about the response. We also have a task_id field to indicate which task this response is for. 

responses 声明为一个 JSON 数组，意味着 Agent
      单次可以同时汇总并回传多个不同任务的执行片段
task_id 的上下文寻址（Task Attribution）：
      • 字典中的 "task_id" 对应此前由服务端在 get_tasking 中下发的 Task UUID。
      •
      服务端以此字段作为外键索引，将这段数据精确挂载到数据库中特定的任务实体下，解决并发任
      务乱序回传时的归属问题

checkin之后的第一次拉取任务,必须是get_tasking吗 可以用post_response吗?

After that though在那之后, comes the actual response output from the task.If you don’t want to hook a certain feature (like sending keystrokes, downloading files, creating artifacts, etc), but just want to return output to the user, the response section can be as simple as: {"task_id": "uuid of task", "user_output": "output of task here"}
功能挂钩（Hooking Features）” vs “通用输出（user_output）


功能挂钩（Hooking Features）
 Mythic 服务端并非单纯的文本日志收集器，它内置了大量高度结构化的专用功能子系统：
  • 文件下载系统（Download Subsystem）：负责分块接收、组合落盘，生成供操作员在 Web
  端下载的文件对象。
  • 文件浏览器（File Browser）：维护目标主机的树状文件目录结构。
  • 进程浏览器（Process Browser）：维护跨会话的目标系统进程树与 PID/PPID 关系。
  • 凭据库（Credentials Database）：结构化解析用户名、域、哈希/密码明文。
  • 键盘记录器（Keylogger）：按窗口标题聚合击键流。
  • 安全审计与痕迹（Artifacts）：记录在目标机上创建的文件、注入的线程、修改的注册表项

如果 Agent
  仅将结果输出为纯文本（user_output），服务端只将其视为无格式字符串渲染在命令行控制台中，上
  述子系统无法感知这些数据。
  Hooking Feature 即指：Agent
  在报文中抛弃（或补充）非结构化文本，转而提供符合服务端预设模式（Schema）的 JSON
  结构，直接“挂钩/注入”到对应的服务端子系统中

若受控端执行的命令需要触发并联动上述专用子系统，就必须在字典中携带对应功能的专属数据结构
  （例如文件下载需携带包含 total_chunks、chunk_num、data 的嵌套字典；上报凭据需携带
  credentials 结构体）

Hooking Features chapter

通用输出（user_output）
对于绝大多数常规命令行指令（如 whoami、pwd、ipconfig、cat、hostname、ls）：

  • 这些命令的目的仅仅是让前端操作员看到执行后的标准输出（stdout/stderr
  文本流），完全不需要触发任何服务端复杂的业务子系统。
  • 此时，受控端完全无需组装复杂的结构体，直接使用最纯粹的标量字段：
    {
        "task_id": "当前任务的UUID",
        "user_output": "命令执行返回的纯文本字符串"
    }

  • 服务端的处理动作：收到带有 user_output
  的数据后，服务端跳过任何中间件业务流，直接将文本原封不动地推送到数据库 response
  表中，并通过 GraphQL 订阅直接实时渲染在 Web 操作台该任务下方。

 最小可行性原型（MVP）极度扁平：
  在开发受控端初期，你完全不需要去实现复杂的凭据解析、进程树反序列化或文件分块协议。受控端
  只需实现一个最基础的通用回传模型：
    // Rust 示例数据结构
    struct PlainResponse {
        task_id: String,
        user_output: String,
        completed: Option<bool>, // 可选：声明任务是否完结
        status: Option<String>,  // 可选：声明执行状态 (如 "success" 或 "error")
    }

这一设计的提出，给 Agent 开发者提供了极大的实现自由度与渐进演进支持：

  1. 最小可行性原型（MVP）极度扁平：
  在开发受控端初期，你完全不需要去实现复杂的凭据解析、进程树反序列化或文件分块协议。受控端
  只需实现一个最基础的通用回传模型：
    // Rust 示例数据结构
    struct PlainResponse {
        task_id: String,
        user_output: String,
        completed: Option<bool>, // 可选：声明任务是否完结
        status: Option<String>,  // 可选：声明执行状态 (如 "success" 或 "error")
    }

  2. 命令输出与平台逻辑彻底解耦：
  普通的终端类命令执行逻辑仅需捕获操作系统的标准输出并转为 UTF-8 字符串填入
  user_output，即可完成交付，降低了受控端命令模块的开发门槛。


You can find many fields to send in the hooking features section, but outside of that you can set:
completed - boolean field to indicate that the task is done or not
status - string field to indicate the current status of the task. If the task completes successfully, you can set this to success, otherwise you can use it to indicate a generic error mesage to the user. If you start the status with error: then in the Mythic UI that status message will turn red to help indicate an error. Any other status you set will appear as blue text.
这两个字段直接控制了服务端与控制面板对该任务的生命周期状态机判定：

  #### 1. completed（布尔标量 bool）

  • 定义：显式声明该任务在受控端本地是否已经彻底终结（Terminal State）。
  • completed: false（默认）：
      • 适用于长耗时流式任务（如端口扫描、大文件下载、持续截屏）。
      • 数据库将该任务保持在 processing 活跃状态，后续 Agent 依然可以继续以该 task_id
      分块上报后续的增量输出。
  • completed: true：
      •
      声明该任务生命周期闭环。服务端收到后将其正式归档为终结状态，此后不再期望该任务产生后
      续输出。
status（字符串标量 String）

  • 定义：向平台与操作员传递简短的执行状态标识。
  • 如上所述，既充当了操作台状态栏的文案，又充当了前端 CSS 渲染的控制信号。


If you want to return a more verbose error message, then you can set completed: true, status: "error: auth failed, and then user_output: "some more complex output that displays in the body of the UI under the task where you can have much more room.暂时不需要深入

Each response style is described in Hooking Features. The format described in each of the Hooking features sections replaces the ... response message piece above所有的 Hooking
  特性（例如文件浏览器对象、凭据对象、进程快照）以及通用文本字段（user_output），在数据结构
  上不是嵌套在某个多层子对象中，而是作为键值对**直接平铺（Inlined）**在该任务字典中，替换掉
  原本示例里的 ...。
  • task_id 是不可变的主键锚点（Anchor Key），其余具体的业务载荷键（Payload
  Key）与它并列共存
这里的hooking feature是什么,工作机制是什么?怎么和responses耦合的?


To continue adding to that JSON response, you can indicate that a command is finished by adding "completed": true or indicate that there was an error with "status": "error".
在同一个任务响应字典内部，控制流元数据（Control Plane Metadata，如
  completed、status）与业务数据载荷（Data Plane Payload，如
  user_output、credentials）同级混入（Mix-in）。
  • 你不需要拆分请求，也不需要创建单独的控制报文，直接在该 JSON 对象中按需追加键值对即可
形态 A：仅回传部分增量输出（流式输出，未终止）
    {
        "task_id": "uuid-1234",
        "user_output": "Processing block 1...\n"
    }
  (不追加 completed，服务端状态机保持处理中)
  • 形态 B：普通指令终态交付（业务数据 + 控制字段叠加）
    {
        "task_id": "uuid-1234",
        "user_output": "Administrator\n",
        "completed": true,
        "status": "success"
    }

  • 形态 C：Hooking 特性 + 错误中断叠加（结构化数据替换 ... 并追加控制字段）
    {
        "task_id": "uuid-1234",
        "file_browser": { "files": [...] },
        "completed": true,
        "status": "error: access denied during traversal"
    }

  ──────
  ### 3. Rust 1.91 结构体建模

  在 Rust 1.91 中，该设计直接映射为一个通过 Option<T> 字段与 #[serde(skip_serializing_if =
  "Option::is_none")] 实现的扁平结构体（Flat Struct）：

    use serde::Serialize;

    #[derive(Serialize)]
    pub struct TaskResponse<'a> {
        // 基础主键
        pub task_id: &'a str,

        // 替换 "..." 的业务数据载荷（互斥或可选组合）
        #[serde(skip_serializing_if = "Option::is_none")]
        pub user_output: Option<&'a str>,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub credentials: Option<Vec<CredentialPayload<'a>>>,

        // 后续通过 "continue adding" 追加的控制状态字段
        #[serde(skip_serializing_if = "Option::is_none")]
        pub completed: Option<bool>,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub status: Option<&'a str>,
    }

  序列化时，未赋值的 Option::None 字段会被 Serde 自动剔除，生成的 JSON
  严格契合该单层内联协议。


delegates - This parameter is not required, but allows for an agent to forward on messages from other callbacks. This is the peer-to-peer scenario where inner messages are passed externally by the egress point. Each of these messages is a self-contained “Agent Message”.暂时不需要深入

未完待续


>Anything you put in user_output will go directly to the user to see. There’s no additional processing that happens. If you want to perform additional processing on the response, then instead of user_output use the process_response key. This will allow you to perform additional processing on whatever is passed through the process_response key - from here, if you want to register something for the user to see, you’ll need to use MythicRPCCreateResponse (you can use any MythicRPC at this point to register files, create credentials, etc).












































