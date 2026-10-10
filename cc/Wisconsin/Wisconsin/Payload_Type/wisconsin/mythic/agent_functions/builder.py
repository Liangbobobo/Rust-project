from mythic_container.PayloadBuilder import *
from mythic_container.MythicCommandBase import *
from mythic_container.MythicRPC import *
import json
import asyncio
import os
import shutil
import tempfile
import base64
from pathlib import Path
from urllib.parse import urlparse


class Wisconsin(PayloadType):
    name = "wisconsin"
    file_extension = "exe"
    author = "@wisconsin"
    supported_os = [SupportedOS.Windows]
    wrapper = False
    wrapped_payload_types = []
    supports_dynamic_loading = False
    c2_profiles = ["http"]
    mythic_encrypts = True
    translation_container = None

    build_parameters = [
        BuildParameter(
            name="output_type",
            parameter_type=BuildParameterType.ChooseOne,
            choices=["WinExe", "WinDll"],
            default_value="WinExe",
            description="Build output format: Windows Executable (.exe) or Dynamic Link Library (.dll)",
        ),
        BuildParameter(
            name="buffer_size",
            parameter_type=BuildParameterType.ChooseOne,
            choices=["4KB", "16KB", "64KB"],
            default_value="4KB",
            description="Active communication buffer & packet size specification (4KB for stealth, 64KB for large file transfers)",
        ),
    ]

    build_steps = [
        BuildStep(
            step_name="Gathering Configuration",
            step_description="Parsing C2 profile & build parameters from Mythic UI",
        ),
        BuildStep(
            step_name="Configuring Agent",
            step_description="Injecting obfuscated configuration into Rust agent source code",
        ),
        BuildStep(
            step_name="Compiling Payload",
            step_description="Compiling Rust agent using cargo build --release",
        ),
    ]

    async def build(self, req: BuildResponse) -> BuildResponse:
        try:
            # 1. 提取构建参数与 C2 通信参数
            output_type = self.get_parameter("output_type") or "WinExe"
            buffer_size = self.get_parameter("buffer_size") or "4KB"

            server_host = "127.0.0.1"
            server_port = 8080
            is_https = False
            post_uri = "/api/v1/post"
            user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"
            master_key_str = "01234567890123456789012345678901"
            sleep_interval = 10
            sleep_jitter = 15

            # 遍历 C2 Profile 配置 (标准 HTTP Profile)
            for c2 in self.c2_profiles:
                p_name = c2.get_c2profile().get("name", "")
                if p_name == "http":
                    params = c2.get_parameters_dict()
                    cb_host = params.get("callback_host", "http://127.0.0.1:8080")
                    parsed_url = urlparse(cb_host)
                    if parsed_url.hostname:
                        server_host = parsed_url.hostname
                    if parsed_url.port:
                        server_port = parsed_url.port
                    elif params.get("callback_port"):
                        try:
                            server_port = int(params.get("callback_port"))
                        except Exception:
                            pass
                    is_https = parsed_url.scheme.lower() == "https"

                    if params.get("post_uri"):
                        post_uri = params.get("post_uri")

                    headers = params.get("headers", {})
                    if isinstance(headers, str):
                        try:
                            headers = json.loads(headers)
                        except Exception:
                            pass
                    if isinstance(headers, dict) and "User-Agent" in headers:
                        user_agent = headers["User-Agent"]

                    if params.get("callback_interval"):
                        try:
                            sleep_interval = int(params.get("callback_interval"))
                        except Exception:
                            pass

                    if params.get("callback_jitter"):
                        try:
                            sleep_jitter = int(params.get("callback_jitter"))
                        except Exception:
                            pass

                    # 提取 AESPSK
                    aes_raw = params.get("AESPSK")
                    if isinstance(aes_raw, dict) and "enc_key" in aes_raw:
                        enc_key = aes_raw.get("enc_key")
                        if enc_key:
                            master_key_str = str(enc_key)
                    elif isinstance(aes_raw, str) and len(aes_raw) > 0:
                        master_key_str = aes_raw

            # 规范化 master_key (需恰好 32 字节)
            if len(master_key_str) < 32:
                master_key_str = master_key_str.ljust(32, "0")
            elif len(master_key_str) > 32:
                # 若为 Base64 编码密钥，尝试解码
                try:
                    decoded = base64.b64decode(master_key_str)
                    if len(decoded) == 32:
                        master_key_str = decoded.decode("latin-1")
                    else:
                        master_key_str = master_key_str[:32]
                except Exception:
                    master_key_str = master_key_str[:32]

            payload_uuid = self.uuid

            # 2. 定位 Rust 源码目录
            candidates = [
                Path(__file__).resolve().parent.parent.parent / "agent_code",
                Path(__file__).resolve().parent.parent.parent.parent,
                Path("/Mythic/agent_code"),
                Path("/Mythic"),
            ]
            agent_source_dir = None
            for cand in candidates:
                if (cand / "Cargo.toml").exists() and (cand / "src").exists():
                    agent_source_dir = cand
                    break

            if not agent_source_dir:
                req.status = BuildStatus.Error
                req.build_stderr = "Could not locate Wisconsin Rust agent source code (Cargo.toml not found)."
                return req

            # 3. 创建临时构建工作空间
            build_temp_dir = Path(tempfile.mkdtemp(prefix=f"wisconsin_{payload_uuid[:8]}_"))
            try:
                # 复制项目代码至临时构建目录 (排除 target 目录)
                for item in agent_source_dir.iterdir():
                    if item.name in ["target", ".git", "Payload_Type"]:
                        continue
                    dest = build_temp_dir / item.name
                    if item.is_dir():
                        shutil.copytree(item, dest, dirs_exist_ok=True)
                    else:
                        shutil.copy2(item, dest)

                # 4. 动态打桩配置 config.rs
                config_file = build_temp_dir / "src" / "config.rs"
                if config_file.exists():
                    content = config_file.read_text(encoding="utf-8")
                    # 替换 C2 配置与 UUID
                    content = content.replace('"http://127.0.0.1:8080"', f'"{cb_host}"')
                    content = content.replace('"127.0.0.1"', f'"{server_host}"')
                    content = content.replace('"/api/v1/post"', f'"{post_uri}"')
                    content = content.replace("b50a5fe8-099d-4611-a2ac-96d93e6ec77b", payload_uuid)
                    content = content.replace("01234567890123456789012345678901", master_key_str)
                    content = content.replace("pub const DEFAULT_SLEEP_INTERVAL: u64 = 10;", f"pub const DEFAULT_SLEEP_INTERVAL: u64 = {sleep_interval};")
                    content = content.replace("pub const DEFAULT_SLEEP_JITTER: u8 = 15;", f"pub const DEFAULT_SLEEP_JITTER: u8 = {sleep_jitter};")
                    content = content.replace("8080\n    }", f"{server_port}\n    }}")
                    if is_https:
                        content = content.replace("false\n    }\n\n    /// 获取默认心跳", "true\n    }\n\n    /// 获取默认心跳")
                    config_file.write_text(content, encoding="utf-8")

                # 5. 动态打桩缓冲区规格 BUFFER_SIZE (src/lib.rs)
                lib_file = build_temp_dir / "src" / "lib.rs"
                if lib_file.exists() and buffer_size != "4KB":
                    lib_content = lib_file.read_text(encoding="utf-8")
                    if buffer_size == "16KB":
                        lib_content = lib_content.replace(
                            "pub const BUFFER_SIZE: usize = WIRE_SIZE_4K;",
                            "pub const BUFFER_SIZE: usize = WIRE_SIZE_16K;",
                        )
                    elif buffer_size == "64KB":
                        lib_content = lib_content.replace(
                            "pub const BUFFER_SIZE: usize = WIRE_SIZE_4K;",
                            "pub const BUFFER_SIZE: usize = WIRE_SIZE_64K;",
                        )
                    lib_file.write_text(lib_content, encoding="utf-8")

                # 6. 执行 Cargo 交叉编译
                target_triple = "x86_64-pc-windows-gnu"
                # 检测宿主机是否有该交叉目标，若无则尝试当前目标
                cmd = ["cargo", "build", "--release"]
                # 检查 cargo 是否支持目标
                res_check = subprocess.run(
                    ["rustup", "target", "list", "--installed"],
                    capture_output=True,
                    text=True,
                )
                if target_triple in res_check.stdout:
                    cmd.extend(["--target", target_triple])
                elif os.name == "nt":
                    # Windows 原生环境下直接使用默认 MSVC / GNU
                    pass
                else:
                    cmd.extend(["--target", target_triple])

                build_proc = await asyncio.create_subprocess_exec(
                    *cmd,
                    cwd=str(build_temp_dir),
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.PIPE,
                )
                stdout, stderr = await build_proc.communicate()

                if build_proc.returncode != 0:
                    req.status = BuildStatus.Error
                    req.build_stderr = f"Cargo build failed with code {build_proc.returncode}:\n{stderr.decode('utf-8', errors='ignore')}"
                    req.build_stdout = stdout.decode("utf-8", errors="ignore")
                    return req

                # 7. 提取产物文件
                artifact_filename = "wisconsin.exe" if output_type == "WinExe" else "wisconsin_lib.dll"
                artifact_candidates = [
                    build_temp_dir / "target" / target_triple / "release" / artifact_filename,
                    build_temp_dir / "target" / "release" / artifact_filename,
                ]

                # 兼容 dll 产物命名
                if output_type == "WinDll":
                    artifact_candidates.extend([
                        build_temp_dir / "target" / target_triple / "release" / "wisconsin_lib.dll",
                        build_temp_dir / "target" / "release" / "wisconsin_lib.dll",
                        build_temp_dir / "target" / target_triple / "release" / "thanatos_wisconsin.dll",
                        build_temp_dir / "target" / "release" / "thanatos_wisconsin.dll",
                        build_temp_dir / "target" / target_triple / "release" / "wisconsin.dll",
                        build_temp_dir / "target" / "release" / "wisconsin.dll",
                    ])

                found_binary = None
                for c in artifact_candidates:
                    if c.exists() and c.is_file():
                        found_binary = c
                        break

                if not found_binary:
                    req.status = BuildStatus.Error
                    req.build_stderr = f"Artifact {artifact_filename} not found after successful compilation."
                    return req

                req.payload = found_binary.read_bytes()
                req.status = BuildStatus.Success
                req.build_message = f"Successfully generated Wisconsin {output_type} payload ({len(req.payload)} bytes)"
                req.build_stdout = stdout.decode("utf-8", errors="ignore")
                return req

            finally:
                shutil.rmtree(build_temp_dir, ignore_errors=True)

        except Exception as e:
            req.status = BuildStatus.Error
            req.build_stderr = f"Unexpected build error: {str(e)}"
            return req
