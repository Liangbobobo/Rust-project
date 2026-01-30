 pub fn get_forwarded_address(
    6     _module: *const i8, // 这里的 module
      参数在你的代码片段中未被使用，保留以匹配签名
    7     address: *mut c_void,
    8     export_dir: *const IMAGE_EXPORT_DIRECTORY,
    9     export_size: usize,
   10     hash: fn(&[u16]) -> u32,
   11 ) -> Option<*mut c_void> {
   12     // 1. 范围检查：判断是否指向导出表内部（即是否为转发器）
   13     let addr_usize = address as usize;
   14     let dir_usize = export_dir as usize;
   15
   16     if addr_usize >= dir_usize && addr_usize < dir_usize + export_size {
   17         unsafe {
   18             // 2. 获取原始字节切片（避免 UTF-8 检查）
   19             // CStr::from_ptr 会自动扫描直到遇到 \0，非常高效且相对安全
   20             let c_str = CStr::from_ptr(address as *const i8);
   21             let bytes = c_str.to_bytes(); // 得到 &[u8]，这是纯 ASCII 字节
   22
   23             // 3. 在字节流中寻找 '.' (ASCII 46)
   24             // position 返回的是相对于 bytes 起始位置的索引
   25             if let Some(dot_index) = bytes.iter().position(|&b| b == b'.')
   26                 // 4. 切分切片
   27                 let dll_name_bytes = &bytes[..dot_index];
   28                 // dot_index + 1 跳过 '.' 符号
   29                 let func_name_bytes = &bytes[dot_index + 1..];
   30
   31                 // 5. 转换为 u16 (Wide String) 以适配 hash 函数
   32                 // 转发器格式通常是 "DLL.Func"，是 ASCII，直接强转为 u16
      即可
   33                 let dll_name_u16: Vec<u16> = dll_name_bytes
   34                     .iter()
   35                     .map(|&b| b as u16)
   36                     .collect();
   37
   38                 let func_name_u16: Vec<u16> = func_name_bytes
   39                     .iter()
   40                     .map(|&b| b as u16)
   41                     .collect();
   42
   43                 // 6. 调用 Hash 函数 (这里演示逻辑，具体如何使用 hash
      查找模块基址取决于你的 helper 实现)
   44                 let _dll_hash = hash(&dll_name_u16);
   45                 let _func_hash = hash(&func_name_u16);
   46
   47                 // 在这里你需要编写加载目标 DLL 并获取函数地址的逻辑
   48                 // 例如：LdrLoadDll -> LdrGetProcedureAddress
   49                 // 因为这部分逻辑比较复杂，通常涉及到递归调用
      get_module_handle/get_proc_address
   50
   51                 // 返回值通常是解析后的真实地址
   52                 // return Some(real_address);
   53
   54                 // 临时占位
   55                 return None;
   56             }
   57         }
   58     }
   59
   60     // 如果不是转发器，address 本身就是函数代码的起始地址
   61     // 但根据函数名 get_forwarded_address，如果不是转发器通常返回 None
      或原地址？
   62     // 这里视你的设计而定，如果只处理转发，则返回 None
   63     None
   64 }

  关键点解释

   1. `CStr::from_ptr(...).to_bytes()`:
      这是核心。它帮你处理了“寻找空字符结尾”的底层逻辑，返回一个安全的 Rust
  &[u8] 切片。这比你自己写 while *ptr != 0 既安全又快，而且完全不进行 UTF-8
  校验。

   2. `bytes.iter().position(|&b| b == b'.')`:
      这是替代 split_once 的字节级操作。它在内存中查找 ASCII 码为 46 的字节。

   3. `map(|&b| b as u16)`:
      PE 导出表里的转发字符串是 ASCII 的，但 Windows API 和你的哈希函数需要
  Unicode (u16)。这里直接将 u8 强转为 u16 是一种标准的 ASCII -> Wide
  转换方式（比使用 MultiByteToWideChar 系统 API 更快且不依赖 OS）。

   4. 关于 `todo!()`:
      转发器处理是非常复杂的。解析出 NTDLL 和 RtlAllocateHeap 后，你实际上需要：
       * 看 NTDLL 是否已加载（遍历 LDR 链表）。
       * 如果没有加载，调用 LdrLoadDll 加载它。
       * 在目标 DLL 中再次调用你的 get_proc_address 逻辑来寻找 RtlAllocateHeap。
      这就是为什么这通常需要递归或者回调的原因。