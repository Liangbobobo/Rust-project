- [wiconsin 设计规范](#wiconsin-设计规范)
  - [关于no std](#关于no-std)
- [wisconsin 项目结构及各文件功能](#wisconsin-项目结构及各文件功能)
  - [lib.rs](#librs)
  - [`.cargo\config.toml`: 链接器底层配置](#cargoconfigtoml-链接器底层配置)
  - [profile](#profile)
- [Mythic 4.0 官方文档严格规定的部分](#mythic-40-官方文档严格规定的部分)
- [Thanatos-Wisconsin](#thanatos-wisconsin)
  - [Thanatos-Wisconsin的网络发包逻辑](#thanatos-wisconsin的网络发包逻辑)
  - [Thanatos-Wisconsin 的任务分发主循环（如何接收命令）](#thanatos-wisconsin-的任务分发主循环如何接收命令)
  - [Thanatos-Wisconsin 的 Mythic 界面描述文件（直接复制）](#thanatos-wisconsin-的-mythic-界面描述文件直接复制)
  - [coffeeldr(BOF)的功能](#coffeeldrbof的功能)
  - [Rust Agent功能](#rust-agent功能)
  - [Thanatos](#thanatos)
  - [三件套的角色](#三件套的角色)
- [项目落地的流程和组件协同情况](#项目落地的流程和组件协同情况)
  - [1：编译打包阶段（在你的开发机上）](#1编译打包阶段在你的开发机上)
  - [2：投放与启动阶段（目标设备）](#2投放与启动阶段目标设备)
  - [3：隐蔽运行与 C2 通信阶段（三件套在 Agent 内部生效）](#3隐蔽运行与-c2-通信阶段三件套在-agent-内部生效)
  - [4：战术扩展阶段（coffeeldr 动态工作）](#4战术扩展阶段coffeeldr-动态工作)
  - [总结](#总结)

# wiconsin 设计规范

## 关于no std
核心Payload / Loader模块：必须使用 no_std.执行核心恶意逻辑的部分，如shellcode加载、进程注入、直接系统调用等。将这些模块置于no_std环境下，可以最大化地减小体积、消除特征，并实现高级规避技术

辅助功能模块：可以保留 std
对于非核心的辅助功能，如配置解析、日志记录（尽管在免杀中应谨慎）、命令行参数处理等，可以保留std以利用其便利性，加快开发速度。这些模块的特征可以通过其他手段（如字符串混淆）来单独处理。

构建与配置管理：利用 Cargo 特性（Features）
通过Cargo的features机制，可以轻松地在std和no_std之间进行条件编译。例如，可以为项目定义一个std特性，在开发调试时启用，在发布免杀版本时禁用，从而在开发效率和最终产物隐蔽性之间取得最佳平衡。
还需要继续讨论到底什么时候使用 no std

# wisconsin 项目结构及各文件功能
Mythic 官方支持任何语言编写的 Agent,官方只规定了服务端容器如何构建、以及网络物理帧如何交互，对客户端工程内部的源代码目录组织结构（Source Code Architecture）不做任何强行规定.

但社区会有惯用模式,比如将profile文件夹来封装网络和加解密逻辑.本项目将沿用此模式

## lib.rs
lib.rs应该是门面,是 crate 的根（crate root），它应该尽量薄，扮演门面 / 组装层的角色，而不是承载具体实现细节.在rust架构中lib核心负责:
1. 声明模块树(pub mod)
2. 暴露公共api(pub use),如 pub use crypto::AesKey;
3. crate 级属性与平台入口胶水.如#![cfg_attr(not(debug_assertions), no_std)]及承载Windows dll的入口胶水代码(dllmain)
4. 少量 crate 级公共常量:如果某个常量确实是整个 crate 的公共契约（例如版本号、全局配置默认值），可以放在 lib.rs 并pub use.但如果是实现细节（如缓冲区大小、分片阈值），就不应该放这里
因此,之前将4kb/16kb等缓冲区常量及其他常量函数放入lib.rs中是不合适的

根据本项目应将`#![no_std]`写入lib.rs或main.rs中的顶部:
- 它会影响整个 crate，所有模块默认都不链接标准库
- 如果某个模块（或测试）需要 std，可以在该模块内单独写 extern crate std;，或者用 `#[cfg(test)] extern crate std`; 在测试时启用

`#![cfg(...)]` 是 crate 级，会条件编译整个 crate，不是你想要的效果。
`#[cfg(...)]` 是 item 级，必须附着在每一个需要条件编译的 item 上
因此`#[cfg(any(debug_assertions, test))]`必须对每个item逐个标注


## `.cargo\config.toml`: 链接器底层配置

- 为 `x86_64-pc-windows-msvc` 目标配置 `rustflags = ["-C", "target-feature=+crt-static"]`，强制静态链接 C 运行库（MT 模式），彻底消除目标机对 `VCRUNTIME140.dll` 等依赖。
- 为 `x86_64-pc-windows-gnu` 配置 `rustflags = ["-C", "link-arg=-s"]` 自动剥离符号。

Windows 平台上，用 MSVC 工具链编译的 C/C++/Rust 程序，底层运行时代码（如基础数学运算、内存分配初始引导、字符转换、SEH 结构化异常处理）均由 Microsoft C Runtime Library (CRT) 提供
- 默认动态链接 (-crt-static / MD 模式)：
  - 生成的 PE 二进制文件（EXE/DLL）的导入表（Import Directory Table, IAT）中会硬编码记录依赖动态库：`VCRUNTIME140.dll`、`MSVCP140.dll`、`ucrtbase.dll` 等。
  - 致命缺陷：当程序在目标机上运行时，Windows 映像加载器（`ntdll!LdrLoadDll -> LdrpProcessStaticImports`）会在系统目录中寻找这些 DLL。如果目标主机是一台未安装对应版本 Visual C++ Redistributable 分发包的纯净 Windows 10/11 或精简版服务器，加载器将直接报错：
    > “由于找不到 VCRUNTIME140.dll，无法继续执行代码。重新安装程序可能会解决此问题。”
  - 在无交互式桌面（如通过计划任务、WMI、PsExec、服务或横向移动权限维持）运行场景下，系统不会弹窗，而是直接发生静默崩溃闪退（NTSTATUS 退出码 `0xC0000135 / STATUS_DLL_NOT_FOUND`）。
- 强制静态链接 (+crt-static / MT 模式)：
  - 编译器与链接器（link.exe）会将 Rust 依赖的 CRT 静态目标文件（`libcmt.lib`、`libvcruntime.lib`、`libucrt.lib`）中实际用到的机器码直接拷贝并嵌入最终二进制文件的 `.text` 节区内。
  - 零外部依赖：生成的 EXE 或 DLL 拥有 100% 自给自足的运行时，在任意未装任何 VC++ 运行库的 Windows 10 / Windows 11 或 Windows Server 纯净裸机上均可即拷即运行。
但这会增加二进制文件体积

## profile
crypto.rs:加密和解密
符合mythic 4.0规范的加解密


# Mythic 4.0 官方文档严格规定的部分

1. 服务端的容器结构

https://docs.mythic-c2.net/version-4.0/customizing/payload-type-development/payload-type-info/payload-type-definition

在服务端的 Docker 容器中，必须有且仅有以下标准组件供 mythic_container 识别：

- Dockerfile：定义该 Agent 编译所需的系统环境（如 Debian + Rust 1.94 工具链）；
- main.py：容器服务入口，执行 `mythic_service.start_and_run_forever()`；
- mythic/agent_functions/builder.py：必须继承 `PayloadType` 类，声明：
  - `name = "wisconsin"`
  - `supported_os = [SupportedOS.Windows]`
  - `c2_profiles = ["http"]`
  - `agent_code_path`：指向存放客户端源码的路径（例如 `./wisconsin/agent_code`）
  - `async def build(self) -> BuildResponse`：接收 Web 前端操作员选择的参数（IP、端口、密钥等），执行实际编译构建命令行（如 `cargo build --release`）并返回编译产物；
- mythic/agent_functions/<command_name>.py：每个命令必须继承 `CommandBase`（如 `sleep.py`、`ls.py` 等）。

官方对客户端源码（`agent_code_path` 指向的内容）的全部规定就只有一句话：它只要能被你的 `builder.py` 里的编译脚本编译成最终可执行文件即可，其内部代码树完全由开发者所选的语言工具链（如 Cargo）自由决定。

2. 网卡传输的通信报文格式

https://docs.mythic-c2.net/version-4.0/customizing/payload-type-development/create_tasking/agent-side-coding/agent-message-format

官方文档对 Agent 客户端真正做出的“不可违背的硬性规定”，是数据包物理帧规范：

```text
base64(
    UUID (36字节ASCII 或 16字节大端二进制) +
    EncBlob(
        JSON({
            "action": "checkin | get_tasking | post_response",
            "...": ...,
            "delegates": [...]
        })
    )
)
```

- 只要 Agent 发送到 HTTP 端的报文满足上述结构（标准 AES-256-CBC + PKCS#7 + HMAC-SHA256），无论这部分逻辑在客户端源码里叫什么文件、放在哪个文件夹，Mythic 服务端都能精准解密与处理。


# Thanatos-Wisconsin

Thanatos是用rust实现的Agent模板,主要研究其和mythic服务器连接,任务分发,mythic界面描述文件

**重构项目应以library为内核,binary为外壳**
1. lib方便测试和调试:发包/加解密/序列化全写在binary(exe程序的main.rs文件中),后续的单元测试(cargo test)或本地mock测试非常别扭.把核心逻辑写在lib.rs中,可随时写测试函数断言发包结果
2. 便于后续变形打包:后期不仅需要.exe文件,还需要把agent编译成动态库.dll去配合dll劫持或反射加载.如果核心是lib,只需要修改cargo.toml中的`crate-type=["cdylib"]`即可编译为dll
3. Rust原生支持二者共存:lib.rs作为核心通信库,main.rs作为可执行文件的入口,只负责调用lib.cargo run时,Rust会自动编译运行main.rs生成exe.
4. 一个 package 可以包含多个 crate(完整的编译单元)：一个 library crate（可选,(根文件通常是src/lib.rs,且只能有一个lib.rs)）和多个 binary crate
5. 一个项目package可以包含一个或多个二进制crate:每个二进制crate对应一个入口文件.默认情况下,src/main.rs是其中一个,但可通过src/bin目录下的多个.rs文件或cargo.toml中的`[[bin]]`配置添加更多二进制目标.即一个package可包含多个binary crate,每个binary crate都有自己的入口文件,这些入口文件通常放在src/bin目录下,并且每个文件都可以包含main函数.



## Thanatos-Wisconsin的网络发包逻辑

Payload_Type/thanatos/thanatos/agent_code/src/profiles/http.rs

其文件组织架构如下:
1. 


## Thanatos-Wisconsin 的任务分发主循环（如何接收命令）

Payload_Type/thanatos/thanatos/agent_code/src/tasking.rs

## Thanatos-Wisconsin 的 Mythic 界面描述文件（直接复制）

Payload_Type/thanatos/thanatos/mythic/agent_functions/builder.py



## coffeeldr(BOF)的功能

各种后渗透功能(如目录查看,凭据查寻,域枚举等),使用coffeeldr + 社区 BOF 生态

单次执行/用完即走/查询类信息收集动作.如:
• 文件与目录操作（dir, cat）
• 进程与网络查看（ps, netstat, arp）
• Active Directory 域枚举（ldapsearch, adcs_enum）
• 凭据与票据收集（kerberoast, klist）

## Rust Agent功能

长效运行、需要维护持续状态、或者涉及平台核心隐蔽能力的逻辑

• C2 心跳与网络通信主循环（必须原生）
• 三件套防护（dinvk API 寻址、uwd 栈伪造、hypnus 休眠加密）
• 长时间后台工作（如端口转发 Redirector、SOCKS 代理）
• 跨平台特有功能（如针对 Linux 的 SSH/SFTP 原生客户端，因为 BOF规范主要是 Windows/COFF 架构）

实际对抗中,Agent只保留通信 + 三件套 + coffeeldr.所有后渗透战术交给社区的BOF生态.

## Thanatos

Thanatos是一个别人写好的agent,但是完全没有三件套的隐蔽功能.

但有与mythic的通信逻辑/心跳循环等落地实践

## 三件套的角色

三件套不能单独在目标设备上运行,跑在目标设备上的程序是agent(植入体/载荷),三件套是打包编译在agent里面的隐蔽护甲代码

# 项目落地的流程和组件协同情况

## 1：编译打包阶段（在你的开发机上）

1. 你写好了 Rust 源码，把 dinvk、uwd、hypnus、coffeeldr 作为 mod或依赖引入。
2. 输入 cargo build --release 进行编译。
3. 编译器把主循环、通信逻辑、三件套代码揉合、压缩、编译成一个单独的、几百KB 的二进制程序 agent.exe

## 2：投放与启动阶段（目标设备）

1. agent.exe 被加载到目标 Windows 设备的内存中运行
2. agent.exe 启动：主线程开始工作

## 3：隐蔽运行与 C2 通信阶段（三件套在 Agent 内部生效）

1. API 解析：Agent 需要获取主机名、用户名、发起 HTTP 连接。此时 dinvk生效，Agent 绝不上导入表（IAT）查函数，而是隐蔽地从内存动态获取 API 地址。
2. 高危调用：Agent 向 C2 发包或调用敏感 API。此时 uwd生效，临时拦截并虚构调用栈，让目标机上的 EDR 以为是 kernel32.dll在正常发包。
3. 网络连线：Agent 向远端的 Mythic C2 服务端 发送 HTTP心跳：“报告指挥部，我已经隐蔽上线，有新任务吗？”
4. 休眠保护：如果 Mythic 回复“无任务，休眠 30 秒”。Agent 准备休眠，此时hypnus 生效，将 Agent 自身的内存页设为不可执行并加密，EDR进行周期性内存扫描时完全扫不到特征

## 4：战术扩展阶段（coffeeldr 动态工作）

1. 30 秒后，hypnus 自动唤醒 Agent，Agent 再次向 Mythic 发心跳。
2. 在 Mythic 的 Web 界面敲下命令：执行 ldapsearch.o。
3. Mythic 将 ldapsearch.o 的字节流发给 Agent。
4. Agent 接收到字节流后，将其传给内部的 coffeeldr 模块。
5. coffeeldr 在当前 Agent 内存中临时划出一块地方，把 ldapsearch.o解析并跑起来，查询域控信息。
6. 查询完毕，coffeeldr 收集结果传给 Agent，Agent 发给 Mythic。
7. coffeeldr 立刻销毁并释放这块临时内存，不留任何痕迹

## 总结

1. 目标设备上运行的只有一个东西：就是你编译出的 Rust Agent。
2. 三件套（dinvk/uwd/hypnus）是写在 Agent 内部的算法与防护逻辑，用来保证Agent 在运行和休眠时绝对不被 EDR 抓到。
3. Mythic 跑在你自己服务器上，用来在浏览器里接收 Agent 的心跳和下发指令。
4. Thanatos 只是一个别人写好的 Agent例子，你不用它本身，只是借用它的网络接口定义来让你的 Agent 能和 Mythic对话。
5. coffeeldr 是写在 Agent 内部的一个解析器，用来跑社区开源的 BOF 工具