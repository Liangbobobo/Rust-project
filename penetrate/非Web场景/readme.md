# 非Web场景

不开HTTP端口,不开Web服务场景下,主要是“原始协议对抗” 与“客户端漏洞投送”这种最难的渗透场景

# 工具

## 外部侦察

Naabu:快速找到 3389 (RDP), 22 (SSH), 445 (SMB), 5985 (WinRM), 1723(PPTP) 等管理端口

ZGrab2 (Censys 的底层)：处理非 HTTP 协议握手最强的工具。它支持 SMB, SSH, Telnet, RDP的深度 Banner 抓取

## 漏洞验证与突破阶段

Nuclei (网络层模板)：Nuclei 的 network 目录下的 YAML 模板。它能探测旧版 RDP的漏洞、SMB 的空会话、或是特定 VPN 服务的配置缺陷

Responder / Pre-Auth Exploits：如果目标是家庭 PC，你可以在公网或相邻网段尝试 NTLM Relay (中继攻击)

：MacroPack 或 Phish0。它们将你的 puerto 载荷伪装成 Office宏、LNK 文件或浏览器更新包

## 进入内网后(横向移动)

Impacket (无可争议的霸主)：必须深入研究这些脚本背后的 RPC (远程过程调用)逻辑。这能教你如何通过合法的 Windows 管理协议进行“非法”操控.也是重中之重

## 关于语言

Python 调脚本，Go 跑扫描，Rust 写内核载荷