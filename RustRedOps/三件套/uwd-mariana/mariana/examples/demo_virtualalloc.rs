use core::ffi::c_void;
use core::ptr;
use mariana::hash_name::{HASH_KERNEL32_DLL, HASH_VIRTUAL_ALLOC};
use mariana::spoof;
use puerto::hash::fnv1a_utf16;
use puerto::module::{get_module_address, get_proc_address};

fn main() {
    println!("============================================================");
    println!("[*] Starting Mariana Call Stack Spoofing Demo...");
    println!("============================================================");

    // ------------------------------------------------------------------------
    // 步骤 1：利用 puerto 动态寻址定位 kernel32.dll 和 VirtualAlloc
    // ------------------------------------------------------------------------
    let kernel32 = get_module_address(Some(HASH_KERNEL32_DLL), Some(fnv1a_utf16))
        .expect("[-] Failed to find kernel32.dll");

    let virtual_alloc =
        get_proc_address(Some(kernel32), Some(HASH_VIRTUAL_ALLOC), Some(fnv1a_utf16))
            .expect("[-] Failed to find VirtualAlloc");

    println!(
        "[+] Target API (VirtualAlloc) located at : {:p}",
        virtual_alloc
    );

    // ------------------------------------------------------------------------
    // 步骤 2：准备 VirtualAlloc 的 4 个入参
    // ------------------------------------------------------------------------
    let lp_address = ptr::null_mut::<c_void>(); // 操作系统自动选择基址 (NULL)
    let dw_size: usize = 0x1000;                // 申请 4096 字节 (1 个物理内存页)
    let fl_allocation_type: u32 = 0x3000;       // MEM_COMMIT (0x1000) | MEM_RESERVE (0x2000)
    let fl_protect: u32 = 0x04;                 // PAGE_READWRITE (可读可写)

    println!("[*] Executing spoof! macro with synthetic/desync call stack...");

    // ------------------------------------------------------------------------
    // 步骤 3：使用 spoof! 宏在伪造/脱敏的调用栈上发起调用
    // ------------------------------------------------------------------------
    let allocated_ptr = spoof!(
        virtual_alloc,
        lp_address,
        dw_size,
        fl_allocation_type,
        fl_protect
    )
    .expect("[-] spoof! call failed with MarianaError");

    // ------------------------------------------------------------------------
    // 步骤 4：校验结果并暂停供调试器（WinDbg/x64dbg）观测
    // ------------------------------------------------------------------------
    assert!(!allocated_ptr.is_null(), "[-] VirtualAlloc returned null pointer");
    println!("[+] Successfully allocated memory at    : {:p}", allocated_ptr);
    println!("------------------------------------------------------------");
    println!("[*] Execution finished successfully!");
    println!("[*] Press [Enter] to exit (keep process alive for debugger)...");

    // 暂停进程，方便你在这期间把 WinDbg / x64dbg 附加（Attach）上去查看内存
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}
