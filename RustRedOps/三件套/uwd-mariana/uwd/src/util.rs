// 本文件在系统dll字节流中,切割出需要的Gadgets

use core::{ffi::c_void, slice::from_raw_parts};
use alloc::vec::Vec;// 能否自定义alloctor(像puerto中的那样)

use obfstr::obfbytes as b;
use dinvk::types::IMAGE_RUNTIME_FUNCTION;

use crate::ignoring_set_fpreg;

/// Searches for a valid instruction offset in a function.
///
/// This scans the function's code region for a `call qword ptr [rip+0]`
/// instruction sequence and returns the offset *after* the instruction.
///
/// # Notes
///
/// The searched gadget pattern is `48 FF 15 00 00 00 00`, and the
/// returned value is `match_offset + 7`.
pub fn find_valid_instruction_offset(
    module: *mut c_void,
    runtime: &IMAGE_RUNTIME_FUNCTION,
) -> Option<u32> {
    let start = module as u64 + runtime.BeginAddress as u64;
    let end = module as u64 + runtime.EndAddress as u64;
    let size = end - start;

    // Find a gadget `call qword ptr [rip+0]`
    let pattern = b!(&[0x48, 0xFF, 0x15]);
    unsafe {
        let bytes = from_raw_parts(start as *const u8, size as usize);
        if let Some(pos) = memchr::memmem::find(bytes, pattern) {
            // Returns valid RVA: offset of the gadget inside the function
            return Some((pos + 7) as u32);
        }
    }

    None
}

/// Scans the code of a module for a given byte pattern, restricted to valid
/// RUNTIME_FUNCTION regions.
/// 
/// 遍历.pdata节中每个合法函数区间,确保找到的gadget都位于有unwind记录的函数内部,避免非函数指令特征
pub fn find_gadget(
    module: *mut c_void, 
    pattern: &[u8], 
    runtime_table: &[IMAGE_RUNTIME_FUNCTION]// 指定在.pdata节中查找
)
// 返回找到的gadget绝对内存地址(VA),该gadget所在函数的物理栈深度 
-> Option<(*mut u8, u32)> {
    unsafe {
        let mut gadgets = runtime_table
            .iter()
            .filter_map(|runtime| {
                let start = module as u64 + runtime.BeginAddress as u64;
                let end = module as u64 + runtime.EndAddress as u64;
                // 饱和算术的方式比较end和start的大小,静默处理错误情况
                let size = end.saturating_sub(start);

                // Read bytes from the function's code region
                let bytes = from_raw_parts(start as *const u8, size as usize);
                let pos = memchr::memmem::find(bytes, pattern)?;

                let addr = (start as *mut u8).wrapping_add(pos);

                // 计算需要伪造的栈帧大小
                let frame_size = ignoring_set_fpreg(module, runtime)?;
                if frame_size == 0 {
                    return None;
                }

                Some((addr, frame_size))
            })
            .collect::<Vec<(*mut u8, u32)>>();

        if gadgets.is_empty() {
            return None;
        }

        // Shuffle to reduce pattern predictability.
        shuffle(&mut gadgets);

        gadgets.first().copied()
    }
}

/// Scans the current thread's stack to locate the return address that falls within
/// the range of the `BaseThreadInitThunk` function from `kernel32.dll`.
#[cfg(feature = "desync")]
pub fn find_base_thread_return_address() -> Option<usize> {
    use dinvk::module::{get_module_address, get_proc_address};
    use dinvk::{hash::{jenkins3, murmur3}, helper::PE};
    use crate::types::Unwind;

    unsafe {
        // Get handle for kernel32.dll
        let kernel32 = get_module_address(2808682670u32, Some(murmur3));
        if kernel32.is_null() {
            return None;
        }

        // Resolves the address of the BaseThreadInitThunk function
        let base_thread = get_proc_address(kernel32, 4073232152u32, Some(jenkins3));
        if base_thread.is_null() {
            return None;
        }

        // Calculate the size of the BaseThreadInitThunk function
        let pe_kernel32 = Unwind::new(PE::parse(kernel32));
        let size = pe_kernel32.function_size(base_thread)? as usize;

        // Access the TEB and stack limits
        let teb = dinvk::winapis::NtCurrentTeb();
        let stack_base = (*teb).Reserved1[1] as usize;
        let stack_limit = (*teb).Reserved1[2] as usize;

        // Stack scanning begins
        let base_addr = base_thread as usize;
        let mut rsp = stack_base - 8;
        while rsp >= stack_limit {
            let val = (rsp as *const usize).read();

            // Checks if the return is in the BaseThreadInitThunk range
            if val >= base_addr && val < base_addr + size {
                return Some(rsp);
            }

            rsp -= 8;
        }

        None
    }
}

/// Randomly shuffles the elements of a list in place.
///
// &mut [T]代表一个可变slice,执行原地修改时,不需要额外分配内存
pub fn shuffle<T>(list: &mut [T]) {

    let mut seed = unsafe { core::arch::x86_64::_rdtsc() };

    // 下面使用了Fisher-Yates洗牌算法
    // (1..list.len()) 左闭右开区间 .rev()将区间内元素位置反转
    for i in (1..list.len()).rev() {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = seed as usize % (i + 1);
        list.swap(i, j);
    }
}
