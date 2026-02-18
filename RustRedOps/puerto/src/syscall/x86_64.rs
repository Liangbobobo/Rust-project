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












// 需要整理的知识点
// address: *mut c_void  let address = address.cast::<u8>(); 使用cast对指针进行转换
// CStr::from_ptr(name_ptr).to_bytes(); 将一个rust表示的原始指针转为C string 再转为&[u8] ,方便使用迭代器及其他rust core功能
// rust中裸指针的方法,u8 u16 u32之间的转换总结(peb pe 文件中)