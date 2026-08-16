;; code responsible for stack spoofing via Synthetic(masm)
;; uwd项目的核心,负责在物理内存中,把cpu寄存器和堆栈捏造成一个合法的系统调用栈,欺骗win10/11内核与edr

;; Synthetic 合成栈模式(默认主流):切断当前线程的真实历史栈,在内存中从0开始凭空捏在一套完整的包含4层合法系统函数的假调用链.
;; 绝大多数edr的栈回溯探针,主要校验的是:从当前RIP能否一路沿着pdata展开到RtlUserThreadStart,且RBP链表是否单调递增.而Synthetic能够完美欺骗这套逻辑.
;; 如果是叶子函数,edr这套逻辑是否失效.当然不可能出现这种漏洞.win的官方栈展开器RtlVirtualUnwind和edr对leaf function对于展开器遇到没有.pdata的叶子函数时情况.详见av edr原理/栈展开
;; 

;;
;; Export
;; 声明全局函数原型,让Rust链接器能够找到这个函数
;; proto是prototype缩写,代表函数声明
SpoofSynthetic proto

;; 这是一条汇编指示符,告诉编译器(ml64.exe)接下来的内容,属于.data范畴,而不是其他如.text的可执行cpu代码
;; 进入可读写数据段(rw权限,存放全局变量和静态变量static)
.data

;; 定义一个Config结构体
;; configuration structure passed to the spoof asm routine = struct Config {}
;;
Config STRUCT
    ;; DQ(define Quadword),8字节的RtlUserThreadStart函数物理地址.默认值为1,其默认值可为1 0 甚至是? 作为语法占位符
    RtlUserThreadStartAddr          DQ 1 
    RtlUserThreadStartFrameSize     DQ 1

    BaseThreadInitThunkAddr         DQ 1
    BaseThreadInitThunkFrameSize    DQ 1

    FirstFrame                      DQ 1
    SecondFrame                     DQ 1
    JmpRbxGadget                    DQ 1
    AddRspXGadget                   DQ 1

    FirstFrameSize                  DQ 1
    SecondFrameSize                 DQ 1
    JmpRbxGadgetFrameSize           DQ 1
    AddRspXGadgetFrameSize          DQ 1

    RbpOffset                       DQ 1

    SpooFunction                    DQ 1
    ReturnAddress                   DQ 1

    ;; DD(define Doubleword)=u32/i32(Rust) 对应cpu寄存器eax,ecx等32位寄存器.这里定义为DD,因为微软规定,ssn必须装入32位的eax(不是64位的rax)
    ;; IsSyscall只是一个判断条件设为dd,是因为内存对齐的要求.详见注释1
    IsSyscall                        DD 0
    Ssn                              DD 0

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

;; 进入.text节 编写cpu指令
.code 

;; proc是procedure(过程/子程序/函数)的缩写,是函数具体实现的真正起点(从{ 开始)
SpoofSynthetic PROC
;;
;; saving non-vol register:这三个寄存器都是非易失性的,用前必须保存,用后必须pop恢复
;;rbp在项目中用于真实旧栈的基准位置指针(以rbp为基准(物理栈顶),记录父函数原本局部变量的位置),整个伪造栈执行完毕后,用于瞬间将所有假栈丢弃并归位.rbx用于gadget片段中(jmp [rbx]),返回真实执行流.r15用于通用寄存器,用于保存数据
push rbp
push rbx
push r15

;;
;; everything between rsp and rbp is our new stack frame for unwinding

;; 这里210h的含义,详见注释2
sub rsp,210h 






;; 注释1
;; 1. 消除padding,防止Rust与汇编内存错位:64位cpu和win64 abi中,数据存放有一条规范,多大字节的数据,必须存放在能被该字节整除的内存地址上(自然对齐,natural alignment)
;; rustc在处理对齐时,会自动padding(如在#[repr(c)]下,会在1字节的数据后面padding 3字节以对齐) 但masm汇编器(ml64.exe)在解析STRUCT时,其内部对跨类型隐式padding的处理规则非常古老且脆弱,不会自动插入.这就导致对齐错位.这里将IsSyscall和Ssn置为dd,两者共8字节,没有任何隐式空洞,Rust和masm不会发生内存偏移的分歧
;; 2. windows sdk 的官方头文件minwindef.h中,微软对bool定义就是一个int,大小就是4字节(c/c++/win api中都是这样定义的).在win内部所有系统结构体中,bool一律是DWORD,Rust与win底层交互时,继承了win官方的bool规范
;; 3. win64下,cpu操作32位寄存器最原生和高效

;; 注释2
;; 在Intel和masm中,如果一个十六进制以a-f开头,前面必须补0(如 0FFh).h代表Hexadecimal 十六进制. 210h=528 十进制
;; win64下 rsp地址必须是16字节对齐的(能被16整除).否则会出现非法指令或内存对齐崩溃
;; 关于4kb缺页.win64规定如果汇编代码单次sub rsp超过或等于4kb(一个内存页),必须调用crt库的__chkstk函数逐页探测内存,如果不直接调用giant探测函数直接减去4kb,会触发STATUS_GUARD_PAGE_VIOLATION 内存崩溃
;; 对该函数:
;; 1. 进入该函数时,Rust call压栈返回地址,rsp-8
;; 2. 3次push 让rsp-24.此时rsp=32 满足16字节对齐
;; 3. sub rsp,210h 以此为锚点,开辟安全空间.开辟的空间也满足16字节对齐
;; 4. mov rbp,rsp;将rbp锚定在此,作为函数栈帧的基准指针.用于操作局部变量,准备跳板和终结符
;; 5. 伪造4层假调用栈: RtlUserThreadStart->BaseThreadInitThunk->SecondFrame (RBP 模式)-> FirstFrame (RSP 模式)
;; 6. ROP Gadget跳板和11个参数(Shadow Space + Stack Args)
;; 必须开辟安全空间:
;; 1. 当线程触发敏感的NtAllocateVirtualMemory,edr不仅在内核观察,还会通过用户态dll向当前线程插入apc或通过hook劫持控制流.edr注入的hook在执行时,会在当前线程栈顶开辟自己的栈帧.如果自己定义的真实数据(push rbp/rbx/r15)距离当前工作区太近,edr的hook routine在向下压栈写入局部变量时,会发生物理覆盖
;; 2. win的异常分发器SEH/VEH的栈下钻机制:如果目标函数在执行过程中,触发了任何软硬件异常(如 内存分页调整、STATUS_GUARD_PAGE_VIOLATION),win内核会调用用户态KiUserExceptionDispatcher.展开器RtlDispatchException 在寻找异常处理routine时,会在当前栈分配庞大的CONTEXT(1232字节)和DISPATCHER_CONTEXT 结构体.这也会占用栈空间
;; 3. 目标函数的参数,伪造的栈帧大小等不同因素,决定了目标函数执行完毕,返回rust时计算rsp比最初下移多少字节(恢复原始栈帧结构)的计算十分复杂.而这里,将安全空间(sub rsp,210h)的底端锚定在rbp中(mov rbp,rsp).在退出时(restoresynthetic),利用mov rsp,rbp在一个cpu时钟周期内就可以丢弃下方所有变长的假栈,之后再add rsp, 210h(丢弃安全空间),pop r15/rbx/rbp(注意和压栈顺序相反),再ret就优雅的返回了rust执行流
;; 但是这些可能占用栈空间的操作 怎么能保证落在开辟的210h中,而不是这个安全空间的前面和后面呢?
;; 正常执行流中,这210h空间是空置的,没有任何正常代码会向里面写入数据.后续所有的假栈,参数,目标api,edr的hook,win的异常分发器,都会在rbp向下更低的空间活动,不会占用这210h
;; 那何不将其置为0
;; 1. 在x64汇编和编译器生成的机器码中,有一个及其正常的现象,以基址指针正向偏移访问.如果空隙为0,那么rbp+0就是保存的r15; rbp+8就是保存的rbx; rbp+16就是保存到rbp.这会出现风险,如果有任何 如 编译器优化的局部变量访问,调试器探针,外部钩子代码尝试读取[rbp + 0x20]附近数据,就会踩进父函数物理私有空间,造成内存覆写.而210h,将r15/rbx/rbp 及父函数私有空间推到了210h之外的空间,这是绝对安全的高地.即使有几百个字节的正向探测或抖动,也触碰不到rust的核心数据
;; 2. win10/11 下,如果一个函数使用了rbp帧指针,但栈分配的大小为0,这是及其罕见且反常的.正常的系统业务函数,局部变量栈空间大小一般在100-200h之间.
;; 附 栈帧结构
; 高地址 (栈底)
; 
;   ┌────────────────────────────────────────────────────────────────────────┐
;     │ 1. Rust 父函数的真实栈空间 (局部变量、生命周期引用的内存)
;   │
;   ├────────────────────────────────────────────────────────────────────────┤
;     │ 2. 压栈备份的非易失寄存器: [RSP+228h] push rbp
;   │
;     │                           [RSP+220h] push rbx
;   │
;     │                           [RSP+218h] push r15
;   │
;   ├────────────────────────────────────────────────────────────────────────┤
;     │ 3. ★【0x210 (528 字节) 物理隔离安全气囊 / Firewall Buffer】★
;   │
;     │    (在此区域内，无论发生什么抖动，上下两端互不干扰)
;   │
;   ├────────────────────────────────────────────────────────────────────────┤
;   ◄── 【当前 RBP 锚定在此！】(mov rbp, rsp)
;     │ 4. 准备跳板与终结符:       [RBP-08h] push RestoreSynthetic
;   │
;     │                           [RBP-10h] push 0 (NULL, 终结展开)
;   │
;   ├────────────────────────────────────────────────────────────────────────┤
;     │ 5. 伪造的 4 层假调用链:   RtlUserThreadStart
;   │
;     │                           BaseThreadInitThunk
;   │
;     │                           SecondFrame (RBP 模式)
;   │
;     │                           FirstFrame (RSP 模式)
;   │
;   ├────────────────────────────────────────────────────────────────────────┤
;     │ 6. ROP Gadget 跳板与 11 个参数 (Shadow Space + Stack Args)
;   │
;   └────────────────────────────────────────────────────────────────────────┘
;   ◄── 当前 RSP (真正执行 syscall 的位置)