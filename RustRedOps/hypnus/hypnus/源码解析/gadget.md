## pub fn get_text_section

win64下,内存以页为单位.PE文件加载进内存的时候,Windows的加载器ldr会根据节表定义,给每个页加上不同的执行权限(R/W/X)

**硬件级的防御：DEP (数据执行保护)**

如果 CPU 尝试去执行一块标记为 RW 但没有 X属性的内存地址，硬件会立即触发一个 访问违规 (Access Violation)异常，系统直接强行关掉你的程序

因此, Gadget（如 jmp r11）必须去 .text 节找

**如何区分源码并放入不同的节表**
1. 编译器根据语法定义给每行代码加上的不同的逻辑标签(CODE/DATA/CONST)
2. 编译器生成.obj文件,obj文件是多个逻辑零件包.链接器Linker把多个obj文件放入不同的节中(CODE->.text;DATA->.data/.bss;CONST->.rdata)
3. 打包生成pe文件结构:链接器生成pe文件头部,并在其中写下节表
4. os加载器ldr读取pe头部.如.text调用NtMapViewOfSection,分配内存,使用mmu把这块内存设为只允许执行.cpu的dep就有了依据.如果在.text之外执行代码,会抛出异常




**.text节特性**
1. 安全性：防止由于缓冲区溢出（Buffer Overflow）导致的恶意代码执行
2. 效率：CPU 有专门的 “指令缓存 (I-Cache)”。将所有的指令集中放在 .text段，能极大地提高 CPU 读取指令的速度
3. 不可修改性：.text段通常是只读的。这意味着病毒很难在不引起报警的情况下，直接修改已加载DLL 的函数代码