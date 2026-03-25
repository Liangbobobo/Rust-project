## pub fn ignoring_set_fpreg

**该函数核心任务:**  
1. 通过win64的二进制元数据,精确计算一个函数在执行时占用多少字节的栈空间
2. 该函数手动模拟windows内核(RtlVirtualUnwind)的行为

```rust
pub fn ignoring_set_fpreg(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<u32> {...}
```
1. module:dll的基址(如kernelbase.dll的内存起始位置)
2. runtime:指向.pdata节中的一个条目.该条目记录了某个函数的起始地址\结束地址\UnwindData(即uwd中的IMAGE_RUNTIME_FUNCTION)
3. 返回该函数在栈上共分配多少字节,以u32表示

