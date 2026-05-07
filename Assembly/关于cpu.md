# CPU

在冯诺依曼架构下,cpu物理层面对齐处理的操作数(比如指针),是通过其**语境**CONTEXT进行区分的.比如对同一个64位的数值,该数值如果被加载进RIP它就是指令地址,如果被加载进RAX,就是一个普通数值.如果作为MOV指令操作数,它就是数据指针.  
1. 在 RAM 颗粒上，存储指令的电荷与存储数据的电荷在物理上没有任何区别
2. 执行区分权：由 CPU 的控制器（Control Unit）和指令译码器（Instruction Decoder）拥有

## Ai时代研究cpu等底层技术的价值


## CPU如何区分指令和地址



## CPU架构区分

依据CPU的指令集架构ISA进行区分

1. x64/AMD64,使用CISC复杂指令集.主要在Windows,Linux,macOS(Intel版),FreeBSD
2. ARM/AARCH64,使用RISC精简指令集.主要在Android,iOS,Linux,Windows on arm,macOS.等移动设备上
3. RISC-V,基于RISC(open)完全开源,主要再Iot,嵌入式,服务器领域等
4. MIPS,使用RISC精简指令集,之前用于路由器,交换机,机顶盒等设备中,但逐渐衰落
5. LoongArch,基于RISC,中国自研
6. 鸿蒙基于Arm和RISC-V

## 指令集架构ISA

指令集对cpu的作用和意义

ISA,Instruction Set Architecture.是硬件和软件之间的契约.而汇编语言是该契约的“人类可读文本表示”  
1. 软件与硬件的Bridge:CPU本质上是数亿晶体管组成的逻辑开关,如果没有指令集软件无法指挥硬件

## CPU结构/组成

## CPU的一般工作流程

1. PE 文件加载：操作系统读取 PE 头部的 EntryPoint 偏移，加上ImageBase，得到一个虚拟地址，塞进 RIP
2. 执行开始：CPU 开始从 RIP 指向的地方“取指”
3. Mm 介入：如果取不到（Present=0），CPU 报错，Windows内存管理器（Mm）开始填页表、分物理页（PFN）
4. 循环往复：只要程序不结束，CPU 就在这个 VA -> MMU -> PA -> RAM 的闭环里循环

