
use alloc::{
    format, vec::Vec, vec,
    string::{String, ToString}, 
};
use core::{
    ffi::{CStr, c_void}, ptr::null_mut, slice::from_raw_parts
};

// use crate::{hash};
use crate::error::Error;
use obfstr::obfbytes;
use crate::types::{HMODULE,LDR_DATA_TABLE_ENTRY};
use spin::Once;
use crate::winapis::{NtcurrentPeb};

/// crate a static variable to store the ntall.dll's address
/// 
/// 
static NTD:Once<u64>=Once::new();



// 在Windows中模块句柄(这里的HNOUDLE)和内存基址本质是一个东西
// HMODULE (句柄)：这是一个逻辑概念，属于 Win32 API的术语。它是一个不透明的标识符，用来代表一个已加载的模块。在 API 文档中（如GetModuleHandle），微软用它来“管理”对象
// Base Address (基址)：这是一个物理概念。它代表了这个 DLL文件被映射到进程虚拟内存空间中的起始内存地址
// 在 Windows 实现中，为了性能，`HMODULE` 的值直接就是该模块在内存中的基地址
// 在 Windows（从 Win32开始）的实现中，模块句柄（`HMODULE`）本质上就是该模块在进程虚拟地址空间中的起始地址
// 如果你拥有一个 HMODULE，你实际上就拥有了一个指向 IMAGE_DOS_HEADER 结构体的指针.在该地址的前 2 个字节，永远是 0x4D 0x5A（ASCII 码的 'MZ'）.这就是为什么你的项目（以及 dinvk）可以直接把 HMODULE 强转为指针，然后去读取e_lfanew（偏移 0x3C），从而找到 NT 头（PE 签名）
// 普通句柄 (如 `HANDLE` for File/Process/Thread)：它们通常是进程句柄表里的一个索引（Index），数值通常很小（如 4, 8, 12...）。你不能直接去读取这些数值指向的内存。模块句柄 (`HMODULE` / `HINSTANCE`)：它们是特例，直接就是内存地址

// 区分retrieve_moudle_add传入的参数类型
pub enum MoudleType<'a> {
    name(& 'a str),
    hash(u32),
    empty
}

#[inline(always)]
pub fn retrieve_moudle_add<T>(module:MoudleType,
hash_func:Option<fn(&[u16]) -> u32>)->Result<HMODULE,Error>
// where T:ToString
{   
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回匹配Result中Error类型的错误
    let hash = hash_func.ok_or(Error::HashFuncNotFound)?;

    // 下面主要是通过TEB->PEB->Ldr->InMemoryOrderModuleList(LIST_ENTRY)->LDR_DATA_TABLE_ENTRY(并非指向第一个字段,而是该结构体的中间位置的字段)
    // 然后在LDR_DATA_TABLE_ENTRY中找到模块在内存中的基址(提取出存储模块基址)

    

    // 这里的unsafe块能不能拆分出一部分放入safe里面
    unsafe {

        // 不调用windows api通过cpu的gs寄存器读取当前进程的peb
    // 
    // gs指向当前线程的TEB起始地址,其offset 0x60(win64)处指向peb地址
    let peb=NtcurrentPeb();

        let ldr = (*peb).Ldr;

        // windows loader初始化模块是固定的
        // (*ldr).InMemoryOrderModuleList指向PEB_LDR_DATA结构体中表头
        // 第一个flink是主程序自身的LDR_DATA_TABLE_ENTRY
        // 第二个flink是ntdll.dll
        // 这里第一次调用flink代表主程序自身,这里其实可以直接两个flink到ntdll,以此增加效率及规避特征码（Logic Morphing）(逆向遍历\随机遍历 更有迷惑性)
        let mut InMemoryOrderModuleList_flink=(*ldr).InMemoryOrderModuleList.Flink;

        // 指向本线程中ntdll的LDR_DATA_TABLE_ENTRY中Offset 0x10位置
        let mut InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY = (*ldr).InMemoryOrderModuleList.Flink as *const LDR_DATA_TABLE_ENTRY;
        
        // 传入的moudle是empty的话,返回自身的基址
        // windwos中传入null,拿回自身handle是一种常规行为,没有危险性
        // 省略了 else分支,会被默认else返回(),导致返回值和函数签名不一致
        // 使用return直接退出整个unsafe块,否则继续向下执行 
        if let MoudleType::empty=module
        {   
            
            return Ok((*peb).ImageBaseAddress);
        }

        // 保存头节点后续用于在双向链表中移动
        let  head_node= InMemoryOrderModuleList_flink;

        // 构建返回地址
        let mut addr = null_mut();


        while !(*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
        .FullDllName
        .Buffer.is_null() {
            
            if (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
            // FullDllName:UNICODE_STRING
            .FullDllName
            // FullDllName.lenght:U16 
            .Length!=0
             {
                // converts the buffer from ntf-16 to String(rust)
                let buffer=from_raw_parts((*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).FullDllName.Buffer, 
            // 从u16转为from_raw_parts签名需要的类型usize,且这里lenght代表元素个数,一个u16(utf-16)占用两个字节,所以除以2
            // 虽然可用.into()代替 as usize,但为了语义的清晰和底层开发的习惯 推荐用as usize
            ((*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).FullDllName.Length/2) as usize);

            // 当传入hash值时
            // converts the moudle as a numeric hash(u32)
            // let mut dll_file_name=
            // // Decode a native endian UTF-16–encoded slice into a String
            // String::from_utf16_lossy(buffer)
            // // 转为大写
            // .to_uppercase();    

            //     if let Ok(dll_hash) = module.to_string()
            //     // Parses this string slice into another type
            //     .parse::<u32>() {
                    
            //     }

                // 源写法对传入的moudle参数,通过to_string()转换然后进行比较,但to_string会alloc memory
                // 通过构建一个枚举 enum MoudleType来区分传入的是u32或str更加隐蔽
                if let MoudleType::hash(dll_hash) = module {
                     
                     
                     if dll_hash == hash(buffer) {
                        addr = (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).Reserved2[0];
                        break;
                }

                // 传入dll名称这种建议后续删掉,不应该传入名称,只需要使用hash就行

                if let MoudleType::name(dll_dile_name) =module  {
                    addr = (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).Reserved2[0];
                        break;
                }
                

            }

            // Moves to the next node in the list of modules
            InMemoryOrderModuleList_flink=(*InMemoryOrderModuleList_flink).Flink;
           
            // Break out of loop if all of the nodes have been checked
            if InMemoryOrderModuleList_flink==head_node{break;}

            InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY=InMemoryOrderModuleList_flink as *const LDR_DATA_TABLE_ENTRY


        }

        

        }
      
    Ok(addr)
    
    }
    

}