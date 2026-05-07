**永远记得用Windbg去验证学到的概念,这是最重要的没有之一**

## Windows 10 and OneCore

Over the years, several flavors of Windows have evolved.多年来,win已经逐渐形成了多种风格  
win8/win phone8已经共享kernel;With win10 the convergence融合 is complete; this shared platform is known as OneCore, and it runs on PCs, phones, the Xbox One game console, the HoloLens(微软的全息眼镜) and Internet of Things (IoT) devices such as the Raspberry Pi 2树莓派.  
比如HoloLens不需要鼠标和物理键盘的相关功能

This book delves into深入探究 the internals of the OneCore kernel, on whatever device it’s running on无论OneCore运行在什么设备上.即本书研究的是所有windows设备的基石OneCore 

## Windows API

The Windows application programming interface (API) is **the user-mode system programming interface**  to the Windows OS family.  
Prior to the introduction of 64-bit versions of Windows介绍64位版本的win之前,win32 api为了与win16 api进行区别.  
本书中提到的Windows api refers to both the 32-bit and 64-bit programming interfaces to Windows.  
有时会使用win32 api in lieu of代替 Windows API,Either way无论哪种方式, it still refers to the 32-bit and 64-bit variants  

资源:
1. The Windows API is described in the Windows SDK(“Windows Software Development Kit工具/工具箱) documentation.微软的开发者平台有免费的文档
2. MSDN
3.  the book Windows via C/C++, Fifth Edition by Jeffrey Richter and Christophe Nasarre

### Windows API flavors

The Windows API originally consisted of C-style functions only.   
The downside was the sheer number大量 of functions coupled with加上 the lack of naming consistency命名一致性/命名空间 and logical groupings逻辑分组 (for example, C++ namespaces). One outcome结果 of these difficulties resulted in some newer APIs using a different API mechanism: the Component Object Model (COM).  
这种机制（指旧版 Windows API）的缺点在于，API函数数量庞大，且缺乏命名规范和逻辑分组（例如 C++命名空间）。这些困难导致的后果之一，便是促使一些较新的 API采用了另一种不同的 API 机制：组件对象模型（Component Object Model,COM）

COM was originally created to enable Microsoft Office applications to communicate and exchange data between documents (such as embedding an Excel chart inside a Word document or a PowerPoint presentation).   
This ability is called Object Linking and Embedding (OLE,对象的链接与嵌入). OLE was originally implemented using an old Windows messaging mechanism called Dynamic Data Exchange (DDE). DDE was inherently limited, which is why a new way of communication was developed: COM. In fact, COM initially was called OLE 2, released to the public circa 1993.

**COM是windows二进制生态系统的通用语言**

COM is based on two foundational principles. 
1. First, clients communicate with objects (sometimes called COM server objects) through interfaces—well-defined contracts with a set of logically related methods grouped under the virtual table dispatch mechanism虚函数分发机制, which is also a common way for C++ compilers to implement virtual functions dispatch. This results in binary compatibility and removal of compiler name mangling issues符号修饰问题. Consequently结果/因此, it is possible to call these methods from many languages (and compilers), such as C, C++, Visual Basic, .NET languages, Delphi and others.
* 原理:COM借鉴了vtable的机制(rust中是Trait Objects的动态分发),COM接口本质是一个指针数值的地址(vtable的指针).只要知道对象的内存结构,无论采用什么语言,只要能读取对应的内存地址并跳转到对应的偏移,就能调用该函数
* Name Mangling:不同的c++编译器(MSVC/GCC)对函数名有不同的修饰逻辑.这意味着用MSVC编译的库,如果由GCC链接,通常会报错"无法解析的外部符号"
* COM不用基于函数名的链接方式,转而使用基于内存偏移量的调用方式.接口方法在vtable中的位置(偏移量)固定.只要ABI定义好,无论编译器如何命名,调用方只要去对应的偏移量处取地址就行.
* Binary Compatibility二进制兼容:调用约定统一为二进制层面的跳转.开发者可以用c++写COM组件,用c#的RuntimeCallableWrapper (RCW)/Rust的windows-rs映射接口调用.**即COM定义的不是某种语言,而是一段内存布局规范,这使得操作系统可以在底层用c/c++实现服务,而在上层提供给各种应用使用**
* 在用Windbg时,出现的大量`call dword ptr [eax+8]`,这就是COM组件通过vtable调用的典型特征.
* 攻击面:COM劫持就是替换一个对象所指向的vtable或修改其中的函数地址,就接管了该COM组件的所有行为

2. The second principle is that component implementation is loaded dynamically rather than being statically linked to the client

The term COM server typically refers to a Dynamic Link Library (DLL) or an executable (EXE) where 
the COM classes are implemented.