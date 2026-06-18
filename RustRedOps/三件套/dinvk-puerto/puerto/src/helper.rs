use core::{ffi::c_void, slice::from_raw_parts};
use crate::types::{
    IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY, IMAGE_NT_HEADERS,
    IMAGE_NT_SIGNATURE, IMAGE_SECTION_HEADER,
};
// 将PE结构体抽象出来,是整个项目更加清晰
/// 
/// 这是一种设计模式,“类型状态模式” (Type State / Newtype Pattern) 或者简单的 “封装抽象”
/// 
// 这样做1.含义清晰(代表一个pe文件格式的内存)2.可以关联方法 3.零成本抽象(该结构体编译后,在内存中布局和*mut c_void是一样的,无额外内存开销及性能损耗) 4. 后续方便扩展
// #[derive(Debug)] // realse版本中不应该用,会增加特征字符串/增加二进制体积,且无实际用途
#[derive(Debug)]
pub struct PE {
    /// Base address of the loaded module.
    pub base: *mut c_void,
}

impl PE {

        #[inline]
        pub fn exports(&self)->Exports<'_> {
            Exports { pe: self }
        }

    /// Creates a new `PE` instance from a module base.
    /// 
    /// 一个module就是一个被win加载器映射到内存中,对齐并解析好导入表后的pe image
    #[inline]
    pub fn parse(base:*mut c_void) ->Self{
        Self { base }
    }

    /// retrieve dos header of the moudle
    #[inline]
    pub fn dos_header(&self)->*const IMAGE_DOS_HEADER {
        self.base as *const IMAGE_DOS_HEADER
    }



    #[inline]
    pub fn nt_header(&self)->Option<*const IMAGE_NT_HEADERS> {
        let dos=self.base as *const IMAGE_DOS_HEADER;

        
        unsafe {

            // dos header中e_lfanew存储的是RVA(文件偏移值（Offset）),需要加上基址(PE的base字段)才得到VA(虽然和FOA通常一样,在内存中成为VA)
            let nt=((self.base as usize)+(*dos).e_lfanew as usize) as *const IMAGE_NT_HEADERS;

            if (*nt).Signature== IMAGE_NT_SIGNATURE{
                Some(nt)
            }
            else {
                None
            }

        }
        
    }

    /// Returns all section headers in the PE.
    /// 得到所有节区切片的引用,失败退出函数返回None
    // &self代表一个PE结构体实例,其内部只存了一个base raw pointer.见注释1
    // 返回值是一个option包裹的slice.rust中&[T]代表slice的引用.详见注释2
    pub fn sections(&self) -> Option<&[IMAGE_SECTION_HEADER]> {
        unsafe {
            // nt代表pe的nt头=*const IMAGE_NT_HEADERS
            let nt = self.nt_header()?;
            
            // 
            let first_section = 
            // nt头从IMAGE_NT_HEADERS强转为字节指针
            (nt as *const u8)
                // 对nt头指针移动(add)size_of(IMAGE_NT_HEADERS)个字节,跨越nt头到达第一个节头的首字节
                .add(size_of::<IMAGE_NT_HEADERS>()) as *const IMAGE_SECTION_HEADER;

            // nt头中FileHeader字段中的NumberOfSections代表节区数量(类型是u16,需要转为usize),以满足from_raw_parts构造slice的要求
            Some(from_raw_parts(first_section, (*nt).FileHeader.NumberOfSections as usize))
        }
    }

    /// Finds a section by its name.
    /// 
    /// 返回的是节区头结构体(和节区实际数据区是分离的),40字节,里面记录了节区名字/RVA(距离pe/dll文件的相对偏移)
    pub fn section_by_name(&self, name: &str) -> Option<&IMAGE_SECTION_HEADER> {
        
        self.sections()?.iter().find(|sec| {
            let raw_name = unsafe {
                
                // win PE结构中,IMAGE_SECTION_HEADER的Name字段类型是[u8;8],这8个字节如果不满以\0 结尾,能够占满就不以\0结尾
                // 关于编码格式:rust里面都是utf-8编码.from_utf8会在运行时进行utf8格式校验.from_utf8_unchecked不校验
                // 合理性:PE文件格式中节区名称(.text等)绝大多数都是ascii(是utf8的子集),天然支持
                 core::str::from_utf8_unchecked(&sec.Name) };

                 // 内存对齐与填充:将内部的\0剥离
            raw_name.trim_end_matches('\0') == name
        })
    }
       
}


/// 重新定义Export结构(PE struct的引用)为了:
/// 
/// 1. 分离出和导出表相关的逻辑,该结构体专注于处理导出表操作,后续可以使用Iterator迭代器 遍历导出表中的内容
/// 2. 未来可扩展,将导出目录的指针缓存到该结构体,不需要每次使用都执行一次查询(增加directory_ptr: *const IMAGE_EXPORT_DIRECTORY,)
// #[derive(Debug)] // 同样只有在调试的时候需要,release中不应该有
pub struct Exports<'a>{
    pub pe:&'a PE,
}

impl <'a> Exports<'a> {

    /// pe->dos header(struct IMAGE_DOS_HEADER)
    /// 
    /// ->nt header(struct IMAGE_NT_HEADERS)
    /// 
    /// ->OptionalHeader(IMAGE_OPTIONAL_HEADER64)
    /// 
    /// ->DataDirectory(DataDirectory: [IMAGE_DATA_DIRECTORY; 16])
    /// 
    /// ->IMAGE_EXPORT_DIRECTORY(里面有三个重要数组)
    pub fn directory(&self)->Option<*const IMAGE_EXPORT_DIRECTORY> {
        
        unsafe {

            // 这里传入的是&self,为啥可以直接使用self?
            // self.pe 实际上等价于 (*self).pe(自动解引用 (Auto-Deref) 特性)
            let nt = self.pe.nt_header()?;

            // 这里为啥要IMAGE_DIRECTORY_ENTRY_EXPORT as usize?
            // Rust 中，数组或切片（Slice）的索引必须是 usize 类型,如果不强转为 usize，编译器会报错 expected usize, found u32
            let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];


            if dir.VirtualAddress== 0 {
                return None;
            }

            // 这里明明指向的是[IMAGE_DIRECTORY_ENTRY_EXPORT]这个数组,为啥要as *const IMAGE_EXPORT_DIRECTORY?
            // OptionalHeader.DataDirectory 是一个拥有 16 个元素的数组。数组的类型是IMAGE_DATA_DIRECTORY
            // 只有转为*const IMAGE_EXPORT_DIRECTORY类型才能以这种类型的指针才能使用
            Some((self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_EXPORT_DIRECTORY)
          
        }

    }
}




// 注释1
// 为什么只传入一个base raw pointer就能代表一个pe结构体实例?
// win的底层开发中,经常只用一个*mut c_void(模块基址/win api中的HMOUDLE/Imagebase)代表整个PE结构体实例
// PE文件在内存中的布局是资办函的相对拓扑解构,只要确定基址(起点),pe内部其他数据解构都能通过RVA+偏移量的方式动态计算出来
// 因此,项目中把base放入PE结构体中的一个字段,该结构体在编译后其内存大小仅8字节,和raw pointer一致;后续通过impl的各种方法解构PE文件的各个结构



// 注释2:数组的引用和切片的引用
// 数组引用:&[u8;10](8字节).普通的8字节单指针
// 切片引用:&[T]:16字节的胖指针(8字节地址+8字节长度)
// 相互转换:rustc可以隐式强制转换,将一个数组引用转为slice引用