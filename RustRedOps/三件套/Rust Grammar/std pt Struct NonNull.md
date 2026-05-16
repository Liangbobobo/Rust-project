# std::ptr::NonNull

```rust
pub struct NonNull<T>
where
    T: ?Sized,{ /* private fields */ }
```

*mut T but non-zero and covariant.

This is often the correct thing to use when building data structures using raw pointers,   
but is ultimately最终/根本上 more dangerous to use because of its additional properties. If you’re not sure if you should use `NonNull<T>`, just use *mut T!
1. Unlike *mut T, the pointer must always be non-null, even if即使 the pointer is never dereferenced. This is so that enums may use this forbidden value as a discriminant使用这个被禁用的值(0/null)作为判别式 – `Option<NonNull<T>>`has the same size as *mut T. However the pointer may still dangle垂悬 if it isn’t dereferenced.
   * 编译器保证`NonNull<T>`为non-null,但不保证解引用后的T是合法的地址
   * 在enum中这里的`Option<NonNull>`

Discriminant (判别式/标签)：这是 Rust枚举内部的一个隐藏字段，用来标记当前是哪个成员。NonNull的优化在于把“0”这个地址直接借用来当成了 Option::None 的判别式。
   1. Forbidden value (被禁止的数值)：在 NonNull 的世界里，数字 0是被禁止存在的。一旦出现 0，整个类型系统的安全契约就崩塌了。
   2. Dangle(悬挂)：动词。指指针指向的内存已经被操作系统收回（比如堆被销毁了），虽然指针数值不是 0，但它已经变成了无头苍蝇。


## NonNull::new_unchecked(handle)

如果不用NonNull::new_unchecked():  
NonNull初始化的方式是NonNull::new.在samoa中如果new失败,要使用NonNull::new(handle).unwrap()处理失败的情况:

如果只使用了NonNull::new(handle)没有使用后面的unwrap():
1. RtlCreateHeap 失败，返回 0
2. NonNull::new(0) 返回 None,HEAP_HANDLE 变成了 None
3. 后续调用RtlAllocateHeap(0,0,size)时,把0当作堆句柄给了ntdll.dll. win的堆管理器试图去内存地址为0处读取_HEAP结构体来分配内存.结果会瞬间触发0xC0000005 (Access Violation) 访问违例异常

如果使用了unwrap()/unwrap_or_else()来处理NonNull::new(handle)为none的情况:
1. 在std下使用unwrap(),在失败情况下,标准库会打印错误信息\回溯调用栈,然后结束进程
2. 在`#[no_std]`下,编译器会强制要求手动实现`#[panic_handler] `标签的函数。如果不写，连编译都通不过
3. 如果自定义了`#[panic_handler] `,在免杀工具为了不依赖os,在`#[panic_handler]  fn painc()->!{}`函数体中要么放入loop{}陷入死循环,卡死当前线程.要么触发汇编异常指令,让进程被系统强杀(unsafe { core::arch::asm!("ud2") })
4. 在3中实现的情况,要么陷入死循环导致cpu飙升,要么自定义painc但会暴露出错的文件名,错误提示字符串作为名为你硬编码放入二进制文件.rdata段中(后期可以自定义实现,看看到底出现什么情况)
5. 即unwrap() 并失败情况：程序会跳入您预先写好的`#[panic_handler]`（通常是死循环或强杀指令）。更要命的是，它会在木马内留下极其明显的明文溯源证据（文件名和 Panic 提示字符串）
6. unwrap_or_else()会调用指定函数.但是如果失败了应该返回什么(rustc要求此处必须返回一个合法的`NonNull<c_void>`)
7. 当RtlCreateHeap 失败（返回0）时,实际上是os表示已经没有一点内存了.再返回什么都是灾难
8. 因此只能直接用new_unchecked(handle),即使handle为0也继续下去,等到分配内存时,被os的内存保护机制(段错误)处理
9. 在这种情况下,无法分配内存已经没有进一步执行下去的必要了,直接将错就错让os处理这种错误
10. 



## 官方文档认为Nonull危险建议不明确时使用*mut T

Nonull能防空指针,官方文档怎么会说它危险,并明确不确定时使用*mut T
1. 这里不是指内存崩溃危险,而是欺骗编译器的危险.Nonull时协变covariance的,而*mut T时不变的.因此编译器为Nonull开启了协变绿灯,可能会把一个短生命周期的变量放在一个期望长生命周期的集合中,导致释放后使用的内存漏洞
2. 如果通过ffi从win api拿到一个指针,不加考虑的对该指针使用了Nonull,但这个指针在win api中实际返回了一个null.这就等于欺骗了rust编译器,从而引发未定义行为ub
3. 官方的意思是,如果不是在写底层vec/hashmap,只是用来传递数据,老老实实使用*mut c_void是最不容易引起复杂生命周期漏洞的做法

## 使用NonNull的原因

在构建底层数据结构时，几乎一面倒地推荐使用 `Option<NonNull<T>>`

1. 无性能开销:`Option<NonNull<T>>` 和 *mut T在内存中占用的大小完全一样（在 64 位下都是 8 字节，None 就是物理上的0x00000000），它们在性能和内存开销上是绝对零差别的.
2. 将“空指针检查”从人脑转移给编译器（类型驱动开发）:在 C/C++（或使用 *mut T）中，拿到一个指针，最可怕的噩梦是：“它到底是不是NULL？”如果忘了写 if (ptr != NULL)，直接去读写，就会引发段错误（Segfault）
3. 类型协变性 (Covariance):协变性决定了具有生命周期的类型能否互相转换.简单的说协变就是长生命周期的子类型能否当作短生命周期的父类型使用.
  * *mut T 在 Rust 中是 不变（Invariant）的.如`MyVec<*mut T>`，那么 `MyVec<&'static str> `绝对不能当做 `MyVec<&'a str>`



## 扩展-NonNull`<u8>`的含义

指针大小和指针指向的对象的大小是两个不同的概念

指针大小:在64位系统中,指针本身永远是64位的(8字节),其物理本质是一个完整的64位的cpu内存地址,可以寻址16EB的虚拟内存空间

`NonNull<u8>`就是保证非空的`*mut u8`,即Raw Pointer.这里的u8表示,当通过这个指针去读内存时,最小的单位是多少,即一次可以读取多少位的数据.如果是u32,即一次读取4字节的数据

之所以使用u8,因为此时rustc不知道这个字段具体存储的结构,只知道是一串原始的 按字节排列的二进制数据;在底层编程中,u8常用作地址占位符,告知rustc,这里有一块内存,先把他当作一堆原始字节,稍后会通过偏移量手动解析,以达到最大的灵活性;同时可以避免对齐陷阱,u8对齐要求是1字节,可以指向内存中的任何位置,不会触发rustc的对齐警告

当看到 *mut u8 或 `NonNull<u8>`代表：  
这是一个 “万能地址”,它本身是 8 字节（在64位 环境下）,含义：它指向的地方存着东西，但具体是什么，我们要看代码随后是怎么解释（cast）它的

## 扩展-Arguments

* #[lang = "format_arguments"]  
  * 一个 Language Item (语言项) 声明:通常情况下，Rust代码是运行在编译器制定的规则之下的。但有些时候，编译器需要知道某些特定的结构体在哪里定义，以便它能亲自参与这些结构体的构造
  * format_args! 宏展开时，生成的代码会直接操作 Arguments结构体的内存布局。如果不加这个属性，编译器就不知道哪个结构体是它亲生的“格式化容器”
  * 底层影响：它打破了常规的封装。即使 Arguments的字段是私有的，编译器也能直接在栈上给它们赋值

* #[stable(feature = "rust1", since = "1.0.0")]
  * 含义：声明该接口自 Rust 1.0.0 版本起就已经稳定
  * 背景知识：这保证了向后兼容性。无论你以后把 Rust升级到哪个版本，这套底层的格式化协议都不会变。这对于需要长期运行的内核驱动或底层库来说是至关重要的

* #[derive(Copy, Clone)]
  *  让结构体具备“按位拷贝”的能力
  *  由于 Arguments 内部全是原始指针（NonNull），拷贝它的代价极小（在 x64下就是 16 字节的赋值）;在格式化流程中，这个对象会被频繁地传递。具备 Copy特性意味着它可以在不触发“所有权转移”逻辑的情况下，通过寄存器或栈快速传递给下游函数

* pub struct Arguments<'a>
  * lifetime `a : 由于Arguments本身不持有数据,支持有对数据的引用(通过指针).
  * `a 确保只要Arguments还活着,它指向的那些栈变量就不能被销毁,杜绝了垂悬指针导致的崩溃

* template: NonNull`<u8>`
  * 含义：一个指向模板数据的非空原始指针
  * 类型深度解析：`NonNull<u8>`：
      * u8：这里并不代表一个字节，而是代表一个未解析的原始内存地址(详细解释:[扩展-NonNull`<u8>`的含义](#扩展-nonnullu8的含义))
* 背景知识：模板里存了什么？
       * 它指向的是一段由编译器生成的二进制元数据块。
       * 这段数据包含：静态字符串片段的地址、占位符的数量、以及每个占位符的类型
         指令。
* OpSec 启示：杀软静态扫描时，会追踪这个指针指向的 .rdata区域。那里是你程序“说话内容”的原材料库


* args: `NonNull<rt::Argument<'a>>`
* 含义：一个指向参数数组的非空原始指针
* `rt::Argument<'a>`：rt 代表 Runtime（运行时）。这说明这个结构体是为运行时处理设计的
* 每一个 Argument 实际上是一对指针：`(&Value, &Formatter)`
    * Value：指向你真实的变量（如 0x1234）
    * Formatter：指向具体的转换函数（如“十六进制转字符串函数”）
* 为什么用 `NonNull` 指针而不是 `&[Argument]`？
    * 规避安全性检查：`&[T]`本身是一个“胖指针”（包含地址和长度,各占8字节,而`NonNull<T>`：这是一个“瘦指针”，在内存中只占 8 字节）。而编译器为了极致精简，选择只存一个地址，而把长度信息隐藏在 template 的元数据里。
    * 手动布局：这允许编译器在栈上以一种非标准的方式排列这些参数，从而优化CPU 缓存命中率。




**args: `NonNull<rt::Argument<'a>>` 这里为什么代表是指向一个数组的指针?**  

这是一个指向单个对象的指针,但是在逻辑上它指向的是一个数组

这是C 风格编程与 Rust 编译器之间的“默契约定”

* 内存中的连续性 (The Continuous Layout)
当你在代码里写 writeln!(console, "{}", a, b, c) 时：  
1. 编译器（rustc）知道你有 3 个参数。
2. 它会在栈上分配一块连续的内存，大小恰好是 3 * `size_of::<rt::Argument>()`
3. 它把这 3 个 Argument 结构体排排坐，一个接一个地填进去

* 指针的“多重身份”
在底层（C 或 Rust指针）中，“指向单个对象的指针”和“指向数组第一个元素的指针”在二进制层面是完全一样的。 它们都只是一个起始内存地址  
  * 编译器的视角：我只需要把这块连续内存的起始地址（第一个参数的位置）存进Arguments.args 字段里
  * 运行时的视角：当我需要处理第 $N$ 个参数时，我拿这个起始地址，向后偏移 `$N\times$ `结构体大小，就能找到它

* 关键来了,为啥不用`&[rt::Argument]`(切片)来表示这个数组?
  * 标准写法 `&[T]`：这是一个“胖指针”，在内存中占 16 字节（8 字节地址 + 8字节长度）
  * 源码写法 `NonNull<T>`：这是一个“瘦指针”，在内存中只占 8 字节
    * 为什么要省这 8 字节？
      *  栈优化：Arguments 对象经常被放在寄存器里传递。8 字节刚好能塞进一个 64位寄存器（如 RAX），而 16 字节就必须拆分或压栈，增加了开销
      *  长度去哪了？：编译器把参数的数量信息（数组长度）编码进了 template字段所指向的元数据里
         *  引擎先读 template：“哦，剧本说这里有 3 个演员”
         *  引擎再去读 args：“好，那我就从这个地址开始，往后读 3 个位置”

**这种设计的红队意义 (OpSec Context)**
这种“地址与长度分离存储”的技术，也是恶意软件隐藏数据的一种高级手段。
* 混淆分析：如果分析师只看 args 指针，他无法通过静态分析得知这个数组有多大。
* 内存布局欺骗：它打破了常规的反编译器（如IDA）对“标准数组”的识别模式，让逆向分析变得更琐碎