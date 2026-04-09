## Struct StackSpoof

```rust
/// Represents a reserved stack region for custom thread execution.
#[derive(Default, Debug, Clone, Copy)]
pub struct StackSpoof {
    /// Address of a `gadget_rbp`, which realigns the stack (`mov rsp, rbp; ret`).
    gadget_rbp: u64,

    /// Stack frame size for `BaseThreadInitThunk`.
    base_thread_size: u32,

    /// Stack frame size for `RtlUserThreadStart`.
    rtl_user_thread_size: u32,

    /// Stack frame size for `EnumResourcesW`.
    enum_date_size: u32,

    /// Stack frame size for `RtlAcquireSRWLockExclusive`.
    rlt_acquire_srw_size: u32,

    /// Type of gadget (`call [rbx]` or `jmp [rbx]`).
    gadget: GadgetKind,
}

```



## 申请第二个4k内存

```rust
  // Allocate pointer to gadget
        let mut gadget_ptr = null_mut();
        let mut ptr_size = 1 << 12;
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut gadget_ptr, 
            0, 
            &mut ptr_size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate gadget pointer page"));
        }
```

**为什么要再次申请4kb内存**
1. call rbx 与 call `[rbx]` 的区别:
* call rbx,cpu直接跳转到rbx中的内存位置.这种情况不需要第二块内存
* call `[rbx]`,cpu去rbx指向的内存中读取数据.如果cpu此时将该数据当作地址跳转,os会立即access violation.
  * 内存中不分指令和数据,指令层面才区分.call `[rbx]`时,cpu内部控制单元触发一次内存总线读取.去rbx指向的内存读取**8字节**数据.此时cpu读取的这8字节,在cpu视角下是不分数据/指令的
  * call指令除了读取,还会将读到的数据填入rip.且cpu会无条件的到rip指向的位置读取下一条要执行的指令
* 加上第二块内存的解决方案:
  * cpu执行call `[rbx]`.这里rbx中存入的是第二块内存的地址.
  * cpu读取8字节(这8字节被预先填入了第一块内存的地址)
  * cpu将该8字节填入rip
  * cpu执行压栈操作,把call指令下面那条指令地址压入当前栈,rsp-8,cpu把rip的当前值(即加上指令长度的rip,也叫返回地址)写入rsp中.cpu读取该8字节地址后,根据该该地址内部指令长度,加上rip的地址.让rip指向下一跳指令地址.
  * cpu之后通过rip找到第一块内存
  * cpu开始执行第一块内存中的指令


## RtlUserThreadStart

spoof.rs中通过
```rust
 let rtl_user = pe_ntdll
            .function_by_offset(cfg.rtl_user_thread.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32)
            .context(s!("missing unwind: RtlUserThreadStart"))?;
```

**背景知识**
win下,当创建一个线程时,cpu并不直接进入对应的代码,cpu首先进入ntdll!RtUserThreadStart.
* 该函数负责初始化thread的seh环境,并真正call业务代码.是所有线程的开始
* 物理位置: 是所有合法线程调用栈的物理最底层(栈底)
* EDR进行线程审计时,顺着当前线程的栈指针一路回溯,检查最后的终点是不是该函数.

**函数原型及特性**