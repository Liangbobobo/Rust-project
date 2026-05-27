

## TP_CALLBACK_ENVIRON_V3 

是win线程池 api中用于底层环境配置的结构体.
1. 本质上是C语言二进制接口（ABI）规范的内存块.
2. 用于告诉os调度器,按照该结构体的环境去执行回调代码

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TP_CALLBACK_ENVIRON_V3 {
    pub Version: u32,
    pub Pool: *mut c_void,
    pub CleanupGroup: *mut c_void,
    pub CleanupGroupCancelCallback: *mut c_void,
    pub RaceDll: *mut c_void,
    pub ActivationContext: isize,
    pub FinalizationCallback: *mut c_void,
    pub u: TP_CALLBACK_ENVIRON_V3_0,
    pub CallbackPriority: i32,
    pub Size: u32,
}

impl Default for TP_CALLBACK_ENVIRON_V3 {
    fn default() -> Self {
        Self {
            Version: 3,
            Pool: null_mut(),
            CleanupGroup: null_mut(),
            CleanupGroupCancelCallback: null_mut(),
            RaceDll: null_mut(),
            ActivationContext: 0,
            FinalizationCallback: null_mut(),
            u: TP_CALLBACK_ENVIRON_V3_0 { Flags: 0 },
            CallbackPriority: 1,
            Size: size_of::<TP_CALLBACK_ENVIRON_V3>() as u32,
        }
    }
}
```

