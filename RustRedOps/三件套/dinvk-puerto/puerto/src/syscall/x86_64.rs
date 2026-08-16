use core::ffi::CStr;
use core::{ffi::c_void, slice::from_raw_parts,ptr::read};

use crate::helper::PE;
use crate::hash::{fnv1a_utf16_from_u8};

use crate::syscall::{RANGE,DOWN,UP};

// retrieve the ssn
pub fn ssn(_func_hash:u32,module:*mut c_void)->Option<u16> {
    
    unsafe {

        // retrieve the export directory and the module hash
        let export_dir = PE::parse(module)
        .exports()
        .directory()?;

        let module =module as usize ;

        // retrieve names\func\oridinals index from export directory

        // names[i]指向的是以 `\0` 结尾的 ASCII字符串
        let names = from_raw_parts((module + (*export_dir).AddressOfNames as usize) as *const u32, 
    (*export_dir).NumberOfNames as usize);

        // functions,在内存处数据类型是u32(RVA)数组;加上基址后指向的是机器码(Opcode),在rust中类型是*const i8或*const u8,占用8字节(u64或usize).其指向的syscall stub是32字节的
        let functions = from_raw_parts((module+(*export_dir).AddressOfFunctions as usize)as *const u32,(*export_dir).NumberOfFunctions as usize);

        // 它就是一个简单的 u16 数字数组，里面的数字直接拿来当做functions 数组的下标使用
        let ordinals = from_raw_parts(
            (module + (*export_dir).AddressOfNameOrdinals as usize) as *const u16, 
            (*export_dir).NumberOfNames as usize
        );

        // 通过三个索引获取地址,names[i]和ordinals[i]是对应的,names[i]中存函数名,ordinals[i]中存functions的索引号,然后用functions获取真实地址
        // rust 有专门针对裸指针的操作 add offset.这里需要整理,并考虑是否应用到puerto项目中
        // 这里为什么用isize? 为了配合Halo's Gate对负方向的内存检索
        for i in 0..(*export_dir).NumberOfNames as isize  {
            // 遍历ordinals索引(实质是一个数组)的到其在数组中存储的值,该值对应functions index的序号,用于获取真实的地址
         let ordinal = ordinals[i as usize] as usize;   

            // 转为*const u8 方便逐字节对比(三种gate)特征
         let address = (module + functions[ordinal] as usize) as *const u8;

         // 传入的func_hash与names[]指向的数据的hash对比,确认
         let name_ptr = (module + names[i as usize] as usize ) as *const i8;
        
        let name_ptr_tou8=CStr::from_ptr(name_ptr).to_bytes();
         // 调用hash函数并比较
         if fnv1a_utf16_from_u8(name_ptr_tou8)==_func_hash {
             
            // 如果hash对上了,使用Hells Gate获取ssn(不通过任何API，直接从 `ntdll.dll` 的内存中“偷”出系统调用号（SSN）)
            // 原理:win10/11的ntdll.dll中,绝大多数系统调用函数的汇编指令是高度统一的```asm  mov r10, rcx       ; 机器码: 4C 8B D1
            // mov eax, 0x0018    ; 机器码: B8 18 00 00 00 (这里的 0x0018 就是 SSN)```

            // 这里read函数需要详细了解?
            // 对address逐字节迭代,找到符合mov r10,rcx mov eax, 0x0018特征码
            // 
            if read(address)==0x4c
            && read(address.add(1))==0x8B
            && read(address.add(2))==0xD1
            // 开始检查rcx mov eax机器码: B8 18 00 00 00
            // 0xB8是mov eax ,<imm32>指令操作码(Opcode).它告诉 CPU：“接下来我要把一个32 位的整数放进 EAX 寄存器”
            && read(address.add(3))==0xB8
            // 第4 5 6 7字节处存放的是ssn的数据(cpu指令的要求),ssn是u16,所以6 7处为0
            && read(address.add(6))==0x00
            && read(address.add(7))==0x00
             {
                let high =read(address.add(5)) as u16 ;
                let low = read(address.add(4)) as u16;

                //在 x86 架构中，数据是小端序（Little-endian）存储的 .address.add(4) 指向 18，address.add(5) 指向 00
                // (0x00 << 8) | 0x18 得到的结果就是 0x0018
                let ssn = (high << 8) | low;            // 拼接成一个 u16 类型的 SSN
                return Some(ssn);

            }

            
         }

         // Halos Gate
            // 原理:检测是否被EDR Hook,当EDR(如 CrowdStrike, SentinelOne 等)修改ntdll.dll内存中函数的前几个字节,写成一个JMP指令(x64机器码0xE9)时,Hell's Gate无法直接读到SSN
            // 应对:邻里检索（Neighboring Search）,利用ntdll.dll特性,系统调用的 SSN通常是连续的，且函数在内存中的排列也是顺序的
         if read(address) == 0xE9 {
                    for idx in 1..RANGE {
                        // check neighboring syscall down
                        if read(address.add(idx * DOWN)) == 0x4C
                            && read(address.add(1 + idx * DOWN)) == 0x8B
                            && read(address.add(2 + idx * DOWN)) == 0xD1
                            && read(address.add(3 + idx * DOWN)) == 0xB8
                            && read(address.add(6 + idx * DOWN)) == 0x00
                            && read(address.add(7 + idx * DOWN)) == 0x00 
                            {
                                let high = read(address.add(5 + idx * DOWN)) as u16;
                                let low = read(address.add(4 + idx * DOWN)) as u16;
                                let ssn = (high << 8) | (low - idx as u16);
                                return Some(ssn);
                            }
    
                        // check neighboring syscall up
                        if read(address.offset(idx as isize * UP)) == 0x4c
                            && read(address.offset(1 + idx as isize * UP)) == 0x8B
                            && read(address.offset(2 + idx as isize * UP)) == 0xD1
                            && read(address.offset(3 + idx as isize * UP)) == 0xB8
                            && read(address.offset(6 + idx as isize * UP)) == 0x00
                            && read(address.offset(7 + idx as isize * UP)) == 0x00 
                            {
                                let high = read(address.offset(5 + idx as isize * UP)) as u16;
                                let low = read(address.offset(4 + idx as isize * UP)) as u16;
                                let ssn = (high << 8) | (low + idx as u16);
                                return Some(ssn);
                            }
                    }
                }

                // Tartarus Gate
                if read(address.add(3)) == 0xE9 {
                    for idx in 1..RANGE {
                        // check neighboring syscall down
                        if read(address.add(idx * DOWN)) == 0x4C
                            && read(address.add(1 + idx * DOWN)) == 0x8B
                            && read(address.add(2 + idx * DOWN)) == 0xD1
                            && read(address.add(3 + idx * DOWN)) == 0xB8
                            && read(address.add(6 + idx * DOWN)) == 0x00
                            && read(address.add(7 + idx * DOWN)) == 0x00 
                            {
                                let high = read(address.add(5 + idx * DOWN)) as u16;
                                let low = read(address.add(4 + idx * DOWN)) as u16;
                                let ssn = (high << 8) | (low - idx as u16);
                                return Some(ssn);
                            }
                            
                        // check neighboring syscall up
                        if read(address.offset(idx as isize * UP)) == 0x4c
                            && read(address.offset(1 + idx as isize * UP)) == 0x8B
                            && read(address.offset(2 + idx as isize * UP)) == 0xD1
                            && read(address.offset(3 + idx as isize * UP)) == 0xB8
                            && read(address.offset(6 + idx as isize * UP)) == 0x00
                            && read(address.offset(7 + idx as isize * UP)) == 0x00 
                            {
                                let high = read(address.offset(5 + idx as isize * UP)) as u16;
                                let low = read(address.offset(4 + idx as isize * UP)) as u16;
                                let ssn = (high << 8) | (low + idx as u16);
                                return Some(ssn);
                            }
                    }
                }

        }

        
    }

    None
}




/// retrieve the syscall address from a given function address:在指定函数物理内存中(输入va)扫描syscall; ret指令(0x0F 0x05 0xC3).
/// 返回该指令在ntdll.dll的VA
/// 关于其返回值Option<u64> 详见注释2
pub fn get_syscall_address(address:*mut c_void)->Option<u64> {
    if address.is_null() {
        return None;
    }

    unsafe {
        let address = address.cast::<u8>();
        // 在u8类型能表达的255字节范围内,检索违背篡改的syscall指令
        // 关于(1..255) 见注释1
        (1..255).find_map({|i|{
            if read(address.add(i))==0x0F
            && read(address.add(i+1))==0x05
            && read(address.add(i+2))==0xC3 {
                Some(address.add(i) as u64 ) 
                
            }
            else {
                None
            }
        }})
    }
}
// 这里*mut c_void代表raw pointer,这是为了接收c类型的原始指针.那么*const c_void中的const代表什么?如果我之后不再改变这个返回值,是不是可以不用const,还是说const代表杜绝执行过程中其他对这个返回值的更改,比如执行中可能被编译器或者os的修改?请详细展开讲讲.





// 需要整理的知识点
// address: *mut c_void  let address = address.cast::<u8>(); 使用cast对指针进行转换
// CStr::from_ptr(name_ptr).to_bytes(); 将一个rust表示的原始指针转为C string 再转为&[u8] ,方便使用迭代器及其他rust core功能
// rust中裸指针的方法,u8 u16 u32之间的转换总结(peb pe 文件中)


// 注释1,关于(1..255)
// 这里有两个深层次问题 1. 机制上(1..255)为什么能够操纵并遍历address之后的物理内存; 2. 安全性上,凭什么断定address后面的这255字节在os中是真实存在,合法,可读的内存.
// 从Rust语法上(1..255)并没有代表内存,只是类似c的循环计数for循环.在后续的闭包中,以address为基准逐步加1(1字节).以此,逐步遍历address之后的255字节
// os层面:address后续的这255字节,会不会没有被分配,读取它会不会导致程序直接崩溃(access violation 0xC0000005).这涉及win10/11 对PE文件(ntdll.dll)的物理内存管理
// 1. 4kb物理内存分页与页面属性(page protections):win把ntdll映射到进程内存时,以4kb为一个物理页page进行连续内存管理.ntdll的.text节有几百k到几mb之间,且整个.text截取被统一赋予了rx权限.
// 2. 函数在.text节中是紧密连续排列的:address是ntdll导出的某个native api的入口首地址.ntdll代码段中成千上百的系统调用函数是紧挨着连续存放的.即使某个函数自身只有32字节长,往后读取255字节物理上依然处于ntdll的.text节区内部(最多读取到下一个函数或填充字节),由于整片内存区域都是可读的,cpu绝不会出发内存违规异常page fault.
// 如果恰好读到最后一个函数,且这个函数很小,导致读取255字节超过ntdll的.text节区范围呢?
// win 10/11下,即使读取超过.text节区末尾,物理上也绝不会触发内存违规异常(page fault/0xC0000005)
// PE镜像在虚拟内存紧密连续映射:即win内核加载器将ntdll映射到进程虚拟内存空间时,在其虚拟内存地址空间排列:PE头(只读)->.text(代码段,rx)->.rdata(只读数据段,只读)->.data(全局数据,rw).所以即使越过.text,cpu内存管理单元mmu绝不会抛出内存不可读异常
// 如果非要杜绝任何极端异常可以
// 1. 将范围从(1..255)的扫描窗口缩小到(1..32).在32字节内100%覆盖win 10/11的所有系统调用桩(待验证)
// 2. 下面是错误的,因为win abi规范,只有会开辟栈帧,修改rsp,包含异常处理函数才强制要求在.pdata中注册IMAGE_RUNTIME_FUCTION,许多版本的win或特定dll中,纯叶子函数在.pdata中没有独立的展开条目.//通过查询该函数在.pdata展开表中EndAddress - BeginAddress ,精准限制扫描范围,绝不超过该函数的物理终止地址:let max_len =  (runtime.EndAddress - runtime.BeginAddress) as usize;  (1..max_len).find_map(|i| { ... })
// 注意这里传入函数的参数 代表的是函数地址而不是整个.text节区的地址.所以扫描的是对应函数区间而不是整个.text节区.
// 注意这里并不是函数prologue而是函数在用户态的全部代码,即整个函数本身.因为ntdll中的函数本身一般都很小,所有复杂操作都是ntdll中的函数在syscall后,进入内核ntoskrnl.exe中执行的

// 注释2
// 曾将返回值用*const c_void代替原uwd.rs中的u64.但权衡之后,应当尊重源码的u64选择:
// 1. 线程安全性-Rust编译器对裸指针的线程隔离:u64/usize作为基本数据类型,天生具有send和sync trait,可以在多线程之间传递或跨线程共享;而*const c_void/*mut c_void,在编译器视角下这两种raw pointer被视为非线程安全,默认没有实现send/sync
//  1.1 后果:如果某结构体/全局配置,包含get_syscall_address返回的指针,那么这样的结构体就是去了send/sync,导致后续无法将其放到arc,static变量,或跨线程调度(如 hypnus/samoa中利用不同线程池的技术)
// 2. 内存混淆与位运算--对抗内存扫描和 sleep obfuscation:如hypnus-samoa中做的堆栈与上下文混淆,地址经常需要进行XOR异或加密,掩码位移,对齐计算(如 addr & !0xFFF),这种情况u64天然有优势,而裸指针,编译器严禁对指针做位运算,必须先as usize算完再as *const c_void
// 3. LLVM严格指针来源规则:在LLVM/Rust优化体系中:指针不仅是一个地址数字,还附带内存来源信息(provenance).在get_syscall_address找到的va属于人工制造的指针.将其定义为u64,是向编译器和静态分析器表明,这是一个纯粹的64位整数值, 不要尝试对其做指针生命周期分析.
// 以上,可以尝试使用u64