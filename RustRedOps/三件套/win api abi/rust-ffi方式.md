

三件套中对于调用win平台下的api函数使用了两种方式,一种是extern "system",一种是windows_targets::link! 它们都是静态链接static linking
1. 它们都会导致生成的pe文件(.exe/.dll)的IAT中明文出现RtlAllocateHeap\ntdll.dll这样的字符串.
2. puerto中extern "system" 这是rust内置的原始FFI声明,告知编译器,有这样一个函数,调用约定是system(stdcall/fastcall),该函数的具体地址找链接器Linker要.
3. 这种方式无外部依赖,但依赖本地编译环境.即如果在Linux上交叉编译或底层缺少某些.lib导入库,链接器可能找不到符号.对于本地win64的环境编译的产物,是否足以用在目标平台上,如果目标平台是win64或win32或出现错误吗?如果目标平台是win11之前的版本win7等会出现错误吗?
4. 如果本地是win64,目标平台是win32.一定会报错.解决方案是在编译时指定目标:argo build--target i686-pc-windows-msvc (尚未验证)
5. 反之编译win32->目标平台win64,通常可以依靠wow64子系统运行 (尚未验证)
6. 跨操作系统版本情况:win11开发机->win7目标机器.完美运行
7. 关于兼容性,涉及到linker的工作原理.使用 extern "system" 链接 RtlAllocateHeap时,linker并没有把这个函数的真实物理代码打包到木马中,也没有硬编码具体内存地址.linker在木马文件(PE文件)的IAT中写入ntdll.dll/RtlAllocateHeap .而Windows完美向后兼容,且RtlCreateHeap、RtlAllocateHeap 和 RtlFreeHeap 这三个 NTAPI，是整个 Windows帝国的内存基石。从 1993 年的 Windows NT 3.1 一直到 2024 年的 Windows11，它们的函数签名（接收几个参数、返回值类型）三十年里一个字节都没有变过.
8. hypnus中windows_targets::link! 这是微软维护的Windows-rs生态一部分.不需要本地有Windows sdk或.lib导入文件,能保证无论在什么平台编译,链接出来的符号都是正确的
9. 但这种方式引入了额外的宏和crate依赖