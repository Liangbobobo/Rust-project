use core::{f32::consts, ffi::c_void};
use crate::types::{IMAGE_DOS_HEADER,IMAGE_NT_HEADERS, IMAGE_NT_SIGNATURE};
// 将PE结构体抽象出来,是整个项目更加清晰
/// 
/// 这是一种设计模式,“类型状态模式” (Type State / Newtype Pattern) 或者简单的 “封装抽象”
/// 
// 这样做1.含义清晰(代表一个pe文件格式的内存)2.可以关联方法 3.零成本抽象(该结构体编译后,在内存中布局和*mut c_void是一样的,无额外内存开销及性能损耗) 4. 后续方便扩展
// #[derive(Debug)] // realse版本中不应该用,会增加特征字符串/增加二进制体积,且无实际用途
pub struct PE {
    /// Base address of the loaded module.
    pub base: *mut c_void,
}

impl PE {

    /// Creates a new `PE` instance from a module base.
    #[inline]
    pub fn prase(base:*mut c_void) ->Self{
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
let nt=((self.base as usize)+(*dos).e_lfanew as usize) as *const IMAGE_NT_HEADERS;

            if (*nt).Signature== IMAGE_NT_SIGNATURE{
                Some(nt)
            }
            else {
                None
            }


        }
        
    }
}


/// 重新定义Export结构(PE struct的引用)为了:
/// 
/// 1. 分离出和导出表相关的逻辑,后续可以使用Iterator迭代器 遍历导出表中的内容
/// 2. 未来可扩展,将导出目录的指针缓存到该结构体,不需要每次使用都执行一次查询(增加directory_ptr: *const IMAGE_EXPORT_DIRECTORY,)
// #[derive(Debug)] // 同样只有在调试的时候需要,release中不应该有
pub struct Exports<'a>{
    pub pe:&'a PE,
}

impl <'a> Exports<'a> {
    
}