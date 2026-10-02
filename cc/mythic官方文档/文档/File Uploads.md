# File Uploads (Mythic -> Agent)

Upload a file from Mythic to the target

---

## What does it mean to upload a file

> beacon用coffeeldr转换后在agent上执行必经之路 

uploading a file from the Mythic server to an agent.
Because these messages aren’t just to an agent directly (they can be routed through a p2p mesh), they can’t just be as simple as a GET request to download a file. 
The file needs to be chunked and routed through the agents. **This isn’t specific to the upload command, this is for any command that wants to leverage利用 a file from Mythic.**

---

In general, there’s a few steps that happen for this process (this can be seen visually形象的/可视的 on the Message Flow page):下面的这些步骤都是基于mythic的web ui发生的吗?不是可以基于其他形式

1. The operator issues some sort of tasking that has a parameter type of “File”. You’ll notice this in the Mythic user interface because you’ll always see a popup for you to supply a file from your file system.
   > 文件上传协议（Mythic → Agent，文件下发/植入） 的初始触发阶段操作员交互与资产预载

2. Once you select a file and hit submit, Mythic loops through all of the files selected and registers them. This process sends each one down to Mythic, saves it off to disk, and assigns it a UUID. These UUIDs are what’s stored in place of the raw bytes when you submit your task. So, if you had an upload command that takes a file and a path, your arguments would end up looking like {"file":"uuid", "path": "/some/path"}(位于 agent发起get_tasking轮询后,服务端回包的tasks数组中的parameters中,如"parameters": "{\"file\":\"7b98a091-c27b-48d6-953e-2fa942858b19\",  \"path\":\"/some/path\"}") rather than {"file": raw bytes of file, "path": "/some/path"}.

3. In the Payload Type’s corresponding command python file there is a function called create_go_tasking that handles the processing of tasks from users before handing them off to the database(服务端内部的 PostgreSQL,agent没有任何数据库) to be fetched by an agent. If your agent supports chunking and transferring files that way, then you don’t need to do anything else, but if your agent requires that you send down the entire file’s contents as part of your parameters, you need to get the associated file.如果不支持流式分片拉取状态机,则由Python 端的 Python 端的 create_go_tasking处理.但指受控端（Agent）的代码逻辑是否实现了流式分片拉取状态机,这是现代agent必须支持的,所以不需要深入Python 端的 create_go_tasking函数
   > 操作员在 Web UI(只能用web ui吗,有没有命令行或其他方式?) 发出指令；
   > 1. 服务端先把任务派发给该载荷类型专属的 Python 容器中间件，调用其 create_go_tasking 预处理钩子函数；
   > 1. Python 钩子处理完毕后，将任务交付给 服务端的 PostgreSQL 数据库（进入任务就绪队列）；
   > 2. 随后，Agent 发起周期性心跳（get_tasking）时，服务端核心服务再从服务端的 PostgreSQL 数据库中检索出该任务，加密下发给 Agent

4. To get the file with Mythic.mythic必须使用RPC（Remote Procedure Call，远程过程调用） 是一种进程间/网络间通信协议机制。
   它的核心语义是：让运行在一个进程或容器中的程序，能够像调用本地函数一样，去执行位于另一个 独立服务（远程节点）上的函数或方法，并透明地接收返回值 来处理Mythic 后端采用的 Docker 容器化微服务架构的多个容器.
   
   命令预处理代码（Python 脚本）运行在独立的 Payload Type 容器 中；
   • PostgreSQL 数据库与文件存储系统由独立的 Mythic Core（Go 核心服务）容器 独占管理；
   • Python 容器没有直接访问宿主机文件存储或直连数据库写表的权限（存在容器命名空间与网络隔离）
   
   以上,重构中使用了 rust agent处理实现分片,这里完全不需要深入，直接跳过即可

   > At this point, if you wanted to use the raw bytes of a file instead of the UUID as part of your tasking, you should use the Get File Contents example above to fetch the actual contents. Then you can then swap out the contents of the parameter with taskData.args.add_arg("arg name", "base64 of file contents here")

5. The agent gets the tasking and sees that there’s a file UUID it needs to pull. It sends an initial message to Mythic saying that it will be downloading the file in chunks of a certain size and requests the first chunk. If the agent is going to be writing the file to disk (versus与...相对 just pulling down the file into memory), then the agent should also send full_path. This allows Mythic to track a new entry in the database with an associated task for uploads.

6. The Mythic server gets the request for the file, makes sure the file exists and belongs to this operation, then gets the first chunk of the file as specified by the agent’s chunk_size and also reports to the agent how many chunks there are. 6. 

7. The Agent can now use this information to request the rest of the chunks of the file.
   > The agent reporting back full_path(Agent 向服务端回传的 responses 数组中的 upload 嵌套字典内,要是 Agent向服务端上报数据，使用的永远是 responses 数组) is what allows Mythic to track the file in the Files search page as a file that has been written to disk. If you don’t report back a full_path or have full_path as an empty string, then Mythic thinks that the file transfer only lived in memory and didn’t touch disk. This is separate from reporting that a file was written to disk as part of artifact tracking on the Reporting Artifacts(红蓝对抗运维取证子系统) page.

---

There is no expectation when doing uploads or downloads that the operator must type the absolute path to a file, that’s a bit of a strict requirement. Instead, Mythic allows operators to specify relative paths and has an option in the upload action to specify the actual full path (this option also exists for downloading files so that the absolute path can be returned for better tracking). This allows Mythic to properly track absolute file system paths that might have the same resulting file name without an extra burden on the operator.

不强求操作员每次都输入绝对路径,但服务端必须依赖全局绝对路径进行资产追踪.Mythic 采取操作员传相对路径，Agent 解析并回报绝对路径.这也是agent自己实现的功能

自研 C2 Agent，本质上是在目标操作系统上编写一个专用的、高度健壮的“远程无状态/有状态运维运行时.在整个生命周期中，除了传输层加密报文外，以下所有底层脏活累活全部需要你在 Rust 中手动实现：

• 路径规范化：相对路径计算、符号链接穿透、环境变量扩展（如 %TEMP% 展开）；
• I/O 调度与背压：文件异步读写句柄、切片滑动窗口、缓冲区溢出控制；
• 进程与内存原语：如果是内存加载执行，需要自己手写内存映射（VirtualAlloc / mmap）、重定位与线程注入；
• 错误边界隔离：捕获 OS 原生错误码（如 ERROR_ACCESS_DENIED、ENOENT），转换为 Mythic 协议能识别的错误状态并回传

It’s not an extremely complex process, but it does require a bit more back-and-forth than a fire-and-forget style.不复杂,更多的需要前后联系而是不是写完就扔那里就能运行的知识点.

---

## Example (agent pull down)

Files can (optionally) be pulled down multiple times (if you set **delete_after_fetch** as True, then the file won’t exist on disk after the first fetch and thus can’t be re-used). This is to prevent bloating up the server with unnecessary files.

An agent pulling down a file to the target is similar to downloading a file from the target to Mythic. The agent makes the following request to Mythic:

```json
{
    "action": "post_response",
    "responses": [
    {
        "upload": {
            "chunk_size": 512000, //bytes of file per chunk
            "file_id": UUID, //the file specified to pull down to the target
            "chunk_num": #, // which chunk are we currently pulling down
            "full_path": "full path to uploaded file on target" //optional
        },
        "task_id": task_id // the associated task that caused the agent to pull down this file
    }]

}
```

> The chunk_num field is 1-based. So, the first chunk you request is "chunk_num": 1

The full_path parameter is helpful for accurate tracking. This allows an operator to be in the /Temp directory and simply call the upload function to the current directory, but allows Mythic to track the full path for easier reporting and deconfliction.

> The full_path parameter is only needed if the agent plans to write the file to disk. If the agent is pulling down a file to load into memory, then there’s no need to report back a full_path.

The agent gets back a message like:

```json
{
    "action": "post_response",
    "responses": [ {
        "status": "success or error",
        "error": "error message if status is error, otherwise key not present",
        "total_chunks": #, // given the previous chunk size, the total num of chunks
        "chunk_num": #, //the current chunk number Mythic is returning
        "chunk_data": "base64_of_data", // the actual file data,
        "file_id": "file id that was requested",
        "task_id": "UUID of task" // task id that was presented in the request for tracking
        }
    ]
}
```

**错误控制:**

This process repeats as many as times as is necessary to pull down all of the contents of the file.

> If there is an error pulling down a file, the server will respond with as much information as possible and blank out the rest (i.e.: {'action': 'post_response', 'responses': [ {'total_chunks': 0, 'chunk_num': 0, 'chunk_data': '', 'file_id': '', 'task_id': '', 'status': 'error', 'error': 'some error message'} ] }) If the task_id was there in the request, but there was an error with some other piece, then the task_id will be present in the response with the right value.

---

## Files in the Tasking JSON

There’s a reason that files aren’t base64 encoded and placed inside the initial tasking blobs. Keeping the files tracked by the main Mythic system and used in a separate call allows the initial messages to stay small and allows for the agent and C2 mechanism to potentially cache or limit the size of the transfers as desired.

Consider the case of using DNS as the C2 mechanism. If the file mentioned in this section was sent through this channel, then the traffic would potentially explode. However, having the option for the agent to pull the file via HTTP or some other mechanism gives greater flexibility and granular control over how the C2 communications flow.

Mythic 之所以没有将文件直接进行 Base64 编码并内嵌到初始任务数据块.将文件交由 Mythic核心系统进行统一跟踪管理，并通过独立的接口调用按需获取，不仅能保证初始下发报文保持轻量，还赋予了 Agent 与 C2通信机制按需实施本地缓存或精细化限制传输速率与分片大小（流量控制）的能力.

设想采用 DNS 隧道作为 C2 通信信道的场景.如果将本节所讨论的文件直接通过该信道下发，其网络报文数量可能会瞬间激增，引发“流量爆炸”.然而，通过允许 Agent 灵活选择通过 HTTP 或其他替代信道主动拉取该文件，为整个 C2通信体系在流量调度和数据流向控制上提供了更高的弹性与更精细的粒度

---

## 设计 Rust对应的分片写入状态机
