# memchr库

## Why Use The Ctare Memchr及适用场景

对于
```rust
fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

// Rust core library
fn search(haystack: &str, needle: &str) -> Option<usize> {
    haystack.find(needle)
}
```

At a high level of the performance,there are two primary ways to look at it:
1. Throughput(吞吐量):极限带宽/SIMD向量化能力.在内存极大场景,且目标字节故意不出现.逼迫memchr从头到尾扫描haystack,无法提前退出.以此测试极限吞吐速率,考验算法/库的方法能否把cpu的AVX2/AVX-512/ARM Neon等SIMD向量化能力拉满
2. Latency:启动开销/函数前置准备成本.在内存极小(3字节),扫描3字节本身只需要ns级时间,但进入memchr函数前,进行内存对齐检查,SIMD寄存器初始化,函数调用压栈等准备工作耗费的时间远比扫描3字节本身长.这是测试算法的前置响应延迟,如一个算法/库扫描1GB极快,但每次启动需要10ns,那么处理几百万个3字节短字符时,性能也不行.
3. memchr has slightly worse latency than the rust core library(search).however,its throughput can easily be over an order of magnitude量级 faster.
4. memchr的名字来源于c标准库libc中的同名函数.本rust memchr库的核心优势:解耦于正在使用的那个libc库的实现质量.在不同的os和平台上,各libc实现质量差异巨大.
5. GNU Linux(glibc)最好;win/嵌入式/android等各平台优化不同,性能忽高忽低;
6. 如果在Rust中调用c的libc::memchr,Rust程序性能会被os的libc绑定.而Rust memchr库不依赖任何os的libc,用纯rust手写了顶级的,跨平台的SIMD扫描引擎.其会自动检测当前cpu硬件特性,自动挂载最高性能的SIMD算法.
7. Rust核心标准库(core/std)唯一存在的子字符串搜索例程(find),只能严格工作在合法utf-8字符串(&str)上(详见rust相关/rust grammer/slice &str).而memchr更加广泛的适用于&str和`&[u8]`中
8. Note:memchr甚至比标准库更快更好.该库的官网自信的认为比std/core更好.但对比标准库,其在不同平台的兼容性值得思考,值得作为出错点考虑.
9. NOTE: Currently, only x86_64, wasm32 and aarch64 targets have vector accelerated implementations of memchr (and friends) and memmem.


## 关于其参数 haystack needle

在计算机科学/Rust底层库(memchr,regex,rust std core::str)经常使用参数名 **haystack**

1. 含义:haystack命名取材英语"find a needle in a haystack"(大海捞针).代表要被检索的大内存缓冲区;needle代表要检索的目标
2. Rust Api命名规范的一致性.Rust Api设计中,参数命名的可预测性放在极高位置










3. The top-level module provides routines for searching for 1, 2 or 3 bytes in the forward or reverse direction. When searching for more than one byte, positions are considered a match if the byte at that position matches any of the bytes.任一byte匹配都视为match
4. routine:原意为常规\例行公事,在系统编程,是子程序(subroutine)/例程的缩写.指一段被精心编写的、用于执行特定任务的可重复调用代码块
5. The memmem sub-module provides forward and reverse substring search routines.
    * 为啥叫Routine而不是常用的Function.为了区分代码逻辑层/执行层.在汇编时代,没有“类（Class）”或“方法（Method）”的概念，只有一段段通过CALL 跳转进去的二进制序列，这些序列被称为 Subroutines.memchr作为一个底层库，继承了这种强调硬件操作的命名传统
    * memchr 里的 routines 通常是 “多态执行” 的。当你调用 find 时，它会根据当前 CPU的能力（是否支持 AVX-512? AVX2? SSE4?），在内部自动派发（Dispatch）到不同的底层routines 去执行;普通 Function：通常只有一种实现,Routine：可能是一组专门为某类硬件优化的机器码逻辑
6. SIMD 指令集加速：memchr 库会探测 CPU 是否支持 AVX2 或SSE2。它利用矢量寄存器一次性加载 16 或 32个字节进寄存器，执行单指令、多数据的并行比对。这比逐字节对比快 10-20 倍
7. Two-Way 算法：这是一种结合了 Boyer-Moore 和 Knuth-Morris-Pratt优化的算法。它能在线性时间内`（$O(n)$）`完成搜索，且不需要额外的内存分配（Space `$O(1)$`）
8. In all such cases, routines operate on`&[u8]` without regard to encoding. This is exactly what you want when searching either UTF-8 or arbitrary bytes.
9. 不需要开辟堆内存，完全在寄存器和已有的栈切片上运行。这完美契合了 uwd 作为 no_std 库的定位
10. memchr 库对边界处理极其严谨。即使 bytes 是空切片（当 size=0时），该函数也会安全返回 None，绝不产生越界读特征
11. 背景:该库中提到的haystack needle来自英语中的经典成语："Finding a needle in a haystack"（大海捞针，直译为“在干草堆里找一根针”）


## memchr::memmem::find 

适用于查找连续的4字节机器码或子字符串.其needle的长度无上限.而memchr,memchr2,memchr3分别代表找1,2,3个独立的单字节更优秀

`pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize>`

1. Returns the index of the first occurrence of the given needle.
2. Note that if you’re are searching for the same needle in many different small haystacks, it may be faster to initialize a Finder once, and reuse it for each search.



## 关于haystack

memchr的官方文档提到过其所有的例子都是针对`&[u8]`的,我想知道这对任何搜索场景都适用吗  
在 Windows 底层开发和 RedOps 领域，它是绝对通用的
1. Rust 中，`&[u8]`（字节切片）是物理内存最纯粹的逻辑表达
2. 连续性：它保证数据在物理内存中是连续排列的
3. 原子单位：u8 代表 1 个字节（8 bits）。在 x64架构下，内存寻址的最小单位就是字节
4. 类型无关性：无论内存里存的是字符串、图片、加密后的 Payload，还是 CPU指令，它们在物理层面全是字节

**局限**
1. 非连续数据：比如搜索链表中的节点，memchr 无能为力，因为它要求 Haystack物理连续
2. 语义模糊匹配：比如“忽略大小写”的字符串搜索。虽然 `&[u8]` 能搜，但你得先把Needle 准备好各种变体（或者使用专门的例程）
3. **对齐要求**:如果你要搜一个特定的 u32 数值，但该数值必须在 4字节对齐的位置上。简单的字节搜索会把跨越边界的随机字节组合也搜出来（产生误报）