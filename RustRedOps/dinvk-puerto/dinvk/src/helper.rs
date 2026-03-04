//! Internal helper module 

use alloc::collections::BTreeMap;
use core::{ffi::{c_void, CStr}, slice::from_raw_parts};

use crate::types::*;

/// Maps exported function addresses to their respective names.
/// 定义一个类型别名，用于存储 "函数地址 -> 函数名" 的映射
/// 使用 BTreeMap 是为了按地址排序，方便查找
pub type Functions<'a> = BTreeMap<usize, &'a str>;

/// Portable Executable (PE) abstraction over a module's in-memory image.
/// PE 结构体：对内存中 PE 模块的抽象封装
/// 只有一个字段 base，存储模块在内存中的基地址（HMODULE）
/// 所有的解析都是基于这个基地址 + 偏移量计算出来的
#[derive(Debug)]
pub struct PE {
    /// Base address of the loaded module.
    pub base: *mut c_void,
}
/// 起点 (`base`): 也就是 HMODULE(具体实现在module.rs)，这是整个 DLL 在内存中的起始地址,这里直接就是 DOS 头 (`IMAGE_DOS_HEADER`)
/// NT 头:从 DOS 头里读取 e_lfanew 字段（这是一个偏移量）, NT Header 地址 = base + e_lfanew
/// 导出表目录 (Data Directory):NT 头里包含 OptionalHeader,OptionalHeader 里有一个数组 DataDirectory，第 0项就是导出表的“目录信息”
/// 这个目录信息里记录了导出表的 RVA
///  导出表地址 = base + DataDirectory[0].VirtualAddress
impl PE {
    /// Creates a new `PE` instance from a module base.
    /// 编译器将这个函数的代码直接复制粘贴到调用它的地方
    /// 而不是生成一个函数调用指令（call）
    /// 实现了零开销抽象.
    /// 默认情况下，编译器不会跨 crate（包）进行内联优化。加上#[inline] 允许其他使用了这个库的代码也能享受这个优化
    #[inline]
    pub fn parse(base: *mut c_void) -> Self {
        Self { base }
    }

    /// Returns the DOS header of the module.
    #[inline]
    pub fn dos_header(&self) -> *const IMAGE_DOS_HEADER {
        // 基于 Windows PE 规范：
        // 模块的基地址（base）直接指向的就是 DOS 头结构体
        // 所以我们只需要强制类型转换（cast）
        self.base as *const IMAGE_DOS_HEADER
    }

    /// Returns a pointer to the `IMAGE_NT_HEADERS`, if valid.
    #[inline]
    pub fn nt_header(&self) -> Option<*const IMAGE_NT_HEADERS> {
        unsafe {

            // 获取dos头指针
            let dos = self.base as *const IMAGE_DOS_HEADER;

            // 获取NT头
            let nt = (self.base as usize + (*dos).e_lfanew as usize) as *const IMAGE_NT_HEADERS;

            if (*nt).Signature == IMAGE_NT_SIGNATURE {
                Some(nt)
            } else {
                None
            }
        }
    }

    /// Returns all section headers in the PE.
    pub fn sections(&self) -> Option<&[IMAGE_SECTION_HEADER]> {
        unsafe {
            let nt = self.nt_header()?;
            let first_section = (nt as *const u8)
                .add(size_of::<IMAGE_NT_HEADERS>()) as *const IMAGE_SECTION_HEADER;
            Some(from_raw_parts(first_section, (*nt).FileHeader.NumberOfSections as usize))
        }
    }

    /// Finds the name of the section containing a specific RVA.
    pub fn section_name_by_rva(&self, rva: u32) -> Option<&str> {
        self.sections()?.iter().find_map(|sec| {
            let start = sec.VirtualAddress;
            let end = start + unsafe { sec.Misc.VirtualSize };
            if rva >= start && rva < end {
                let name = unsafe { core::str::from_utf8_unchecked(&sec.Name[..]) };
                Some(name.trim_end_matches('\0'))
            } else {
                None
            }
        })
    }

    /// Finds a section by its name.
    pub fn section_by_name(&self, name: &str) -> Option<&IMAGE_SECTION_HEADER> {
        self.sections()?.iter().find(|sec| {
            let raw_name = unsafe { core::str::from_utf8_unchecked(&sec.Name) };
            raw_name.trim_end_matches('\0') == name
        })
    }

    /// Exports helper
    #[inline]
    pub fn exports(&self) -> Exports<'_> {
        Exports { pe: self }
    }
}

/// Provides access to the export table of a PE image.
#[derive(Debug)]
pub struct Exports<'a> {
    /// Reference to the parsed PE image.
    pub pe: &'a PE,
}

impl<'a> Exports<'a> {
    /// Returns a pointer to the `IMAGE_EXPORT_DIRECTORY`, if present.
    /// 获取导出函数目录表指针
    pub fn directory(&self) -> Option<*const IMAGE_EXPORT_DIRECTORY> {
        unsafe {
            let nt = self.pe.nt_header()?;
            let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];

            if dir.VirtualAddress == 0 {
                return None;
            }

            Some((self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_EXPORT_DIRECTORY)
        }
    }

    /// Returns a map of exported function addresses and their names.
    pub fn functions(&self) -> Option<Functions<'a>> {
        unsafe {
            let base = self.pe.base as usize;
            let dir = self.directory()?;

            let names = from_raw_parts(
                (base + (*dir).AddressOfNames as usize) as *const u32,
                (*dir).NumberOfNames as usize,
            );

            let funcs = from_raw_parts(
                (base + (*dir).AddressOfFunctions as usize) as *const u32,
                (*dir).NumberOfFunctions as usize,
            );

            let ords = from_raw_parts(
                (base + (*dir).AddressOfNameOrdinals as usize) as *const u16,
                (*dir).NumberOfNames as usize,
            );

            let mut map = BTreeMap::new();
            for i in 0..(*dir).NumberOfNames as usize {
                let ordinal = ords[i] as usize;
                let addr = base + funcs[ordinal] as usize;
                let name_ptr = (base + names[i] as usize) as *const i8;

                let name = CStr::from_ptr(name_ptr)
                .to_str()
                .unwrap_or("");
            
                map.insert(addr, name);
            }

            Some(map)
        }
    }
}
