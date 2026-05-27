// 此工具用于生成 samoa 重构时，winapis.rs 中所需的 API 和模块的哈希值。
// 算法严格遵循 puerto 的跨编码哈希对齐理念 (fnv1a_utf16_from_u8)
// samoa/winapis.rs/fn winapis需要用到的函数hash值

// 将 puerto/src/hash.rs 的逻辑复制到这里独立运行
pub fn fnv1a_utf16_from_u8(data: &[u8]) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 0x3D91_4AB7;
    const FNV_PRIME: u32 = 0xAD37_79B9;

    let mut hash = FNV_OFFSET_BASIS;

    for &val in data {
        // Case Folding: a-z -> A-Z
        let chr = if val >= 97 && val <= 122 { val - 32 } else { val };

        // 模拟 UTF-16: 第一个字节是 ASCII，第二个字节是 0
        let bytes = [chr, 0u8];

        for &byte in &bytes {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

fn main() {
    let api_list = vec![
        "NtSignalAndWaitForSingleObject",
        "NtQueueApcThread",
        "NtAlertResumeThread",
        "NtQueryInformationProcess",
        "NtLockVirtualMemory",
        "NtDuplicateObject",
        "NtCreateEvent",
        "NtWaitForSingleObject",
        "NtClose",
        "TpAllocPool",
        "TpSetPoolStackInformation",
        "TpSetPoolMinThreads",
        "TpSetPoolMaxThreads",
        "TpAllocTimer",
        "TpSetTimer",
        "TpAllocWait",
        "TpSetWait",
        "NtSetEvent",
        "CloseThreadpool",
        "RtlWalkHeap",
        "SetProcessValidCallTargets",
        "ConvertFiberToThread",
        "ConvertThreadToFiber",
        "CreateFiber",
        "DeleteFiber",
        "SwitchToFiber",
    ];

    println!("[+] 开始计算 Samoa (winapis.rs) 依赖的 API 哈希值 (fnv1a_utf16_from_u8):");
    println!("==========================================================================");

    for api in api_list {
        let hash = fnv1a_utf16_from_u8(api.as_bytes());
        // 打印出可直接复制粘贴到 winapis.rs 中的代码格式
        println!("{}: transmute(get_proc_address(module_handle, {}u32, Some(fnv1a_utf16_from_u8))),", api, hash);
    }
    
    println!("==========================================================================\n");

    // 顺便生成模块名称的哈希 (假设 dinvk 在解析时会去掉 .dll 并转大写)
    // 根据 puerto/src/hash.rs 的注释，直接传 "ntdll" 等名字进去即可
    let modules = vec!["ntdll", "kernel32", "kernelbase", "cryptbase"];
    println!("[+] 模块名称哈希值计算:");
    println!("==========================================================================");
    for md in modules {
        let hash = fnv1a_utf16_from_u8(md.as_bytes());
        println!("let {} = get_module_address({}u32, Some(fnv1a_utf16));", md, hash);
    }
}
