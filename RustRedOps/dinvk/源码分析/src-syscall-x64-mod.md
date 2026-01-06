# mod.rs

重点是获取ssn

## Hell's Gate Halo's Gate Tartarus Gate三种技术retrieve ssn

### 背景知识

在现代操作系统（如 Windows/Linux）中，出于安全稳定考虑，内存被划分为用户空间 (User Mode, Ring 3) 和 内核空间 (Kernel Mode, Ring 0)  

1. 用户模式：你的程序在这里运行，权限受限，不能直接操作硬件或物理内存。  
2. 内核模式：操作系统核心在这里运行，拥有最高权限。  
3. Syscall (系统调用)：这是用户模式进入内核模式的唯一合法大门。

**关键机制：**  
当 CPU 执行 syscall 指令时，它会做两件大事：  

1. 切换 CPU 状态从 Ring 3 到 Ring 0。  
2. 根据 EAX 寄存器 中的数值（也就是 SSN），在内核的一张大表格（SSDT - System Service Descriptor Table）中查找对应的处理函数指针（比如 NtAllocateVirtualMemory 的内核实现）。

**结论：**
EAX 寄存器是用户通往内核的“暗号”。如果 EAX 里的值不对，内核就不知道你想干嘛，或者会报错。因此，我们前面所有的努力（Hell's Gate 等），核心目的只有一个：把正确的数字塞进 EAX。

### 1. 干净的 Syscall Stub（系统调用存根） (The "Clean" Stub)

在 Windows x64 环境下，核心系统库 ntdll.dll  
负责将用户模式的请求传递给内核模式,**是用户态进入内核态的桥梁**。它里面包含了成百上千个系统调用（Syscalls）。

如果没有任何安全软件（EDR/AV）干扰，ntdll.dll 中的一个典型函数（比如 NtAllocateVirtualMemory）在内存中的 32  
字节（0x20）区域长这样：

```Text
┌───────────────┬────────────────┬─────────────────────┬───────────────────────────────────────┐
│ 偏移 (Offset)  │ 机器码 (Hex)    │ 汇编指令 (Assembly)  │ 含义                                  │
├───────────────┼────────────────┼─────────────────────┼───────────────────────────────────────┤
│ +00           │ 4C 8B D1       │ MOV R10, RCX        │ 将参数1从 RCX 移到 R10 (syscall 约定) │
│ +03           │ B8 <SSN> 00 00 │ MOV EAX, <SSN>      │ 关键！ 将系统调用号放入 EAX           │
│ +07           │ 00             │ (Padding)           │ 填充                                  │
│ +08           │ 0F 05          │ SYSCALL             │ 陷入内核                              │
│ +10           │ C3             │ RET                 │ 返回                                  │
│ ...           │ 0F 1F 84 ...   │ NOP                 │ 填充到下一个 32 字节边界              │
└───────────────┴────────────────┴─────────────────────┴───────────────────────────────────────┘
```

获取 SSN 的核心目标就是读取 mov eax, `<SSN>` 指令中的立即数  
我们的目标就是拿到 +04 和 +05 位置的那两个字节（也就是 SSN）。

### 2. 被 Hook 的 Syscall 存根

EDR 为了监控，会覆盖这些指令。  

* 头部 Hook (Head Hook): 覆盖前 5 个字节为 JMP  (机器码 0xE9)。  
* 中部 Hook (Trampoline Hook): 保留前 3 个字节 (MOV R10, RCX)，覆盖第 4 个字节开始的内容为 JMP

SSN 的连续性:  
ntdll.dll 中的系统调用通常按地址顺序排列，其 SSN 也是递增的。如果函数 A 的 SSN是 n，那么紧邻它的下一个系统调用函数 B 的 SSN 通常是 n+1
---

## 技术一：Hell's Gate (地狱之门)

这是最理想的情况。我们假设内存没有被修改，直接去读。

**原理：**  
如果在函数地址处，字节序列完全符合“干净 Syscall”的特征，那么第 4 和 第 5 个字节必然是 SSN。

**代码深度解析：**

```rust
// address 是函数在内存中的起始指针.类型是*const u8,以逐字节对比
// 检查是否符合 Windows x64 系统调用存根 (Stub) 的标准特征
// 对应的汇编指令:
// 4C 8B D1       MOV R10, RCX
// B8 XX XX 00 00 MOV EAX, <SSN>
if read(address) == 0x4C            // 偏移 0: 必须是 4C ,即(MOV R10, RCX)的前缀.`0x4C` 告诉 CPU：“接下来的指令是 64 位宽度的(W=1)，并且目标寄存器使用了扩展寄存器集中的 R8-R15 之一 (R=1).在这里，它确切地指代了 MOV R10, ... 中的 R10 和 64 位操作
    && read(address.add(1)) == 0x8B // 偏移 1: 必须是 8B,MOV 操作码
    && read(address.add(2)) == 0xD1 // 偏移 2: 必须是 D1 (ModR/M: R10 -> RCX)
    && read(address.add(3)) == 0xB8 // 偏移 3: 必须是 B8 (MOV EAX, ...)
    && read(address.add(6)) == 0x00 // 偏移 6: SSN 高位通常为0，作为额外校验
    && read(address.add(7)) == 0x00 // 偏移 7: 额外校验
{
    // 特征匹配成功！提取 SSN。
    // 假设机器码是 B8 18 00 00 00 (SSN = 0x18)

    // read(address.add(5)) 读取偏移5 -> 0x00 (高位)
    let high = read(address.add(5)) as u16;

    // read(address.add(4)) 读取偏移4 -> 0x18 (低位)
    let low = read(address.add(4)) as u16;

    // 组合 (Little-Endian 字节序):
    // (0x00 << 8) | 0x18 = 0x0018
    let ssn = (high << 8) | low;
    return Some(ssn);
}
```

---

## 技术二：Halo's Gate (光环之门)

**场景：** EDR 修改了函数开头，变成了 E9 xx xx xx xx (JMP)。Hell's Gate 第一步检查 0x4C 就会失败。

**原理：**  
利用 ntdll.dll 的空间局部性和SSN 连续性。  
在 ntdll 的 .text 段中，系统调用函数是紧挨着的，每个占用 32 字节（DOWN = 32）。  

* 地址 X : NtMapViewOfSection (SSN: 0x27) -> 被 Hook  
* 地址 X+32: NtUnmapViewOfSection (SSN: 0x28) -> 干净  

如果我们要找 NtMapViewOfSection 的 SSN，发现它烂了。我们就往下走 32 字节，看看邻居是不是好的。如果是好的，读出邻居的  
SSN (0x28)，然后 减去 1，就得到了我们想要的 0x27。

**代码深度解析：**

```rust
// 1. 识别被 Hook (0xE9 = JMP)
if read(address) == 0xE9 {
    // 2. 开始向外搜索，RANGE = 255 (也就是搜上下 255 个邻居)
    for idx in 1..RANGE {

        // --- 向下搜索 (Next Neighbors) ---
        // 计算邻居地址：当前地址 + (第几个邻居 * 32字节)
        let neighbor_addr = address.add(idx * DOWN);

        // 检查这个邻居是不是干净的 (逻辑同 Hell's Gate)
        if read(neighbor_addr) == 0x4C && ... {
            // 找到好邻居了！读取它的 SSN
            let neighbor_high = read(neighbor_addr.add(5)) as u16;
            let neighbor_low  = read(neighbor_addr.add(4)) as u16;

            // 关键数学推导：
            // 想要的SSN = 邻居SSN - 邻居的距离(idx)
            // 因为内存地址越大，SSN 越大，所以往下找要减。
            let ssn = (neighbor_high << 8) | (neighbor_low - idx as u16);
            return Some(ssn);
        }

        // --- 向上搜索 (Previous Neighbors) ---
        // 计算邻居地址：当前地址 + (第几个邻居 * -32字节)
        // 使用 .offset() 因为 UP 是负数 (-32)
        let prev_neighbor_addr = address.offset(idx as isize * UP);

        if read(prev_neighbor_addr) == 0x4C && ... {
             // 关键数学推导：
             // 想要的SSN = 邻居SSN + 邻居的距离(idx)
             // 因为往上找，邻居的 SSN 比我们要的小，所以要加回来。
             let high = ...;
             let low = ...;
             let ssn = (high << 8) | (low + idx as u16);
             return Some(ssn);
        }
    }
}
```

---

## 技术三：Tartarus Gate (塔耳塔洛斯之门)

**场景：** 这是一个更高级的 Hook 场景。  
EDR 知道攻击者会检查第一个字节是不是 E9。所以 EDR 这样做：  

* 保留 MOV R10, RCX (前3字节: 4C 8B D1)。这样 Halo's Gate 就以为这不是 Hook，因为它只检查第一个字节是不是 E9。  
* 把 MOV EAX, SSN (第4字节开始) 覆盖成 JMP (E9 ...)。  

**结果：**  

1. Hell's Gate 失败：因为第4字节不是 B8。  
2. Halo's Gate 失败：因为第1字节不是 E9，它直接跳过了搜索逻辑。  

**原理：**  
Tartarus Gate 补上了这个漏洞。它在 Hell's Gate 失败后，专门检查偏移量为 3 的位置（即第4个字节）是不是  
E9。如果是，说明是这种“心机” Hook，然后立刻启动和 Halo's Gate 一模一样的邻居搜索逻辑。

**代码深度解析：**

```rust
// 此时 Hell's Gate 已经 return 或者是失败了
// 此时 Halo's Gate 检查 index 0 也失败了 (因为它不是 E9)

// 检查偏移 3 (address + 3) 是不是 E9
if read(address.add(3)) == 0xE9 {
    // 哎呀，原来是在这里 Hook 的！
    // 既然被 Hook 了，我就不能读当前的 SSN 了。
    // 剩下的逻辑和 Halo's Gate 完全一样：找邻居。

    for idx in 1..RANGE {
        // 向下找...
        // 计算: ssn = neighbor_ssn - idx

        // 向上找...
        // 计算: ssn = neighbor_ssn + idx
    }
}
```

## 总结流程图

当调用 ssn("NtAllocateVirtualMemory", ...) 时：

1. 解析导出表，找到函数地址 0x7FF...A0。  
2. Step 1: Hell's Gate 检查  
    * 地址 0x...A0 的字节是 4C 8B D1 B8 ... 吗？  
    * 如果是 -> Bingo! 提取 SSN，结束。  
    * 如果不是 -> 进入 Step 2。  
3. Step 2: Halo's Gate 检查  
    * 地址 0x...A0 的字节是 E9 ... 吗？  
    * 如果是 -> 启动邻居搜索，计算 SSN，结束。  
    * 如果不是 -> 进入 Step 3。  
4. Step 3: Tartarus Gate 检查  
    * 地址 0x...A3 (偏移3) 的字节是 E9 ... 吗？  
    * 如果是 -> 启动邻居搜索，计算 SSN，结束。  
    * 如果不是 -> 返回 None (或者更高级的失败处理)。  

这个项目通过这一套组合拳，极大地提高了在受控环境（有 EDR 挂钩）下获取正确 SSN 的成功率。
