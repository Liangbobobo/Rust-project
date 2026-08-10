// 本文件涉及的结构体之间的联系
// (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION]:异常表入口.DataDirectory是包含16个元素的数组,每个元素的类型是IMAGE_DATA_DIRECTORY（数据目录项）.这里是DataDirectory的第4个元素(index是3),其记录整个异常表(.pdata节)在内存中的RVA,其类型也是IMAGE_DATA_DIRECTORY.其内部字段VirtualAddress：异常表在内存中的相对虚拟偏移量（RVA）,Size：异常表在内存中的总字节大小.内核通过 pe_base + VirtualAddress得到指向下一步结构体(IMAGE_RUNTIME_FUNCTION)数组的首地址指针.指向一个连续存放在.pdata节中的结构体数组(微软文档称为Exception table,但实质上是一个匿名数组),其类型是IMAGE_RUNTIME_FUNCTION
// ->
// IMAGE_RUNTIME_FUNCTION(运行时函数目录项,12字节),代表每个函数机器码区间和unwind信息.其内部字段BeginAddress (u32 RVA), EndAddress (u32 RVA),UnwindData (u32 RVA).内核判断cpu的rip是否落在[BeginAddress, EndAddress) 这个左闭右合区间内.如果是内核通过e_base + UnwindData得到指向下一步结构体UNWIND_INFO首地址的指针.如果不是就报异常
// ->
// UNWIND_INFO(4字节头+动态长度)代表对应函数的转速退栈控制信息(通常在 .rdata节).代表该函数栈分配信息,即函数启动时预留的栈空间大小,并挂载具体操作码.其内部字段1. VersionFlags (1 字节)：存放版本号和标志（如判断是不是链式UNW_FLAG_CHAININFO 或含异常处理 UNW_FLAG_EHANDLER）;2. CountOfCodes (1 字节)：记录了后面紧跟的 UNWIND_CODE数组中有多少个操作码;3. FrameInfo (1 字节)：记录是否使用帧指针（如 RBP）;4. UnwindCode：指向下一步的第一个数组元素.在UNWIND_INFO的前四个字节头部后,就是连续的UNWIND_CODE结构体数组
// UNWIND_CODE:其类型是union,大小2字节.具体表示为哪个字段,永远先看Anonymous的unwindop确定指令含义(压栈还是分配内存),如果unwindop中是普通指令(如UWOP_PUSH_NONVOL)则UNWIND_CODE表示为Anonymous.如果是UWOP_ALLOC_LARGE这种,说明这里不能表示全部指令,此时这里仍然表示为Anonymous字段,紧随其后的下一个UNWIND_CODE槽位表示为FrameOffset,代表cpu分配了多少内存空间.其第一个字段是FrameOffset: u16,使用时表示放弃位域拆解,把这个字段当作16位无符号整数.第2个字段是操作码数组,2字节.描述单步汇编级别的栈操作(如分配了32字节栈,RBX压栈).其内部以UNWIND_CODE_0位域解析:CodeOffset代表操作发生在prologue的第几个字节;UnwindOp:动作类型(压栈,分配栈,移动栈);OpInfo:动作涉及的寄存器.
// 内核会循环读取CountOfCodes个UNWIND_CODE,累加出该函数在prologue中共占用的栈空间字节数.如果数量是奇数会跳过2字节的对齐填充.指针指向最后的收尾union
// ->
// 跳过所有的操作码后,指针最终落到 union UNWIND_INFO_0(收尾路由union,大小4字节).作用:当函数执行发生异常,决定下一步动作.
// 其内部字段:ExceptionHandler(u32 RVA):去异常处理程序.如果Flags包含UNW_FLAG_EHANDLER,则指向语言特定异常分发器(如c的__C_specific_handler)
// 其内部字段:FunctionEntry(u32,RVA):去父级函数.如果Flags包含UNW_FLAG_CHAININFO,则指向父级函数的IMAGE_RUNTIME_FUNCTION
// 如果走向FunctionEntry,系统会拿到这个RVA指向的IMAGE_RUNTIME_FUNCTION回到父函数,递归的对父函数进行退栈,知道追溯到线程起点.
// 如果走向ExceptionHandler,则把控制权交给异常分发器

#![allow(unused)]

use core::{ffi::c_void, slice::from_raw_parts};
use puerto::{helper::PE, types::*};

/// indicates the presence存在 of an exception handle in the function:win64 SEH开关
pub const UNW_FLAG_EHANDLER: u8 = 0x1;

/// indicates chained unwind information is present:SEH中用于处理复杂,非连续函数或编译器优化的高级物理开关.如果在UNWIND_INFO中这个标志位被设为1,意味着当前结构体只描述了该函数一部分栈行为.
/// 某些情况下win无法用单一的.pdata记录项(对复杂函数, win在.pdata节中创建多个条目记录 entries)描述其栈行为.win因此引入了链式回溯:将函数拆分为多个子区域,每个子区域拥有自己的UNWIND_INFO,但通过UNW_FLAG_CHAININFO指向同一父函数的UNWIND_INFO
pub const UNW_FLAG_CHAININFO: u8 = 0x4;

/// provide access to the unwind(exception handling)information of a PE image
#[cfg_attr(debug_assertions, derive(Debug))] // 代替#[derive(Debug)]:在debug编译期派发Debug属性,在release期剔除Debug属性
pub struct Unwind {
    pub pe: PE,
}

impl Unwind {
    /// create a ne Unwind:实质上就是ntdll.dll 或 kernel32.dll的PE.用以摸清dll文件内部函数的栈深度.后续通过汇编在内存中模拟这些内部函数的栈结构
    pub fn new(pe: PE) -> Self {
        Unwind { pe }
    }

    /// return all runtime function entries
    pub fn entries(&self) -> Option<&[IMAGE_RUNTIME_FUNCTION]> {
        let nt = self.pe.nt_header()?;

        // pe.OptionalHeader->DataDirectory(异常数据目录项,16个类型为IMAGE_DATA_DIRECTORY的数组)->索引3(即 .pdata节描述符).
        // 这里记录了函数回溯表exception directory的RVA与大小.
        // dir.VirtualAddress+pe.base指向的是连续存放IMAGE_RUNTIME_FUNCTION结构体数组的起始地址
        let dir = unsafe { (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION] };
        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return None;
        }

        let addr =
            (self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_RUNTIME_FUNCTION;
        let len = dir.Size as usize / size_of::<IMAGE_RUNTIME_FUNCTION>();

        Some(unsafe {
            // 返回slice即&[IMAGE_RUNTIME_FUNCTION],方便后续像使用数组那样调用iter() find()等方法
            from_raw_parts(addr, len)
        })
    }

    /// find a runtime function by its RVA
    /// 为了对抗ASLR IMAGE_RUNTIME_FUNCTION中的三个字段都是RVA(u32)
    pub fn function_by_offset(&self, offset: u32) -> Option<&IMAGE_RUNTIME_FUNCTION> {
        self.entries()?.iter().find(|f| f.BeginAddress == offset)
    }

    /// gets the size in bytes of a function using the unwind table
    ///
    // 条件编译:当前项目在编译时启用了desync的特性时,才编译下面的函数/代码块.否则忽略
    #[cfg(feature = "desync")]
    // 这里的func就是VA(绝对虚拟地址).具体异常函数目录结构见注释1
    pub fn function_size(&self, func: *mut c_void) -> Option<u64> {
        let offset = (func as usize - self.pe.base as usize) as u32;

        let entry = self.function_by_offset(offset)?;

        let start = self.pe.base as u64 + entry.BeginAddress as u64;
        let end = self.pe.base as u64 + entry.EndAddress as u64;

        Some(end - start)
    }
}

/// configuration structure passed to the spoof ASM routine
#[repr(C)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct Config {
    /// address rtluserthreadstart
    pub rtl_user_addr: *const c_void,

    ///stack size rtluserthreadstart
    pub rtl_user_thread_size: u64,

    /// address basethreadinitthunk
    pub base_thread_addr: *const c_void,

    /// stack size basethreadinitthunk
    pub base_thread_size: u64,

    /// fist(fake) return address frame
    pub first_frame_fp: *const c_void,

    /// second(ROP) return address frame
    pub second_frame_fp: *const c_void,

    /// gadget:jmp [rbx]
    pub jmp_rbx_gadget: *const c_void,

    /// gadget:add rsp,X; ret
    pub add_rsp_gadget: *const c_void,

    /// stack size of first spoofed frame
    pub first_frame_size: u64,

    /// stack sie of second spoofed frame
    pub second_frame_size: u64,

    /// stack frame size where the jmp [rbx] gadget resides常驻
    pub jmp_rbx_frame_size: u64,

    /// stack frame size where the add rsp,X gadget resides
    pub add_rsp_frame_size: u64,

    /// offset on the stack where rbp is pushed
    pub rbp_stack_offset: u64,

    /// the function to be spoofed/called
    pub spoof_function: *const c_void,

    /// return address(used as stack-resume point after call)
    pub return_address: *const c_void,

    /// checks if the target is a syscall
    pub is_syscall: u32,

     /// System Service Number (SSN)
    pub ssn: u32,

    /// arguments that will be passed to the function that will be spoofed
    pub number_args: u64,
    pub arg01: *const c_void,
    pub arg02: *const c_void,
    pub arg03: *const c_void,
    pub arg04: *const c_void,
    pub arg05: *const c_void,
    pub arg06: *const c_void,
    pub arg07: *const c_void,
    pub arg08: *const c_void,
    pub arg09: *const c_void,
    pub arg10: *const c_void,
    pub arg11: *const c_void,
}

impl Default for Config {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

/// enumeration of x86_64 general-purpose register
///
/// used in unwind parsing or register mapping logic
#[derive(Clone, Copy)]
#[cfg_attr(debug_assertions, derive(Debug))]
#[repr(u8)]
#[allow(dead_code)]
pub enum Registers {
    Rax = 0,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

impl PartialEq<usize> for Registers {
    fn eq(&self, other: &usize) -> bool {
        *self as usize == *other
    }
}

/// structure containing the unwind information of a function
///
/// 动态大小,基础头部固定为4字节,紧随其后的CountOfCodes(1字节)指定UNWIND_CODE(2字节)数组的数量,随后是可选的UNWIND_INFO_0(4字节)和ExceptionData(4字节)
#[repr(C)]
pub struct UNWIND_INFO {
    /// separate structure via containing Version and flags:低3位的Version(unwind结构的版本号,win64固定为1)和高5位的Flags(0x1=UNW_FLAG_EHANDLER,表示有异常处理程序,0x4=UNW_FLAG_CHAININFO,表示本段是链式回溯,指向父函数)
    pub VersionFlags: UNWIND_VERSION_FLAGS,

    /// size of the function prologue in bytes:prologue的字节数
    pub SizeOfProlog: u8,

    /// number of non-array UnwindCode entries:UnwindCOde数组中元素的个数,每个元素即操作码占用2字节
    pub CountOfCodes: u8,

    /// separate structure containing FrameRegister and FrameOffset:如果函数建立了帧指针,低4位记录作为帧指针的寄存器编号(如 5代表rbp).如果为0表示不使用帧指针(纯rsp寻址);高4位:帧指针寄存器相对于rsp的偏移量(实际字节数为该值乘以16.为啥是16,因为:1. win64下,一个寄存器8字节,而高4位最大只能表示15字节 2. win64下应用程序二进制接口abi规定,cpu的rsp在调用任何函数时,必须保持16字节对齐.即帧指针寄存器如rbp,必须是16的倍数 3. 这里使用了乘法缩放,即高4位每种变化都代表16字节,表示的偏移量在0至15*16=240字节)
    pub FrameInfo: UNWIND_FRAME_INFO,

    /// Array of unwind codes describing specific operations.UNWIND_CODE是一个union,其第二个字段指向一个动态数组,长度由CountOfCodes决定.但UnwindCode大小是2字节,因为在union UNWIND_CODE内部,没有任何指针,它的两个成员都只有2字节.union的大小等于它最大成员的大小.其内部字段分别是FrameOffset: u16和Anonymous: UNWIND_CODE_0(16位包装结构体,2字节,是数组第一个元素本身),即其内部不存储任何数组指针,只存数据本身. 详见注释4
    pub UnwindCode: UNWIND_CODE,

    /// 在以C表示的微软官方文档中,该结构体在pub UnwindCode: UNWIND_CODE这里就结束了.后面的UNWIND_INFO_0和ExceptionData是可选的,追加在数组后面的数据,因此不写入结构体定义中.因为c不支持在一个可变长度数组(即 这里的UNWIND_CODE)后面生命其他静态字段.
    /// 这里加上这两个字段原因,为了让阅读代码的人知道还有这两个字段,以及让rust能够编译这两个类型.但这也导致rust编译出来的UNWIND_INFO结构体变成一个大小固定(14字节)的错位结构体.详见 注释3
    /// array of unwind codes decribing sepecific opreations
    pub Anonymous: UNWIND_INFO_0,

    /// optional exception data
    pub ExceptionData: u32,
}

/// union representing a single unwind operation code
///
/// 是UNWIND_INFO的一个字段,是一个union.详见注释6
#[repr(C)]
pub union UNWIND_CODE {
    /// offset into the stack frame for the opereation
    pub FrameOffset: u16,

    /// structured fields of the unwind code
    pub Anonymous: UNWIND_CODE_0,
}

/// union representing optional exception handler or chained function entry
/// 
/// 该union具体表现为哪个字段,取决于UNWIND_INFO结构体头部的VersionFlags.Flags()的值.
#[repr(C)]
pub union UNWIND_INFO_0 {
    /// address of the exception handler(RVA)
    pub ExceptionHandler: u32,

    /// address of a chain function entry
    pub FunctionEntry: u32,
}

/// 操作码枚举.unwind operation codes used by the windows x64 exception handling model:https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64
/// 
/// 它并没有定义在UNWIND_INFO中,而是一个独立的enum.用来翻译UNWIND_CODE.Anonymous(2字节,类型 UNWIND_CODE_0,)其中4这个位域值UnwindOp(4bit)对应的操作码.其背景知识见注释5;具体如何翻译见注释7
#[repr(u8)]
#[allow(dead_code)]
pub enum UNWIND_OP_CODES {
    // 非易失性寄存器压栈（如 push rbp）.RSP 自动减8,OpInfo 记录被压栈的寄存器编号,占用1个slot.对应位域位于UNWIND_CODE.Anonymous(UNWIND_CODE_0)中高4比特位(12-15位),小端序.详见注释6
    UWOP_PUSH_NONVOL = 0,
    // 分配大段栈空间。若 OpInfo = 0 占用 2 个slots，下一 slot 乘以 8 是分配量；若 OpInfo = 1 占用 3 个 slots，下两slots 组合是分配量
    UWOP_ALLOC_LARGE = 1,
    // 分配小段栈空间（8 至 128字节）。分配大小计算公式为 (OpInfo + 1) * 8，占用 1 个 slot
    UWOP_ALLOC_SMALL = 2,
    UWOP_SET_FPREG = 3,
    UWOP_SAVE_NONVOL = 4,
    UWOP_SAVE_NONVOL_BIG = 5,
    UWOP_EPILOG = 6,
    UWOP_SPARE_CODE = 7,
    UWOP_SAVE_XMM128 = 8,
    UWOP_SAVE_XMM128BIG = 9,
    UWOP_PUSH_MACH_FRAME = 10,
}

// 利用rust的transmute的一段极其优雅,零开销,内存安全的类型转换.使UNWIND_OPCODES类型得到了try_from()和try_into()方法
impl TryFrom<u8> for UNWIND_OP_CODES {
    // 转换失败返回()空单元类型,不占用多余错误字符串内存
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // 0..=10 闭区间语法,匹配0-10这个11个u8类型的数字
            0..=10 =>Ok(unsafe {
                core::mem::transmute::<u8,UNWIND_OP_CODES>(value)
            }),
            _=>Err(()),
        }
    }
}

bitfield::bitfield! {
    /// bitfield representation of an unwind code entry
    /// 
    /// 一个u16大小的位域,代表prologue中某条修改栈解构的汇编指令的回溯metadata,
    /// -CodeOffset代表对应的汇编指令执行完毕后,相对prologue起始位置的字节偏移量;
    /// -UnwindOp代表对栈做了什么操作(如压栈,开辟空间,设置帧指针rbp);
    /// -OpInfo是对UnwindOp的附加参数(压栈对应的寄存器,开辟空间的大小等)
    #[repr(C)]
    #[derive(Clone,Copy)]
    #[cfg_attr(debug_assertions, derive(Debug))]
    pub struct UNWIND_CODE_0(u16);

    /// byte offset from the start of the proluge where this operation is applied
    /// -CodeOffset代表对应的汇编指令执行完毕后,相对prologue起始位置的字节偏移量
    pub u8,CodeOffset,SetCodeOffset:7,0;

    /// the unwind operation code
    /// -UnwindOp代表对栈做了什么操作(如压栈,开辟空间,设置帧指针rbp);
    pub u8,UnwindOp,SetUnwindOp:11,8;

    /// additional operation-specific information
    /// -OpInfo是对UnwindOp的附加参数(压栈对应的寄存器,开辟空间的大小等)
    pub u8,OpInfo,SetOpInfo:15,12;
}

bitfield::bitfield! {
/// bitfield representation of an unwind code entry
///
/// 关于bitfield::bitfield! 相关详见注释2
#[repr(C)]
#[derive(Clone,Copy)]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct UNWIND_VERSION_FLAGS(u8);

/// unwind info format version
pub u8,Version,SetVersion:2,0;

/// unwind flags
pub u8,Flags,SetFlags:7,3;
}

bitfield::bitfield! {
    /// compact紧凑的 representation of frame register and offset fields
    #[repr(C)]
    #[derive(Clone,Copy)]
    #[cfg_attr(debug_assertions,derive(Debug))]
    pub struct UNWIND_FRAME_INFO(u8);

    /// the register used as the frame pointer
    pub u8,FrameRegister,SetFrameRegister:3,0;

    /// offset from the stack pointer to the frame pointer
    pub u8,FrameOffset,SetFrameOffset:7,4;
}

// 注释1
// (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION]  即 .pdata异常函数表,其在内存中是一个由几千个 IMAGE_RUNTIME_FUNCTION 组成的数组
// 每个IMAGE_RUNTIME_FUNCTION是表中的一个条目,本身不包含具体的汇编指令(对应的汇编指令,即机器码在.text节中).其三个字段分别表示对应函数机器码内存开始位置,内存结束位置,对应的栈是如何分配的

// 注释2
// Rust原生不支持c的位域模式(将固定字节不同的位分别赋值,用以代表不同的含义).必须使用第三方库bitfield::bitfield! 宏：定义一个只占用 1字节的元组结构体 UNWIND_VERSION_FLAGS(u8)，并自动为它生成位移（<< />>）和按位与（&）的 Get/Set 函数
// 以上,该一字节在内存中的8bit(0-7)被切割,低三位用于记录unwind的Version信息,高5位用于记录unwind的Flags信息,存放前文定义的NW_FLAG_EHANDLER (0x1)、UNW_FLAG_CHAININFO(0x4) 等标志
// 此外,该宏在底层自动为UNWIND_VERSION_FLAGS结构体实现四个内联函数:fn Version(提取低三位),fn SetVersion(设置低三位的值),fn Flags(提取高5位的值),fn SetFlags(设置高5位的值)
// 其在项目中作用:通过该宏生成的Flags()函数,来判断子区域是否存在链式信息

// 注释3
// 前文提到UNWIND_INFO结构体中有两个optional字段,c原型中通过宏或裸指针定位来访问这两个字段.那么这两个字段到底在内存中什么位置,或者说它们定义在什么地方
// 在c的视角下:头文件中的C结构体声明,仅仅是给程序员看的名片.在编译器生成二进制文件时,是在内存中映射数据的.
// 如何把这些可选字段写入内存的(编译器):在c中,如果源码包含__try/__except 时,编译器(MSVC/GCC)在编译期间:
// 1. 检测到有异常处理 2. 读取UNWIND_CODE栈操作码(假设有3个) 3. 编译器生成最终.exe/.dll文件时,会在系统.rdata节(只读节区)中,划出一片连续的物理字节空间,把对应的数据全部写入,包括:
// 1. 写入4字节UNWIND_INFO头部; 2. 写入6字节的UNWIND_CODE数组(3个元素); 3. 写入2字节对齐填充(4字节对齐); 4. 编译器在没有任何c结构体约束的情况下,强行在后面多写入4字节,存入ExceptionHandler地址; 5. 编译器再强行在其后写入若干字节,存入异常作用域数据(ExceptionData)
// 以上,在编译出来的文件中,这些数据已经物理存在了.
//
// 这些数据在内存中的情况:当程序运行被加载到内存后,这些字节区域完全是连续且静止的.以CountOfCodes=3为例,其内存layout如下
// 内存相对偏移         | 长度 (Byte) | 这块内存放的是什么 | 对应的概念名字
// -------------------|-------------|--------------------|--------------------
/* 0 ~ 3 */          /* 4 */       /* UNWIND_INFO */    /* 结构体头部 */
/*                   */
 /* 固定头部 (Version, */
/*                   */
 /* Flags 等)          */
/* 4 ~ 9 */
 /* 6 */
 /* 3 个 */
 /* UnwindCode 数组 */
/*                   */
 /* UNWIND_CODE（每个  */
/*                   */
 /* 2 字节）           */
/* 10 ~ 11 */
 /* 2 */
 /* 2 字节的对齐零填充 */
 /* 填充区 */
/* 12 ~ 15 */
 /* 4 */
 /* 异常处理器函数的   */
 /* UNWIND_INFO_0（可 */
/*                   */
 /* RVA 指针 (如       */
 /* 选字段） */
/*                   */
 /* __C_specific_handl */
/*                   */
 /* er)                */
/* 16 ~ 19 */
 /* 4 */
 /* 异常数据的大小/作  */
 /* ExceptionData（可 */
/*                   */
 /* 用域数             */
 /* 选字段） */

// 后续如何访问这两个optional字段
// 1. 异常发生:当程序崩溃,win内核(ntdll.dll中的RtlVirtualUnwind)接管执行流
// 2. 定位内存:内核通过.pdata找到上述连续内存的起点
// 3. 使用宏解码:如GetUnwindExceptionHandler(unwindInfo)
// 4. 路由成功：内核成功拿到了异常处理器的指针，开始执行你的 __except 代码

// 注释4
// 如前文,在运行时的内存中,UNWIND_CODE是一个由CountOfCodes决定的动态数组.但在rust语法声明中,无法直接在结构体内部声明一个动态大小的数组(因为结构体在编译期必须有固定大小).这就需要把内存中物理事实和rust/c语言的结构体声明语法分开来看,这个矛盾在底层用了一种妥协的解决方案:
// rust中可以用Vec表示一个动态数组,但Vec在底层是一个指针,在win的PE内存中,这个数组必须紧贴在FrameInfo后面,不能有指针跳转;也不能用[UNWIND_CODE;N],因为N是动态的,编译器无法确定.
// 因此,只能在结构体中声明单个UNWIND_CODE(2字节),它在语法上只是一个锚点/路标,用于标记动态数组的起点在这里
// 因为结构体声明只有2字节,如果在rust中unwind_info.UnwindCode，只能拿到第 1 个元素.uwd中将unwind_info.UnwindCode as *const UNWIND_CODE转为其在内存中的首地址的指针.后续通过unwind_info.CountOfCodes获取其len,最后通过slice::from_raw_parts强行恢复该动态数组的slice.(好强)

// 注释5
// win32时代,SEH是动态挂载到栈上的.程序运行到哪,就把异常处理函数地址压倒栈顶(FS:[0]链表).这带来了严重的性能开销,且极易受到栈溢出覆盖攻击(SEH Overwrite).
// win64下,微软废除了这种栈设计,改用查表法(Table-Driven Unwinding):
// 1.编译器记录一切:当编译器编译rust/c++源码时,知道并记录每行汇编是如何修改rsp的
// 2. 生成说明:编译器把这些修改栈的操作,翻译成原子操作码UNWIND_OP_CODES,并打包写入.pdata
// 3. 逆向执行退栈:当发生异常或edr扫描堆栈时,win回溯器(RtlVirtualUnwind)会读取这些操作码,逆向执行它们.如 UWOP_ALLOC_SMALL,回溯器执行rsp+=大小;UWOP_PUSH_NONVOL,回溯器执行pop 寄存器.通过这种方式,回溯器不需要真正运行代码,就能完美模拟函数返回过程,层层回溯到调用源头.
// 每个UNWIND_CODE结构体大小固定2字节(16位),称为一个slot(插槽)
// 对UWOP_ALLOC_SMALL,小栈分配,占用1个slot.由于4bit的Opinfo只能表示0-15.为了不占用额外内存,微软规定实际分配大小=(OpInfo+1)*8.如sub rsp,0x28(40字节),计算得到OpInfo=(40/8)-1=4.只需要这4个比特位就能表达40字节的分配.此情况下,函数分配的栈大小在8-128字节区间,.如果超过128字节,编译器就切换到UWOP_ALLOC_LARGE模式
// 对UWOP_ALLOC_LARGE,大栈分配,占用2-3个slot.如果函数sub rsp,0x10000(分配64k空间),4比特存不下.此时规定:如果分配量小于512k,OpInfo置为0,编译器在当前2字节操作码后面,强行再写入2字节(即第二个slot),来存放分配大小(分配大小除以8的值);
// 如果分配量大于512kb,OpInfo置为1,编译器会在后面强行再写入4个字节(第2,3个slot)来存放完整的32位分配大小.
// 所以,最小和最大分配的空间为8至4GB-8字节.这也是解析器(如uwd.rs)遇到UWOP_ALLOC_LARGE时,必须让循环指针自增+=2或+=3,跳过后面的参数插槽,否则会把大小当成下一个操作码来解析

// 注释6
// 每个UNWIND_CODE.Anonymous(UNWIND_CODE_0)中以小端序(little-endian)分布
// 如前文,UNWIND_CODE占2字节,第1个字节(0-7),完整存放位域中CodeOffset字段.该CodeOffset字段记录,当前这一步栈操作对应的汇编指令,距离该函数起点的相对字节距离.在生成.pdata异常表时,编译器会为每步操作各生产一个UNWIND_CODE结构体,并将每步汇编指令的偏移分别填入CodeOffset字段
// 第2字节的8bit,高4位(12-15)存放OpInfo,低4位(8-11)存放UnwindOp

// 注释7
// 如前UNWIND_CODE.Anonymous(2字节)的第1个字节存放对应的汇编指令偏移(CodeOffset).第2个字节的高4位为Opinfo,低4位为UnwindOp.编译器将UNWIND_CODE.Anonymous根据其位域中UnwindOp的值翻译为对应的UNWIND_OP_CODES字段(其每个字段代表不同的操作指令):
// 调用 unwind_code.Anonymous.UnwindOp() 时,bitfield!在底层:
// 1. 拿到Anonymous 的 16 位整数值;
// 2. 执行 (value >> 8) & 0x0F（向右移 8 位以跳过第一个字节的CodeOffset，然后与 1111 即 0x0F 做按位与，截取低 4 位）
// 3. 返回一个干净的 u8 整数值（取值范围必然是 0 到 15 之间）
// 4. 调用 Rust 语言的特征接口 TryFrom(impl TryFrom<u8> for UNWIND_OP_CODES中实现)
// 5. 在其实现中调用了core::mem::transmute (内存重解释)直接将其:这是一个零成本（Zero-Cost）的编译器指令。因为我们给 UNWIND_OP_CODES 加上了#[repr(u8)] 标记，这告诉编译器，这个枚举在内存里的大小就是 1 字节。所以，transmute 只是在编译期对类型进行了重新解释（直接把 u8 的数字认定为UNWIND_OP_CODES 枚举），在运行期不需要执行任何转换代码，效率极高
// 虽然UNWIND_OP_CODES是8位的,上面截取的是低4位.但仍然不会出错:1. 4 位的数据可以无损装进 8 位中,任何合法的 4 位操作码，都可以用 8 位的 u8 无损表示;
// 2. 当宏提取出 4 位的 UnwindOp（比如 10，二进制为 1010）并将其返回为 u8 时，CPU会自动在前面补零
// 3. #[repr(u8)] 确保了内存大小对齐.Rust 中，transmute 要求源类型和目标类型的物理大小必须完全一致.因为我们给 UNWIND_OP_CODES 加上了#[repr(u8)]，所以编译器在内存中给这个枚举分配的空间也是 8 位（1 字节）.因此，transmute 是把一个 8 位的 u8 转换成一个 8位的枚举，大小完全对等，安全编译通过