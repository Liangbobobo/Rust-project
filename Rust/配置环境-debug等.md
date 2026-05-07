

## 编译及debug工具链

Rust 工具链: 
stable-x86_64-pc-windows-msvc
stable-x86_64-pc-windows-gnu
的选择会影响很多配置和debug工具  

两者只是在本地编译时使用了不同的编译工具msvc或者gnu,都可以生成跨平台应用.  
使用 Cargo 的交叉编译功能 (--target参数)，这个过程与您本地使用的是 msvc 还是 gnu 无关。

msvc:
1. 需要链接一个用 Visual Studio 编译的第三方 C/C++ 库（例如，很多 Windows平台的商业 SDK）。 
2. 最“原生”的 Windows 开发体验和最好的系统集成。
GNU:
希望在所有平台上使用一致的 GCC/GNU 工具链。
1. 需要链接一个只能用 GCC/MinGW 编译的开源 C/C++ 库。




## 使用msys2在win11上安装llvm

### 第一步：安装 MSYS2 和 MinGW-w64 工具链 (包含 GCC 和 LLDB)

#### 1. 下载并安装 MSYS2

* 访问 MSYS2 官方网站：https://www.msys2.org/
* 下载并运行安装程序。建议安装到默认路径（通常是 `C:\msys64`）

#### 2. 更新 MSYS2 包管理器

* 安装完成后，从 Windows 的"开始"菜单中找到并打开 "MSYS2 MSYS" 终端
* 在终端中，运行以下命令更新包数据库和核心组件。如果提示关闭终端，请照做，然后重新打开 "MSYS2 MSYS" 终端并再次运行 `pacman -Su`

```bash
pacman -Syu
pacman -Su
```

#### 3. 安装 MinGW-w64 工具链 (包含 GCC)

* 在同一个 MSYS2 终端中，运行以下命令安装完整的 MinGW-w64 工具链。这个包包含了 `gcc.exe`、`dlltool.exe` 等所有 gnu 工具链编译 Rust 项目所需的 C/C++ 编译器和构建工具

```bash
pacman -S mingw-w64-x86_64-toolchain
```

* 当提示选择要安装的包时，直接按 Enter 键接受默认选项（即全部安装）
* 当提示确认安装时，输入 `Y` 并按 Enter 键

#### 4. 安装 LLDB 调试器

* 在同一个 MSYS2 终端中，运行以下命令安装 CodeLLDB 所需的 LLDB 调试器：

```bash
pacman -S mingw-w64-x86_64-lldb
```

* 当提示确认安装时，输入 `Y` 并按 Enter 键

---

### 第二步：配置系统环境变量 (PATH)

#### 1. 找到 MSYS2 的 `bin` 目录

* LLDB 和 GCC 都安装在 MSYS2 的 `mingw64\bin` 目录下。如果 MSYS2 安装在 `C:\msys64`，那么完整的路径就是 `C:\msys64\mingw64\bin`

#### 2. 将此路径添加到 Windows 系统 PATH 环境变量

* 在 Windows 搜索栏中输入"环境变量"，然后选择"编辑系统环境变量"
* 点击"环境变量"按钮
* 在"系统变量"部分，找到名为 `Path` 的变量，然后双击它
* 点击"新建"，然后输入 `C:\msys64\mingw64\bin`
* 点击"确定"关闭所有窗口

---

### 第三步：配置 Rust 工具链

#### 1. 打开 Windows 命令提示符 (CMD) 或 PowerShell

**请注意，不是 MSYS2 终端**

#### 2. 安装 `stable-gnu` 工具链

```bash
rustup toolchain install stable-gnu
```

#### 3. 将 `stable-gnu` 设置为默认工具链

```bash
rustup default stable-gnu
```

---

### 第四步：配置 VS Code / Cursor

#### 1. 打开 VS Code / Cursor

#### 2. 打开用户设置 (JSON)

* 按下 `Ctrl+Shift+P`
* 输入 `settings.json`，然后选择 "Preferences: Open User Settings (JSON)"

#### 3. 添加 `lldb.executable` 配置

* 在打开的 `settings.json` 文件中，添加或修改以下行，明确告诉 CodeLLDB 调试器 `lldb.exe` 的位置。请确保路径中的双反斜杠 `\\`

```json
{
    // ... 您可能还有其他设置，请在其中添加下面这行
    "lldb.executable": "C:\\msys64\\mingw64\\bin\\lldb.exe"
}
```

#### 4. 保存文件

---

### 第五步：验证和调试

#### 1. 完全关闭并重新启动 VS Code / Cursor

这是为了确保所有环境变量和设置都已加载

#### 2. 打开您的 Rust 项目文件夹

#### 3. 手动构建项目

在 VS Code 的终端中，进入您的项目根目录，运行：

```bash
cargo build
```

这应该会成功编译您的项目

#### 4. 开始调试

* 在您的 Rust 代码中设置一个断点
* 点击左侧边栏的 "Run and Debug" (运行和调试) 图标
* 点击绿色的 "Run and Debug" 按钮，或从下拉菜单中选择您的可执行文件进行调试

现在，您应该能够使用 CodeLLDB 成功调试您的 Rust 项目了。