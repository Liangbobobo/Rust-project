// 禁用#[derive(Debug)]
// 禁用result<>,改用option<*mut c_void>

// 指令混淆优化 (Anti-EDR)
//   EDR 经常通过扫描 NtCurrentPeb (其内部是 mov rax, gs:[60h]) 及其后的
//   Ldr 访问模式来定位“手动模块解析”行为。
//   优化建议： 可以在访问 Ldr 时加入微小的指针运算混淆，或者使用
//   core::ptr::read_volatile 防止编译器生成过于规律的汇编模式。


// 哈希函数的多样化： 不要只传一个固定的
//       hash_func。可以考虑在调用处根据不同的模块使用不同的哈希算法（例如
//       ：找 ntdll 用 fnv1a，找 kernel32 用 murmur3），这会让你的行为在
//       EDR 的启发式分析中显得极其混乱，难以定性。
//    2. 不使用 `is_null()`： 在极其严苛的免杀场景下，可以使用 (ptr as
//       usize) == 0 来代替
//       ptr.is_null()。虽然语义相同，但有时能避开某些针对 Rust
//       标准库特定函数签名的扫描。
//    3. 循环展开 (Loop Unrolling)： 如果你知道目标 DLL
//       通常在链表的前几个位置，可以尝试手动展开前两次循环。这会打破常见的
//       “遍历链表”指令序列指纹。


use core::{
    ffi::{c_void},
    ptr::null_mut,
    slice::from_raw_parts,
};

use crate::helper::PE;
use crate::types::{HMODULE, LDR_DATA_TABLE_ENTRY};
use crate::winapis::NtCurrentPeb;
use spin::Once;

type hash_type = Option<u32>;

/// crate a static variable to store the ntall.dll's address
///
///
static NTD: Once<u64> = Once::new();


/// 获取模块基址
///
///
/// 使用Option(定义在core中)不需要引入std
#[inline(always)]
pub fn retrieve_moudle_add(
    module: hash_type, // 传入对应的模块的hash值
    hash_func: Option<fn(&[u16]) -> u32>,
) -> Option<HMODULE>

{
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回None,并退出retrieve_moudle_add
    let hash = hash_func?;

    
    unsafe {
        let peb = NtCurrentPeb();
        let ldr = (*peb).Ldr;

        let mut InMemoryOrderModuleList_flink = (*ldr).InMemoryOrderModuleList.Flink;
        let mut InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY =
            (*ldr).InMemoryOrderModuleList.Flink as *const LDR_DATA_TABLE_ENTRY;

        // 处理传入的module是None的情况
        let module=match module {
            Some(h)=>h,
            None=>return Some((*peb).ImageBaseAddress)
        };
            
        

        let head_node = InMemoryOrderModuleList_flink;
        let mut addr = null_mut();

        while !(*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
            .FullDllName
            .Buffer
            .is_null()
        {
            if (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
                .FullDllName
                .Length
                != 0
            {
                let buffer = from_raw_parts(
                    (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
                        .FullDllName
                        .Buffer,
                    ((*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
                        .FullDllName
                        .Length
                        / 2) as usize,
                );

                if module== hash(buffer) {
                    addr = (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).Reserved2[0];
                    break;
                }
            }
            InMemoryOrderModuleList_flink = (*InMemoryOrderModuleList_flink).Flink;

            if InMemoryOrderModuleList_flink == head_node {
                break;
            }

            InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY =
                InMemoryOrderModuleList_flink as *const LDR_DATA_TABLE_ENTRY
        }

        if addr.is_null() { None } else { Some(addr) }
    }
}

// 与dinvk的原代码对比,重写及删除部分是否更优?
pub fn get_proc_address(
    h_moudle: Option<HMODULE>,
    function: hash_type,
    hash_func: Option<fn(&[u16]) -> u32>,
) -> Option<*mut c_void> {
    // 使用? ,当Some会解出里面的内容并向左赋值,None会直接让整个 get_proc_address 函数返回None
    let h_moudle_base = h_moudle?;

    // initializes a new pe struct
    let pe = PE::parse(h_moudle_base);
    unsafe {
        // 这里的zip逻辑会在后续实现中完善
        // let Some((nt_header,export_dir))=pe.nt_header().zip(pe.exports)
    }

    todo!()
}
