关于你的疑问：resolve_api_set_map 返回 u8（即
  &[u8]）在语义上是可以的，但由于 Windows PEB 中的字符串（包括 API Set
  的物理映射名）原生就是 UTF-16
  (`u16`)，最安全且最高效（零分配）的方案是直接返回 `Option<&[u16]>`。

  这样你可以直接将结果传给你的 hash 函数（它的签名是 &[u16]），完全避免了在
  u8 和 u16 之间来回转换。

  以下是实现 resolve_api_set_map 剩余逻辑以及在 get_forwarded_address
  中安全使用的方案：

  1. resolve_api_set_map 的完整实现

  在 types.rs 中确保你有 API_SET_VALUE_ENTRY 结构体。

    1 fn resolve_api_set_map<'a>(
    2     host_name: *const i8, // 宿主模块名 (用于过滤重定向)
    3     contract_name: &[u8], // api set 契约名 (ASCII)
    4 ) -> Option<&'a [u16]> {
    5     unsafe {
    6         let peb = NtCurrentPeb();
    7         let map = (*peb).ApiSetMap;
    8         let base = map as usize;
    9
   10         let ns_entry_ptr = (base + (*map).EntryOffset as usize) as
      *const API_SET_NAMESPACE_ENTRY;
   11         let ns_entries = from_raw_parts(ns_entry_ptr, (*map).Count as
      usize);
   12
   13         for entry in ns_entries {
   14             // 获取 ApiSet 名字的 RVA 并转为 u16 切片
   15             let name_u16 = from_raw_parts(
   16                 (base + entry.NameOffset as usize) as *const u16,
   17                 entry.NameLength as usize / 2
   18             );
   19
   20             // 方案：使用迭代器比较 &[u16] 和 &[u8] (免分配)
   21             if name_u16.len() >= contract_name.len() &&
   22                contract_name.iter().zip(name_u16.iter()).all(|(&a, &b)
      a as u16 == b)
   23             {
   24                 // 找到匹配的 Entry，现在解析 Value (物理映射)
   25                 let values = from_raw_parts(
   26                     (base + entry.ValueOffset as usize) as *const
      API_SET_VALUE_ENTRY,
   27                     entry.ValueCount as usize
   28                 );
   29
   30                 if values.is_empty() { return None; }
   31
   32                 // 核心：如果有多个映射值，需要根据 host_name 过滤
   33                 // 比如某些 API 在不同的宿主下会重定向到不同的 DLL
   34                 let mut target_val = &values[0];
   35
   36                 if values.len() > 1 && !host_name.is_null() {
   37                     for val in values {
   38                         if val.NameLength != 0 {
   39                             let import_name = from_raw_parts(
   40                                 (base + val.NameOffset as usize) as
      *const u16,
   41                                 val.NameLength as usize / 2
   42                             );
   43
   44                             // 检查宿主名是否匹配
      (这里需要实现一个简单的 u16 与 i8 比较)
   45                             if compare_u16_i8(import_name, host_name)
   46                                 target_val = val;
   47                                 break;
   48                             }
   49                         }
   50                     }
   51                 }
   52
   53                 // 返回真正的物理 DLL 名字的 u16 切片
   54                 return Some(from_raw_parts(
   55                     (base + target_val.ValueOffset as usize) as *const
      u16,
   56                     target_val.ValueLength as usize / 2
   57                 ));
   58             }
   59         }
   60     }
   61     None
   62 }
   63
   64 // 辅助函数：比较 UTF-16 切片和 C 风格 ASCII 字符串
   65 unsafe fn compare_u16_i8(u16_str: &[u16], i8_ptr: *const i8) -> bool {
   66     let mut i = 0;
   67     while i < u16_str.len() {
   68         let c = *i8_ptr.add(i);
   69         if c == 0 || (u16_str[i] != c as u16) { return false; }
   70         i += 1;
   71     }
   72     *i8_ptr.add(i) == 0 // 确保长度也一致
   73 }

  2. 在 get_forwarded_address 中的下一步处理

  在获取到模块地址后，你需要递归调用一次查找逻辑，因为转发可能有多层（虽然罕
  见）。

    1             // ... 接你的代码 ...
    2             if dll_name_bytes.starts_with(b"api-ms") ||
      dll_name_bytes.starts_with(b"ext-ms") {
    3                 let last_index = dll_name_bytes.iter().rposition(|&b|
      == b'-').unwrap_or(dll_name_bytes.len());
    4
    5                 if let Some(resolved_mod_u16) =
      resolve_api_set_map(module, &dll_name_bytes[..last_index]) {
    6                     let h_mod =
      retrieve_module_add(Some(hash(resolved_mod_u16)), Some(hash));
    7
    8                     if let Some(base) = h_mod {
    9                         // 将函数名转为 u16 并在目标模块中递归查找
   10                         return lookup_recursive(base, func_name_bytes,
      hash);
   11                     }
   12                 }
   13             } else {
   14                 // 普通 DLL 处理：比如 "NTDLL.RtlAlloc"
   15                 // 你需要在栈上构造 "NTDLL.DLL" 的 u16 数组来计算模块
      hash
   16                 let mut buf = [0u16; 64];
   17                 let mut len = 0;
   18                 for &b in dll_name_bytes { buf[len] = b as u16; len +=
      1; }
   19                 // 补全后缀以匹配 retrieve_module_add 里的 Ldr 链表
   20                 for &b in b".DLL" { buf[len] = b as u16; len += 1; }
   21
   22                 if let Some(base) =
      retrieve_module_add(Some(hash(&buf[..len])), Some(hash)) {
   23                     return lookup_recursive(base, func_name_bytes,
      hash);
   24                 }
   25             }

  为什么这样写最安全？

   1. 内存安全：resolve_api_set_map 返回的是指针切片
      &[u16]，它的生命周期直接绑定在 PEB
      所在的内存区域。在进程运行期间，这块内存是合法的，不会发生悬垂指针。
   2. 类型对齐：通过返回 &[u16]，你完美契合了 hash(buffer: &[u16])
      的参数要求。
   3. 零副作用：你没有修改 PEB，没有调用任何可能会被 EDR 钩住的系统函数（如
      wcscmp 或 String 转换），所有的比较逻辑都是你手写的纯内存操作。

  下一步建议：
  如果你在 retrieve_module_add 找不到模块，说明目标 DLL
  可能尚未加载。在高级重构中，你通常需要在这里实现一个简单的 LoadLibrary
  逻辑，或者通过解析 Ldr 尝试手动加载。