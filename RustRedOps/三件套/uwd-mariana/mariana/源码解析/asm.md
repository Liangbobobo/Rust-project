# asm

1. MSVC （Microsoft Visual C++） 即(MASM - Microsoft Macro Assembler).使用Intel风格语法
2. GNU(NASM / GAS - GNU Assembler):GNU与MSVC的语法不通、关键字不通、段定义不通、编译器不通。它们虽然共享一套 CPU指令集，但在‘如何写出这些指令’的代码契约上，存在物理隔离。
3. 使用cargo-build时,rust会根据目标环境调用x86_64-pc-windows-msvc/x86_64-pc-windows-gnu


# MSVC/synthetic

## SpoofSynthetic proto

函数原型声明

1. SpoofSynthetic:函数名标识符
2. proto:MASM关键字,代表Prototype原型
3. 告诉rustc,SpoofSynthetic proto是一个函数.是rust与asm交互开始的地方.在uwd.rs中定义extern "C" {fn SpoofSynthetic proto();}.且两者在链接器Linker层面需要完美匹配
4. Refactor中应该改为其他名称

## .data

1. .是段前缀操作符,data关键字代表Data Segment数据段;作用是接下来的所有声明(直到下一个.code为止)都不要放在代码执行区,放在内存的可读写数据区;rustc会在生成的.obj文件中创建一个名为.data的节
    * win64下的进程空间中,.data段通常是rw可读写的,代码所在的.text段是rx可读可执行的
    * 严格遵循段定义是绕过AV启发式扫描的基础,但在Refactor中,有把段名改为.rdata只读或其他自定义段的情况.但这需要具体审慎的分析

## Config STRUCT

```asm
Config STRUCT
    RtlUserThreadStartAddr       DQ 1
    RtlUserThreadStartFrameSize  DQ 1
    
    BaseThreadInitThunkAddr      DQ 1
    BaseThreadInitThunkFrameSize DQ 1

    FirstFrame                   DQ 1
    SecondFrame                  DQ 1
    JmpRbxGadget                 DQ 1
    AddRspXGadget                DQ 1

    FirstFrameSize               DQ 1
    SecondFrameSize              DQ 1
    JmpRbxGadgetFrameSize        DQ 1
    AddRspXGadgetFrameSize       DQ 1

    RbpOffset                    DQ 1

    SpooFunction                 DQ 1
    ReturnAddress                DQ 1

    IsSyscall                    DD 0
    Ssn                          DD 0

    NArgs                        DQ 1
    Arg01                        DQ 1
    Arg02                        DQ 1
    Arg03                        DQ 1
    Arg04                        DQ 1
    Arg05                        DQ 1
    Arg06                        DQ 1
    Arg07                        DQ 1
    Arg08                        DQ 1
    Arg09                        DQ 1
    Arg10                        DQ 1
    Arg11                        DQ 1
Config ENDS
```

1. Config STRUCT ... Config ENDS定义一个结构体.物理上与普通的定义变量不同,定义struct并不在内存中分配空间,而是在汇编器的符号表中建立一张偏移量表.如RtlUserThreadStartAddr 被标记为偏移量 +0;BaseThreadInitThunkAddr 被标记为偏移量 +16（0x10）
    * 对于普通的变量声明如,MyVar DQ 1,汇编器会在数据段腾出8字节,并把地址命名为MyVar;而上述的结构体仅仅建立一个逻辑,也就是汇编器内部维护着一张符号表(如Config.RtlUserThreadStartAddr 0,且使用RVA进行寻址)