# 重写需要注意

1. 将敏感的变量或函数名通过全选替换为一个随机的字符串,当然自己要留一个备份表

## 编译方式

Strip（剥离）符号,以release方式编译,并添加strip = true  
大部分局部变量名和内部函数名都会消失，变成内存地址

## 变量及函数名

改为自己的一套命名规则  
为了防止针对开源代码的模糊哈希（Fuzzy Hashing）匹配,以及防止被逆向分析人员一眼认出是 dinvk 的变种

关键字符串栈混淆（Stack Strings）：除了使用 obfstr，对于关键的 DLL名（如 "ntdll.dll"），尝试手动构建栈字符串（即 let mut s = [0u8; 10]; s[0] = 'n' as u8; ...），这样IDA/Ghidra 很难直接识别出字符串。





## panic

有一些字符串可能通过 Panic 信息或者是 format!宏泄露出来。  
比如 panic!("NtAllocateVirtualMemory Failed")这种字符串一旦出现在二进制里。

### 关于panic的修改

在调试完成后再修改panic,否则会大大增加调试难度

1. 彻底移除所有错误提示字符串。不要 eprintln!，不要panic!("message")。出错直接返回错误码或静默退出。
2. 在 Cargo.toml 中开启 panic = "abort"
3. 重写 src/panic.rs，确保 Panic 时什么都不做（直接死循环或调用NtTerminateProcess），绝对不要打印任何信息。但这会增加调试难度,应当在调试完整后再加入

## Control Flow Graph(CFG)

杀软引擎会分析函数的代码结构（比如：这里有一个循环，循环里有一个如果是 'M'开头的判断，然后访问了偏移 0x30... 哦，这是在遍历 PEB）。

1. 打乱逻辑顺序：原版是 while 循环，你改成 loop + match。
2. 加入垃圾代码（Junk Code）：在遍历 PEB的过程中，加入一些无意义的数学运算或永远为真的判断，改变汇编指令的指纹。
3. 更换遍历方式：原版是遍历 InMemoryOrderModuleList，你可以改为遍历InLoadOrderModuleList（偏移不同），或者直接硬编码查找特定特征。

## 特征码与常量 (Magic Numbers)

问题：dinvk 里用到的 Hash 常量、Hell's Gate 的搜寻范围（UP = -32, DOWN
     = 32）、以及 syscall 指令的机器码匹配模式。
   * 重写建议：
       * 修改 Hash 算法：正如你上一个问题提到的，只用一个你自己修改过参数的
         Hash 算法，替换掉原项目中所有的 Hash 常量。
       * 修改搜寻策略：在 syscall/mod.rs 中，不要死板地用 0x0F 0x05 搜
         syscall。可以尝试搜寻函数序言（prologue）或者其他特征。



          E. 编译配置 (Cargo.toml)
  这是最简单但最重要的。确保你的 Cargo.toml
  包含以下配置以最小化体积并剥离符号：

   1 [profile.release]
   2 opt-level = "z"       # 优化体积 (或者 "s")
   3 lto = true            # 链接时优化，能极大打乱代码结构
   4 codegen-units = 1     # 降低并行编译，增强 LTO 效果
   5 panic = "abort"       # 移除 unwinding 代码
   6 strip = true          # 自动剥离符号 (Rust 1.59+)

  3. 特别提醒：关于硬件断点 (Hardware Breakpoint)

  dinvk 的一大特色是利用硬件断点进行参数欺骗（Spoofing）。
   * 风险：这种技术虽然能绕过一些 EDR 的
     Hook，但操作调试寄存器（Dr0-Dr7）本身就是一个非常可疑的行为。现在的 EDR
     对 NtGetContextThread / NtSetContextThread 监控非常严。
   * 建议：如果你是初学者，建议在重写时先剔除硬件断点功能，只保留 Indirect
     Syscall。硬件断点很容易弄巧成拙，导致程序直接被杀软的主动防御（HIPS）拦
     截。

  总结你的重写路线图：

   1. Cargo 配置：先把 profile.release 配置好，确保没有符号泄露。
   2. 改名与换血：修改函数名，替换 Hash 算法。
   3. 沉默是金：移除所有 println!, eprintln! 和带消息的 panic!。
   4. 逻辑变形：在 module.rs 这种核心逻辑里，手动调整代码结构，不要照抄。
   5. 字符串隐藏：所有硬编码字符串必须混淆。