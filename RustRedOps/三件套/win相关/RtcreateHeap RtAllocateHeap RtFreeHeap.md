
## 函数原型

https://ntdoc.m417z.com/rtlcreateheap

## RtlCreateHeap

在samoa/allocator.rs中,该函数这样使用

```rust
 let handle = unsafe { 
            RtlCreateHeap(
                HEAP_GROWABLE, 
                null_mut(), 
                0, 
                0, 
                null_mut(), 
                null_mut()
            ) 
        };
```

定位:这是Windows NTDLL导出的最核心的内存分配原语.如在c++的malloc 或 new，在 Windows 下最终都会走到这里

作用：它向 Windows 操作系统的内存管理器申请，在当前进程的虚拟内存空间中，划分出一块全新的、独立的区域，专门作为一个“私有堆”来使用

返回值 (HANDLE)：它返回一个指向极其复杂的内部结构体 _HEAP的指针。后续所有的 RtlAllocateHeap操作，都要拿着这个令牌，告诉系统“这个私有堆里分配内存”。