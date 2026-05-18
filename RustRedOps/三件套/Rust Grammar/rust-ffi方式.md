

## static linking

在samoa::allocator中

三件套中对于调用win平台下的api函数使用了两种方式,一种是extern "system",一种是windows_targets::link! 它们都是静态链接static linking
```rust
// 以下两种都是static linking

windows_targets::link!("ntdll" "system" fn RtlAllocateHeap(heap: HANDLE, flags: u32, size: usize) -> *mut c_void);

// 注意这里的大括号 {}
extern "system" {
        // 里面声明了一个具体的函数名
        fn NtAllocateVirtualMemory(
            ProcessHandle: *mut c_void,
            BaseAddress: *mut *mut c_void,
            // ...
        ) -> i32;
    }

```
1. 它们都会导致生成的pe文件(.exe/.dll)的IAT中明文出现RtlAllocateHeap\ntdll.dll这样的字符串.
2. puerto中extern "system" 这是rust内置的原始FFI声明,告知编译器,有这样一个函数,调用约定是system(stdcall/fastcall),该函数的具体地址找链接器Linker要.
3. 这种方式无外部依赖,但依赖本地编译环境.即如果在Linux上交叉编译或底层缺少某些.lib导入库,链接器可能找不到符号.对于本地win64的环境编译的产物,是否足以用在目标平台上,如果目标平台是win64或win32或出现错误吗?如果目标平台是win11之前的版本win7等会出现错误吗?
4. 如果本地是win64,目标平台是win32.一定会报错.解决方案是在编译时指定目标:argo build--target i686-pc-windows-msvc (尚未验证)
5. 反之编译win32->目标平台win64,通常可以依靠wow64子系统运行 (尚未验证)
6. 跨操作系统版本情况:win11开发机->win7目标机器.完美运行
7. 关于兼容性,涉及到linker的工作原理.使用 extern "system" 链接 RtlAllocateHeap时,linker并没有把这个函数的真实物理代码打包到木马中,也没有硬编码具体内存地址.linker在木马文件(PE文件)的IAT中写入ntdll.dll/RtlAllocateHeap .而Windows完美向后兼容,且RtlCreateHeap、RtlAllocateHeap 和 RtlFreeHeap 这三个 NTAPI，是整个 Windows帝国的内存基石。从 1993 年的 Windows NT 3.1 一直到 2024 年的 Windows11，它们的函数签名（接收几个参数、返回值类型）三十年里一个字节都没有变过.
8. hypnus中windows_targets::link! 这是微软维护的Windows-rs生态一部分.不需要本地有Windows sdk或.lib导入文件,能保证无论在什么平台编译,链接出来的符号都是正确的
9. 但这种方式引入了额外的宏和crate依赖



## 动态函数指针类型定义(Dynamic Function Pointer Signature)

和上面static linking之间的区别:使用{}还是()
```rust
    // 注意这里的大括号 {}
    // static linking
    extern "system" {
       // 里面声明了一个具体的函数名
        fn NtAllocateVirtualMemory(
            ProcessHandle: *mut c_void,
           BaseAddress: *mut *mut c_void,
            // ...
        ) -> i32;
    }

// Dynamic Function Pointer Signature
// 注意这里用的是 type，并且没有大括号 {}，直接跟了 fn
    pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
        ProcessHandle: *mut c_void,
        BaseAddress: *mut *mut c_void,
        // ...
    ) -> i32;
```

1. 编译器理解:要调用一个名字精确叫 NtAllocateVirtualMemory的函数。虽然代码不在这里，但在链接（Linking）阶段，链接器（Linker）必须去外部的DLL（比如 ntdll.dll）里找到这个名字，并把它记录在我的 IAT 表里.因此会产生IAT记录
2. 编译器理解:定义了一种新的数据类型，名字叫NtAllocateVirtualMemoryFn。这种数据类型的本质是一个‘函数指针’。如果在运行时（Runtime），程序员把某个内存地址强制塞进这个类型里，并且调用了它，那么我（编译器）必须按照system（Windows stdcall）的规则来安排 CPU 寄存器和压栈顺序
3. 关键点：编译器根本不关心这个指针最终指向的函数叫什么名字，也不关心它在哪个 DLL里。它只关心“怎么传参”。因为没有绑定具体的名字，链接器（Linker）根本不会去找外部库，当然也就绝对不会在 IAT 表中留下任何痕迹