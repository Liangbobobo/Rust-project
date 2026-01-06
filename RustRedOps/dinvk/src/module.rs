//! Module resolution helpers for Windows.
//该文件是项目的基石,实现了在不调用任何windows api的情况下,手动解析内存中PE结构来获取模块和函数地址.
// 这种技术一般被称为"Reflective DLL Injection反射式dll注入" 或 "Shellcode coding"风格的变成,主要目的是规避EDR/AV的api hook监控,或者为了在no_std环境下运行


//引入alloc库,这是一个no_std库,提供了堆分配类型
use alloc::{
    format, vec::Vec, vec,
    string::{String, ToString}, 
};
use core::{
    ffi::{c_void, CStr}, 
    ptr::null_mut, 
    slice::from_raw_parts
};

use obfstr::obfstr as s;
use crate::{types::*, helper::PE};
use crate::hash::{crc32ba, murmur3};
use crate::winapis::{LoadLibraryA, NtCurrentPeb};

/// Stores the NTDLL address
static NTDLL: spin::Once<u64> = spin::Once::new();

/// Retrieves the base address of the `ntdll.dll` module.
#[inline(always)]
pub fn get_ntdll_address() -> *mut c_void {
    // lazy one-time initialization and thread safe and 只初始化一次 ntdll.dll地址
    *NTDLL.call_once(|| get_module_address(
        2788516083u32, 
        Some(murmur3)) as u64
    ) as *mut c_void
}

/// Resolves the base address of a module loaded in memory by name or hash.
///
/// # Examples
///
/// ```
/// // Retrieving module address via string and hash
/// let base = get_module_address("ntdll.dll", None);
/// let base = get_module_address(2788516083u32, Some(murmur3));
/// ```
pub fn get_module_address<T>(
    module: T,
    hash: Option<fn(&str) -> u32>
) -> HMODULE
where 
    T: ToString
{
    unsafe {
        let hash = hash.unwrap_or(crc32ba);
        //获取PEB指针,通过 GS:[0x60] on x64
        let peb = NtCurrentPeb();
        //获取 PEB_LDR_DATA 结构指针
        let ldr_data = (*peb).Ldr;

        //(*ldr_data).InMemoryOrderModuleList,获取PEB_LDR_DATA中InMemoryOrderModuleList(是一个双向链表,代表模块在内存中的布局及排列的链表)的第一个节点
        //.Flink,双向链表的下一个节点,即主程序本身(.exe)的LDR_DATA_TABLE_ENTRY的中间位置(通常是0x10的偏移处).
        let mut list_node = (*ldr_data).InMemoryOrderModuleList.Flink;
        
        //rust中* 这个符号被重载(overloaded)了,当*在变量前表示解引用,取值操作;当出现*const *mut的时候,代表这是裸指针类型
        //这里的as只改变了指针的类型标签,不改变地址数值,也不会丢失数据,更没有读取内存中的数据
        //直接使用指向LDR_DATA_TABLE_ENTRY中InMemoryOrderLinks这个链表的指针,作为LDR_DATA_TABLE_ENTRY的0x00处指针,方便找到DllBase的地址
        //这里data_table_entry指向LDR_DATA_TABLE_ENTRY中InMemoryOrderLinks的位置
        //由于as *const LDR_DATA_TABLE_ENTRY;这里将data_table_entry转为指向LDR_DATA_TABLE_ENTRY结构体中InMemoryOrderLinks所在的偏移位置(0x10),但是在编译器看来仍然是指向这个结构体的第0个字节处
        let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as *const LDR_DATA_TABLE_ENTRY;

        //提供的模块参数(moudule)为空,这里将停止执行并返回当前主程序(.exe)的基址
        //此处,模拟了 Windows 官方 API `GetModuleHandle`(用于获取模块的句柄（即内存基址）) 的行为,GetModuleHandle官方文档规定,如果传入的参数是 NULL（在 Rust 中对应空字符串或None），该函数返回用于创建调用进程的文件（即 .exe 文件）的句柄
        if module.to_string().is_empty() {
            return (*peb).ImageBaseAddress;
        }

        // Save a reference to the head nod for the list
        let head_node = list_node;
        let mut addr = null_mut();
        
        //(*data_table_entry).FullDllName实际上是访问 LDR_DATA_TABLE_ENTRY中BaseDllName这个字段(因为此时Base是0x10,FULLDLLNAME是0x48,这个操作直接指向BaseDllName的位置0x58)
        while !(*data_table_entry).FullDllName.Buffer.is_null() {
            if (*data_table_entry).FullDllName.Length != 0 {
                // Converts the buffer from UTF-16 to a `String`
                let buffer = from_raw_parts(
                    (*data_table_entry).FullDllName.Buffer,
                    ((*data_table_entry).FullDllName.Length / 2) as usize
                );
            
                // Try interpreting `module` as a numeric hash (u32)
                let mut dll_file_name = String::from_utf16_lossy(buffer).to_uppercase();
                if let Ok(dll_hash) = module.to_string().parse::<u32>() {
                    if dll_hash == hash(&dll_file_name) {
                        addr = (*data_table_entry).Reserved2[0];
                        break;
                    }
                } else {
                    // If it is not an `u32`, it is treated as a string
                    let module = canonicalize_module(&module.to_string());
                    dll_file_name = canonicalize_module(&dll_file_name);
                    if dll_file_name == module {
                        addr = (*data_table_entry).Reserved2[0];
                        break;
                    }
                }
            }

            // Moves to the next node in the list of modules
            list_node = (*list_node).Flink;

            // Break out of loop if all of the nodes have been checked
            if list_node == head_node {
                break
            }

            data_table_entry = list_node as *const LDR_DATA_TABLE_ENTRY
        }
        
        addr
    }
}

/// Retrieves the address of an exported function from a loaded module.
///
/// # Examples
/// 
/// ```
/// // Retrieving exported API address via string, ordinal and hash
/// let addr = get_proc_address(kernel32, "LoadLibraryA", None);
/// let addr = get_proc_address(kernel32, 3962820501u32, Some(jenkins));
/// let addr = get_proc_address(kernel32, 997, None);
/// ```
/// 
/// get_proc_address这个函数名中的proc,是 "Procedure"（过程）的缩写,早期以Procedure代指不返回值的函数
pub fn get_proc_address<T>(
    h_module: HMODULE, // 目标模块的句柄（即内存中的基地址），通常由 LoadLibrary 返回或遍历 PEB 获取
    function: T, //要查找的函数标识：可以是函数名字符串，也可以是序号，或者哈希值（以字符串形式传入）
    hash: Option<fn(&str) -> u32>
) -> *mut c_void // 返回找到的函数地址（裸指针），如果未找到则返回空指针
where 
    T: ToString,
{
    if h_module.is_null() {
        return null_mut();
    }

    unsafe {
        // Converts the module handle to a base address
        // 将模块句柄（指针）转换为 usize 类型。
        // 在 PE 结构解析中，所有的 RVA (Relative Virtual Address) 都是相对于这个基地址的偏移
        // 所以我们需要这个数值来计算内存中的绝对地址 (VA = Base + RVA)。
        //这里是对指针的操作,h_module指针指向的就是pe文件的dos头(IMAGE_DOS_HEADER)
        //但根据指针自身的偏移就能找到其他字段
        let h_module = h_module as usize;

        // Initializes the PE parser from the base address
        // 使用项目定义的 PE 助手结构体解析模块头。
        // 这通常涉及读取 DOS 头以找到 NT 头的位置
        // 通过IMAGE_DOS_HEADER(结构体)->e_lfanew(字段)->IMAGE_NT_HEADERS
        let pe = PE::parse(h_module as *mut c_void);

        // Retrieves the NT header and export directory
        // pe.nt_header() 返回 NT 头指针，pe.exports().directory() 返回导出表目录指针。
        //使用let some进行解构
        let Some((nt_header, export_dir)) = pe.nt_header().zip(pe.exports().directory()) else {
            return null_mut();
        };

        // Retrieves the size of the export table
        //通过export table大小,来判断该模块是否 函数转发”(Export Forwarding)
        //Windows 规定：如果导出地址表中的某个地址 RVA 位于导出目录（Export Directory）的内存范围内，
        // 那么该地址不是代码入口，而是一个指向 "DllName.FunctionName" 字符串的 RVA，即转发。
        //这里的DataDirectory类型是IMAGE_DATA_DIRECTORY,是包含16个元素的数组
        //这16个数组结构都是IMAGE_DATA_DIRECTORY
        //IMAGE_DIRECTORY_ENTRY_EXPORT定义为 0u16,代表第一个数组,指向存储导出表的结构
        let export_size = (*nt_header).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize].Size as usize;

        // Retrieving information from module names
        //IMAGE_EXPORT_DIRECTORY->AddressOfNames,指向一个存储该模块函数名的数组,该数组不在本程序的二进制文件中,而在目标dll的内存映射区中
        //AddressOfNames as usize,这里当作RVA来使用
        //names是一个slice,长度是导出表的函数个数
        //h_module + (*export_dir).AddressOfNames as usize,这里组成了slice中u32数组的绝对地址
        //但,slice中u32指向的仍然是RVA
        //构建导出函数名字符串的索引表slice
        let names = from_raw_parts(
            (h_module + (*export_dir).AddressOfNames as usize) as *const u32, 
            (*export_dir).NumberOfNames as usize
        );

        // Retrieving information from functions
        // 构建导出函数地址表 (AddressOfFunctions, EAT) 的切片。
        // 这个数组存储的是 RVA (u32)，指向函数的实际代码入口。
        // NumberOfFunctions 是所有导出函数（包括仅通过序号导出的）的总数。
        let functions = from_raw_parts(
            (h_module + (*export_dir).AddressOfFunctions as usize) as *const u32, 
            (*export_dir).NumberOfFunctions as usize
        );

        // Retrieving information from ordinals
        // 构建导出序号表 (AddressOfNameOrdinals) 的切片。
        // 这是一个 u16 数组，它充当了 "名称表" 到 "函数地址表" 的索引映射。
        // 它的长度与 names 数组一致。即：ordinals[i] 是 names[i]对应的函数在 functions 数组中的索引。
        //导出表会按照序号\名称\地址分别索引,名称表以a-z排序,地址表以0,1顺延排序,序号表以名称的顺序进行排序
        //顺序表和地址表不能根据其索引做到一一对应
        //AddressOfNameOrdinals 和 AddressOfNames 的性质完全一样，它们指向的数据都存储在 PE 文件的导出表数据区域中（通常是.edata 或 .rdata 节），并且在 PE 文件被加载到内存后，都位于目标模块（DLL）的内存映射区内
        let ordinals = from_raw_parts(
            (h_module + (*export_dir).AddressOfNameOrdinals as usize) as *const u16, 
            (*export_dir).NumberOfNames as usize
        );

        // Convert Api name to String
        // 将输入的 function 参数统一转换为 String，以便后续尝试解析为数字或作为字符串比较。
        let api_name = function.to_string();

        // Import By Ordinal
        //--- 场景 1: 通过序号 (Ordinal) 导入 ---
        // 尝试将 api_name 解析为 u32 数字。如果成功，并且该数字 <= 0xFFFF (序号是 u16)，则认为是序号查找。
        if let Ok(ordinal) = api_name.parse::<u32>() && ordinal <= 0xFFFF {

            //传入的参数会被转为u32,但ordinal是16位的,这里通过与运算,可以把高位切掉,保证传入的参数不会超过u16的最大值65535.否则可能会出现数组越界或逻辑错误
            // 确保序号只取低 16 位。
            let ordinal = ordinal & 0xFFFF;

            //检查序号的有效性：
            // (*export_dir).Base 是导出函数的起始序号基数（通常为1
            // 真实的数组索引 = 传入的序号 - Base
            // 如果计算出的索引小于 0 或超出了 functions数组的范围，则该序号无效
            if ordinal < (*export_dir).Base || (ordinal >= (*export_dir).Base + (*export_dir).NumberOfFunctions) {
                return null_mut();
            }


            //计算并返回函数地址：
            // 1. ordinal - Base 得到索引。
            // 2. functions[...] 取出该索引处的 RVA。
            // 3. h_module + RVA 得到绝对地址。
            //注意：此处未对序号导出的函数做“转发检查”，通常序号导出很少涉及转发， 严格来说也应检查
            return (h_module + functions[ordinal as usize - (*export_dir).Base as usize] as usize) as *mut c_void;
        }

        // Extract DLL name from export directory for forwarder resolution
        // 这在处理转发时需要用到（如果是转发，我们需要知道当前在哪个DLL 里）
        //  (*export_dir).Name 是指向 DLL 名称字符串的 RVA
        let dll_name = {
            let ptr = (h_module + (*export_dir).Name as usize) as *const i8;
            //详见module.md的源码
            CStr::from_ptr(ptr)
            .to_string_lossy()
            .into_owned()
        };

        // Import By Name or Hash
        let hash = hash.unwrap_or(crc32ba);

        // 遍历导出名称表 (names)
        for i in 0..(*export_dir).NumberOfNames as usize {

            // 解析当前索引 i 处的函数名。
            // names[i] 是 RVA -> +Base -> 指针 -> CStr -> &str
            let name = CStr::from_ptr((h_module + names[i] as usize) as *const i8)
                //不使用to_string_lossy(),避免在循环中分配内存,详见module.md
                .to_str()
                .unwrap_or("");

            // 通过序号表查找对应的函数地址表索引。
            //Index i -> Names[i] (函数名) -> Ordinals[i](函数表索引) -> Functions[Index] (函数地址 RVA)
            let ordinal = ordinals[i] as usize;

            // 计算当前函数的绝对地址 (VA)
            let address = (h_module + functions[ordinal] as usize) as *mut c_void;

            // 检查用户输入是否是数字（哈希值）
            if let Ok(api_hash) = api_name.parse::<u32>() {
                // Comparison by hash
                if hash(name) == api_hash {

                    // 匹配成功！调用 get_forwarded_address处理转发逻辑。
                    // 如果该地址指向代码，get_forwarded_address会直接返回该地址。
                   // 如果该地址指向转发字符串，它会递归加载目标 DLL并查找目标函数。
                    return get_forwarded_address(&dll_name, address, export_dir, export_size, hash);
                }
            } else {
                // Comparison by String
                if name == api_name {
                     // 匹配成功！同样处理转发逻辑并返回。
                    return get_forwarded_address(&dll_name, address, export_dir, export_size, hash);
                }
            }
        }
    }

    null_mut()
}

/// Resolves forwarded exports to the actual implementation address.
/// 处理Windows PE 导出表中比较棘手的“函数转发 (Forwarding)”机制，尤其涉及到了现代 Windows的 API Set Schema (虚拟 DLL) 解析
/// 解析转发导出（Forwarded Exports）以获取实际的实现地址。
/// 在 Windows PE 格式中，如果导出地址表 (EAT) 中的某个地址 RVA 指向了
/// 导出目录（Export Directory）自身的内存范围内，那么该地址不是代码入口
/// 而是一个指向 ASCII 字符串的指针，该字符串指明了真正的函数位置（如"NTDLL.RtlAllocateHeap"）
fn get_forwarded_address(
    module: &str,//当前模块,如"KERNEL32.dll"），用于 API Set 解析时的宿主判断
    address: *mut c_void,//在导出表中找到的原始地址（可能是代码地址，也可能是指向转发字符串的指针）
    export_dir: *const IMAGE_EXPORT_DIRECTORY,//导出目录表的指针，用于判断地址范围
    export_size: usize,//导出目录表的大小
    hash: fn(&str) -> u32,
) -> *mut c_void {
    // Detect if the address is a forwarder RVA
    //当前函数地址落在导出表地址范围内,那么就不是函数代码,而是一个指向字符串的指针
    if (address as usize) >= export_dir as usize &&
       (address as usize) < (export_dir as usize + export_size)
    {   
        //在导出表范围内,address指向的是一个 ASCII 字符串（C 风格，以 nul 结尾）
        let cstr = unsafe { CStr::from_ptr(address as *const i8) };
        let forwarder = cstr.to_str().unwrap_or_default();

        // 转发字符串的标准格式是 "MODULE.FUNCTION" (例如"NTDLL.RtlAllocateHeap")
        //split_once('.') 将其分割为模块名 (module_name) 和函数名(function_name)
        let (module_name, function_name) = forwarder.split_once('.')
        .unwrap_or(("", ""));

        // If forwarder is of type api-ms-* or ext-ms-*
        //2. 处理 API Set (虚拟 DLL) 机制
        // Windows 为了解耦，引入了 "api-ms-*" 或 "ext-ms-*" 开头的虚拟 DLL 名
        // 这些虚拟 DLL 并不直接存在于磁盘上，而是通过 PEB 中的 ApiSetMap映射到真实的物理 DLL（如 kernelbase.dll）
        let module_resolved = if module_name.starts_with(s!("api-ms")) || module_name.starts_with(s!("ext-ms")) {

            // 尝试标准化虚拟模块名
            // 某些情况下，转发名可能带有版本号或其他后缀
            // 这里 rsplit_once('-') 尝试去除最后一个连字符后的部分作为 "BaseContract"（基础契约名）
            // 例如：从 "api-ms-win-core-processthreads-l1-1-0" 提取"api-ms-win-core-processthreads-l1-1"
            let base_contract = module_name.rsplit_once('-')
            //此时根据上个函数的返回值,确定了传入map的参数是(b,_),将rsplit_once的返回值的第一个wrap到option
            .map(|(b, _)| b)
            .unwrap_or(module_name);

            //调用该函数查询PEB,解析虚拟dll对应的真实物理dll列表
            resolve_api_set_map(module, base_contract)
        } else {

            // 如果不是 API Set 虚拟 DLL，说明是指向普通的物理 DLL.直接构造文件名（通常添加 ".dll" 后缀）
            Some(vec![format!("{}.dll", module_name)])
        };

        // Try resolving the symbol from all resolved modules
        //3. 尝试从所有解析出的候选模块中查找符号
        if let Some(modules) = module_resolved {
            for module in modules {

                // 尝试获取目标模块在内存中的基地址
                let mut addr = get_module_address(module.as_str(), None);

                // 如果目标模块尚未加载到当前进程内存中，则手动调用 LoadLibrary加载它
                // 这是必须的，因为转发可能指向一个尚未被使用的 DLL?这里如果直接调用会被hook发现吗? :会被发现,但也有解决方案,需要自己补充
                if addr.is_null() {
                    addr = LoadLibraryA(module.as_str());
                }

                if !addr.is_null() {
                    let resolved = get_proc_address(addr, hash(function_name), Some(hash));
                    if !resolved.is_null() {
                        return resolved;
                    }
                }
            }
        }
    }

    address
}

/// Resolves ApiSet contracts to the actual implementing DLLs.
///
/// This parses the ApiSetMap from the PEB and returns all possible DLLs,
/// excluding the current module itself if `ValueCount > 1`.
/// Windows 7 之前，系统 DLL (如 kernel32.dll)是巨大的单体。
/// 为了重构和解耦，微软引入了 "MinWin" 和 "API Sets"。
/// 当你看到 api-ms-win-core-file-l1-1-0.dll这种奇怪的名字时，它并不是硬盘上的文件，
/// 而是一个“契约 (Contract)”或“虚拟DLL”。
/// 系统加载器会查找 PEB (进程环境块) 中的ApiSetMap，将这个虚拟名字映射到真正的物理文件（通常是 kernelbase.dll 或kernel32.dll）。
/// 这段代码就是手动实现了这个映射查询过程,将 API Set 契约（虚拟 DLL 名）解析为实际实现的物理 DLL 名
/// 该函数解析 PEB 中的 ApiSetMap 结构，返回所有可能的物理 DLL 列表
/// 如果存在多个映射值（ValueCount > 1），会排除当前模块自身以避免循环引用
fn resolve_api_set_map(
    host_name: &str,// 当前正在处理的“宿主”模块名（即谁在引用这个 APISet，用于过滤）
    contract_name: &str// 要解析的 API Set 契约名（如"api-ms-win-core-processthreads-l1-1"）
) -> Option<Vec<String>> {
    unsafe {
        let peb = NtCurrentPeb();

        // 获取 ApiSetMap 的指针,ApiSetMap 是一个未公开的结构 (API_SET_NAMESPACE)，存在于进程内存中,专门用于管理虚拟 DLL 到物理 DLL 的重定向
        let map = (*peb).ApiSetMap;
        
        // Base pointer for the namespace entry array
        // 计算命名空间条目数组的起始地址。
        // (*map).EntryOffset 是相对于 map 基地址的字节偏移量。
        // ns_entry 指向 API_SET_NAMESPACE_ENTRY 结构体数组的第一个元素
        let ns_entry = ((*map).EntryOffset as usize + map as usize) as *const API_SET_NAMESPACE_ENTRY;

        // 构建命名空间条目的切片，Count 是条目的总数量
        let ns_entries = from_raw_parts(ns_entry, (*map).Count as usize);
        
        // 遍历所有 API Set 条目，寻找匹配的 contract_name
        for entry in ns_entries {

            // 获取当前条目的名称字符串（虚拟 DLL 名）
            // NameOffset 是相对于 map 基址的偏移
            // NameLength 是字节长度，UTF-16 字符串需要除以 2 得到字符数 (u16)
            let name = String::from_utf16_lossy(from_raw_parts(
                (map as usize + entry.NameOffset as usize) as *const u16,
                entry.NameLength as usize / 2,
            ));


            // 检查当前条目的名称是否以我们要找的 contract_name 开头。
           // 这里使用 starts_with 而不是完全相等，是因为 API Set名字有时包含不同的版本后缀或扩展。
            if name.starts_with(contract_name) {
                let values = from_raw_parts(

                    // 如果匹配，获取该条目对应的值Value）数组，即物理dll映射信息。
                    // ValueOffset 指向 API_SET_VALUE_ENTRY 数组
                    (map as usize + entry.ValueOffset as usize) as *const API_SET_VALUE_ENTRY, 
                    entry.ValueCount as usize
                );

                // Only one value: direct forward
                // 情况 1: 只有一个映射值 (ValueCount == 1)。
                // 这是最常见的情况，直接转发到目标 DLL。
                if values.len() == 1 {
                    let val = &values[0];
                    // 解析目标物理 DLL 的名称
                    let dll = String::from_utf16_lossy(from_raw_parts(
                        (map as usize + val.ValueOffset as usize) as *const u16,
                        val.ValueLength as usize / 2,
                    ));

                    return Some(vec![dll]);
                }
                
                // Multiple values: skip the host DLL to avoid self-resolving
                //情况 2: 有多个映射值 (ValueCount > 1)
                // 这通常发生在同一个 API Set 在不同情况下由不同 DLL 提供支持时
                // (例如：某些 API 可能根据宿主是 kernel32还是其他模块，指向不同的实现)
                let mut result = Vec::new();
                for val in values {

                    // 获取 "Name" 字段，这在 API_SET_VALUE_ENTRY 中通常代表"Importing Name"（导入者名称）。
                    // 如果这个字段不为空，表示该映射仅在特定模块导入此 API Set时生效。
                    let name = String::from_utf16_lossy(from_raw_parts(
                        (map as usize + val.ValueOffset as usize) as *const u16,
                        val.ValueLength as usize / 2,
                    ));


                    // 过滤逻辑：
                    // 如果映射规则指定的导入者 (name) 与当前宿主 (host_name)不匹配（忽略大小写），
                   // 我们才将其视为有效的目标。
                   //这里的逻辑稍微有点绕：它实际上是在排除“自己指向自己”的情况，或者根据微软的 Sche规则选择备选。
                    if !name.eq_ignore_ascii_case(host_name) {
                        // 解析实际的物理 DLL 名称 (Value)
                        let dll = String::from_utf16_lossy(from_raw_parts(
                            (map as usize + val.ValueOffset as usize) as *const u16,
                            val.ValueLength as usize / 2,
                        ));
   
                        result.push(dll);
                    }
                }
                
                if !result.is_empty() {
                    return Some(result);
                }
            }
        }
    }

    None
}

pub fn canonicalize_module(name: &str) -> String {
    let file = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let upper = file.to_ascii_uppercase();
    upper.trim_end_matches(".DLL").to_string()
}

#[cfg(test)]
mod tests {
    use core::ptr::null_mut;
    use super::*;

    #[test]
    fn test_modules() {
        assert_ne!(get_module_address("kernel32.dll", None), null_mut());
        assert_ne!(get_module_address("kernel32.DLL", None), null_mut());
        assert_ne!(get_module_address("kernel32", None), null_mut());
        assert_ne!(get_module_address("KERNEL32.dll", None), null_mut());
        assert_ne!(get_module_address("KERNEL32", None), null_mut());
    }

    #[test]
    fn test_function() {
        let module = get_module_address("KERNEL32.dll", None);
        assert_ne!(module, null_mut());

        let addr = get_proc_address(module, "VirtualAlloc", None);
        assert_ne!(addr, null_mut());
    }

    #[test]
    fn test_forwarded() {
        let kernel32 = get_module_address("KERNEL32.dll", None);
        assert_ne!(kernel32, null_mut());

        // KERNEL32 forwarded exports
        assert_ne!(
            get_proc_address(kernel32, "SetIoRingCompletionEvent", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(kernel32, "SetProtectedPolicy", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(kernel32, "SetProcessDefaultCpuSetMasks", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(kernel32, "SetDefaultDllDirectories", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(kernel32, "SetProcessDefaultCpuSets", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(kernel32, "InitializeProcThreadAttributeList", None),
            null_mut()
        );

        // ADVAPI32 forwarded exports
        let advapi32 = LoadLibraryA("advapi32.dll");
        assert_ne!(advapi32, null_mut());

        assert_ne!(
            get_proc_address(advapi32, "SystemFunction028", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(advapi32, "PerfIncrementULongCounterValue", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(advapi32, "PerfSetCounterRefValue", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(advapi32, "I_QueryTagInformation", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(advapi32, "TraceQueryInformation", None),
            null_mut()
        );

        assert_ne!(
            get_proc_address(advapi32, "TraceMessage", None),
            null_mut()
        );
    }
}
