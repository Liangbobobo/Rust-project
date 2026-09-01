Rust std中core::option::Option对unwrap底层实现

```rust
pub const fn unwrap(self) -> T {
        match self {
            Some(val) => val,
            None => panic!("called `Option::unwrap()` on a `None` value"),
        }       
    }
```

1. 由此可见,unwrap本质是一个内部自带panic!宏的match! 一旦遇到None,会无条件触发panic!,进而调用panic_handler,如果程序中没有特殊的异常恢复,整个进程会退出.在win的生产环境或敏感服务中,会导致0xC0000005或STATUS_FATAL_APP_EXIT 崩溃弹窗
2. 对match:它不是异常处理,是正常的条件控制流.遇到None,只执行_=>None分支,把控制权交给调用者.调用者可以决定重试/更换DLL模块/静默退出
3. match编译出的机器码极致纯净,unwrap()的机器码注入了painc!(call core::panicking::panic) 同时打印错误信息