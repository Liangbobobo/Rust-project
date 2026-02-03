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
use core::{ffi::CStr, ffi::c_void, ptr::null_mut, slice::from_raw_parts};

use crate::hash::fnv1a_utf16;
use crate::helper::PE;
use crate::types::{
    API_SET_NAMESPACE_ENTRY, HMODULE, IMAGE_EXPORT_DIRECTORY, LDR_DATA_TABLE_ENTRY,
};
use crate::winapis::NtCurrentPeb;
use crate::{debug_log, types::IMAGE_DIRECTORY_ENTRY_EXPORT};
use alloc::string::String;
use alloc::vec::Vec;
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
///
/// 返回的虽然是option,但里面的内容仍然可能为空?
/// 该函数还待优化
#[inline(always)]
pub fn retrieve_module_add(
    module: hash_type, // 传入对应的模块的hash值
    hash_func: Option<fn(&[u16]) -> u32>,
) -> Option<HMODULE> {
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回None,并退出retrieve_module_add
    // 需要增加debug时的错误提示,使用debug_log!
    // let hash = hash_func?; // 源代码

    let Some(hash) = hash_func else {
        debug_log!("调用的hash函数指针不可用");
        return None;
    };

    unsafe {
        let peb = NtCurrentPeb();
        let ldr = (*peb).Ldr;

        let mut InMemoryOrderModuleList_flink = (*ldr).InMemoryOrderModuleList.Flink;
        let mut InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY =
            (*ldr).InMemoryOrderModuleList.Flink as *const LDR_DATA_TABLE_ENTRY;

        // 处理传入的module是None的情况
        let module = match module {
            Some(h) => h,
            // 需要增加debug时的错误提示,使用debug_log!
            // 需要对这种条件下的返回值(*peb).ImageBaseAddress做is_null()判定,排除极端环境(peb破坏)返回some(0x0)的情况,这时候依然会出现错误
            None => {
                if (*peb).ImageBaseAddress.is_null() {
                    debug_log!("[-] ImageBaseAddress is NULL");
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

                if module == hash(buffer) {
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
/// 通过ordinal找到的地址不需要做是否为转发地址的判断
///
/// 通过hash值找到的地址需要判断是否为转发地址,并进一步在get_forwarded_address中处理
pub fn get_proc_address(
    h_module: Option<HMODULE>,
    function: hash_type,
    hash_func: Option<fn(&[u16]) -> u32>,
) -> Option<*mut c_void> {
    // 使用let-else解构option,这样可以错误输出
    // 语义上，None 表示“没找到”，Some(null_mut()) 表示“找到了，但地址是 0”
    // 关于是option好还是*mut c_void好,需要进一步分析,option在底层还是有极少的语言特性的,*mut c_void是最隐蔽的
    let Some(h_module) = h_module else {
        debug_log!("传入的h_module不可用");
        return None;
    };

    // initializes a new pe struct
    let pe = PE::parse(h_module);

    // 将传入的h_module转为usize方便后续计算
    // 在win64下这是u64
    let h_module = h_module as usize;

    unsafe {
        // nt header->
        // 检查nt header 和export_dir这两个指向IMAGE_NT_HEADER 和 MAGE_EXPORT_DIRECTORY的指针是否可用
        let Some((nt_header, export_dir)) = pe.nt_header().zip(pe.exports().directory()) else {
            debug_log!("nt header或export_dir指针不可用");
            return None;
        };

        // retrieve export table 大小,用于判断该模块是否是export forwarding(函数转发)
        let export_size = (*nt_header).OptionalHeader.DataDirectory
        // IMAGE_DIRECTORY_ENTRY_EXPORT 是索引值,在rust中slice和数组必须是usize
        [IMAGE_DIRECTORY_ENTRY_EXPORT as usize]
            .Size as usize;

        // 下面分别获取IMAGE_EXPORT_DIRECTORY 中AddressOfNames AddressOfNameOrdinals AddressOfFunctions三个字段(字段的类型都是u32,代表各自的RVA),后期会用RVA加上基址找到实际的指针地址
        // 虽然指针是u32(4字节的),但AddressOfNameOrdinals指向的内容是u16(2字节).其余两个是u32
        // 在名称数组中i位置找到需要的函数,在序号数组中使用i获取对应的地址数组的下标idx,使用idx从地址数组中取函数的地址(RVA)

        // AddressOfNames(RVA)指向一个数组([u32]类型),数组中每个元素也是RVA
        // PE文件规范,所有RVA都是4字节(u32).
        // names数组是*const u32的,加上基址后需要转为一个指向ascii字符串的指针(*const i8)
        // names[i]指向的是以 `\0` 结尾的 ASCII字符串
        // ASCII 字符在内存中占用 1 个字节，所以在 Rust（以及 C）中，我们用 i8（即 c_char）指针来指向它(使用i8保存和c的兼容性)
        // 在计算names[i]中的字符串个数时,如果我们把它当作 u32 指针，一次就会读出 4 个字母（比如把 "NtMa" 读成一个数字），这显然是不对的
        // names这个数组(指针是*const u32,指针指向的类型是u32),但这个数组代表的是函数名的RVA,加上基址后得到真正指向函数名的指针(此时得到的指针仍然是*const u32的),由于函数名是ascii字符串,所以要转为* const i8才能指向函数名的第一个字符.
        let names = from_raw_parts(
            (h_module + (*export_dir).AddressOfNames as usize) as *const u32,
            (*export_dir).NumberOfNames as usize,
        );

        // AddressOfNameOrdinals([u16]类型)
        // ordinals[i] 是 names[i]对应的函数在 functions 数组中的索引
        //
        let ordinals = from_raw_parts(
            (h_module + (*export_dir).AddressOfNameOrdinals as usize) as *const u16,
            (*export_dir).NumberOfNames as usize,
        );

        // AddressOfFunctions([u32])
        let functions = from_raw_parts(
            (h_module + (*export_dir).AddressOfFunctions as usize) as *const u32,
            (*export_dir).NumberOfFunctions as usize,
        );

        // 如果传入的function是ordinal,返回对应的函数的地址
        if let Some(ordinals) = function
            && ordinals <= 0xFFFF
        {
            // 保留低16位
            // 任何和1的与运算,都会保留原值(任何和0的与运算都会变为0)
            // 0xFFFF的低16位1,高16位0(Mask的功能)
            let ordinals = ordinals & 0xFFFF;

            // export.base+(*export_dir).NumberOfFunctions)判断(不是(*export_dir).NumberOfNames)是否在addressoffunctions指向的数组中
            if ordinals <= (*export_dir).Base
                || ordinals >= (*export_dir).Base + (*export_dir).NumberOfFunctions
            {
                return None;
            }

            return Some(
                (h_module + functions[ordinals as usize - (*export_dir).Base as usize] as usize)
                    as *mut c_void,
            );
        }

        // 当传入的fucntion是函数名的hash值,此处去掉了dinvk中以函数名查找
        for i in 0..(*export_dir).NumberOfNames as usize {
            // 得到函数名的第一个字符
            let first_char = (h_module + names[i] as usize) as *const i8;

            let mut len = 0;
            while *first_char.add(len) != 0 {
                len += 1;
            }

            let to_u16 = from_raw_parts(
                (h_module + names[i] as usize) as *const u16,
                (len + 1) / 2 as usize,
            );

            let hash_func = hash_func.unwrap();

            let func_hash = hash_func(to_u16);

            if function.unwrap() == func_hash {
                // 返回函数的地址
                let idx = ordinals[i] as usize;

                // retrieve dll for get_forwarded_address
                // *const i8(裸指针) 代表一个指向 const i8内容的指针,const是约束指针指向的内容的,只能读取不能修改内容
                let dll = (h_module + (*export_dir).Name as usize) as *const i8;

                let func_addr = (h_module + functions[idx] as usize) as *mut c_void;

                return get_forwarded_address(dll, func_addr, export_dir, export_size, hash_func);
            }
        }
    }

    return Some(null_mut());
}

pub fn get_forwarded_address(
    module: *const i8,
    address: *mut c_void,
    export_dir: *const IMAGE_EXPORT_DIRECTORY,
    export_size: usize,
    hash: fn(&[u16]) -> u32,
) -> Option<*mut c_void> {
    // 如果不是转发函数,EAT里的RVA应指向.text中代码段的位置,是真正的机器码.如果EAT指向导出目录,自己的内存范围,则是一个转发函数
    // 此时address指向的是一个指针(该指针指向的是ascii字符串),这个字符串的格式是 moudle.function.之后再通过module查找函数地址
    if (address as usize) >= export_dir as usize
        && (address as usize) <= export_dir as usize + export_size
    {
        unsafe {
            // 源dinvk中是将*const i8转为str,再通过splite_once分割,重构中直接对指针指向的i8内容进行分割
            // 手动找*const i8中的边界容易出错,利用CStr转为bytes,可以利用core的优化及避开utf-8的校验
            let cstr = CStr::from_ptr(address as *const i8);

            // 该转换是否有副作用?
            let byte = cstr.to_bytes();

            // 导出转发（Forwarder） 在内存中的原始数据格式是固定的(如api-ms-win-core-file-l1-1-0.CreateFileW),所以必须先通过. 分割一下
            // 使用if-let else将其内部变量拿出来用
            let Some(dot_index) = byte.iter().position(|&b| b == b'.') else {
                debug_log!("[-] Invalid forwarder format: missing dot");

                return None;
            };

            let dll_name_bytes = &byte[..dot_index];

            let func_name_bytes = &byte[dot_index + 1..];

            // 去掉最右侧的 - 连字符
            if dll_name_bytes.starts_with(b"api-ms") || dll_name_bytes.starts_with(b"ext-ms") {
                // 从右开始找 - (ascii 45)位置
                let module_resolved =
                    if let Some(last_index) = dll_name_bytes.iter().rposition(|&b| b == b'-') {
                        resolve_api_set_map(module, &dll_name_bytes[..last_index])
                    } else {
                        resolve_api_set_map(module, dll_name_bytes)
                    };
            }
            // 使用resolve_api_set_map的返回值,进一步处理

        }
    }

    todo!()
}

/// peb.ApiSetMap
/// 继续使用module的*const i8格式
fn resolve_api_set_map(
    host_name: *const i8, // 宿主模块名
    contract_name: &[u8], // api set契约名
) -> *const &[u16] {
    unsafe {
        let peb = NtCurrentPeb();
        let map = (*peb).ApiSetMap;

        // 找到数组中指向第一个API_SET_NAMESPACE_ENTRY的地址
        let ns_entry =
            ((*map).EntryOffset as usize + map as usize) as *const API_SET_NAMESPACE_ENTRY;
        
        // 将ns_entry指针和Count构建为一个slice,方便迭代处理里面的每个元素
        let ns_entrys = from_raw_parts(ns_entry, (*map).Count as usize);


        // 遍历每个API_SET_NAMESPACE_ENTRY
         for entry in ns_entrys{

            // peb结构中的字段基本上都是u16的,两个字节代表一个字符
            let name_u16=from_raw_parts(
                (map as usize+entry.NameOffset as usize) as *const u16,entry.NameLength as usize / 2,
        );

        // 直接使用迭代器对u8和u16比较,避免转为string产生内存分配
        let k =contract_name.len() ;
        if name_u16.len()>=contract_name.len()&&
        // 使用了滑动窗口,windows(k)不复制数据,创建迭代器,将调用者以k长度为单位分割
        // 比如 name_u16 是 [A, B, C, D]，k 是 2，它会依次给出 [A, B], [B, C], [C, D]
        name_u16.windows(k)
        // 一旦有一个窗口满足条件，立刻停止搜索（短路特性），返回 true
        .any(|window|{
            window.iter()
            .zip(contract_name.iter())
            .all(|(&b16,&b8)|b16==b8 as u16)
        })
        {
            // 如果找到了匹配的entry,解析value(物理地址)
            // 这里为啥不用除以2了?
            let values =from_raw_parts((map as usize+entry.ValueOffset as usize)as *const u16, 
        entry.ValueCount as usize) ;
            if values.is_empty(){return null_mut();}

            // 如果有多个映射值,需要根据host_name过滤(如某些api再不同宿主下会重定向到不同的dll)
            // 对多个宿主
        }

        
        

         }
    }

    todo!()
}







// u[8] u[16]等不同编码方式之间的转换需要自己实现方便使用