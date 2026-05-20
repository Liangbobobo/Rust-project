# util

## end.saturating_sub(start)

函数原型:`pub const fn saturating_sub(self, rhs: Self) -> Self`

Saturating integer subtraction. Computes self - rhs, saturating at the numeric bounds instead of overflowing.

普通的 C 或 Rust 中，通常使用 - 操作符执行减法.但在RedOps中,这么做有着巨大的风险  
1. 在极端情况下(如 PE被恶意破坏\内存被篡改\解析了畸形的.pdata),可能导致end小于start
    * Debug下,rust会触发panic
    * release下,会出现整数溢出Integer Wrap-around,u64会从0翻转回一个巨大的正数
    * 此后在from_raw_parts(start, size)会视图映射几千G的内存,触发Access Violation(C0000005)异常.导致payload暴露

2. saturating_sub执行的是饱和式减法,如果end小于start,硬件/逻辑不会报错,而是强行将运算结果饱和到该类型的最小值(源码中是u64的最小值0)
3. project role:在后续的from_raw_parts(start, size)中,如果size为0,说明.pdata是无效/畸形的,from_raw_parts(start, size)会生成长度为0的空切片.后面的memmem::find会返回none.这种情况,代码会静默跳过这个错误的函数条目.这种零分支成本的错误处理(不要判断 if end< start),实现了无感防御,不仅使得生成的汇编代码简洁/平滑,更减少了指纹
4. 在RedOp中只要涉及RVA转VA的情况,都应遵循这种原则,使用这种饱和算术原则


## let addr = (start as *mut u8).wrapping_add(pos)

**wrapping_add()指针类型内置的算术方法**  
1. 普通的ptr.add()方法有前置条件,编译器和运行时要求:计算结果生成的指针必须落在这个对象被分配的内存范围内.如果pos导致指针超出函数的物理边界,调用add可能会触发未定义行为(UB)
2. 对于wrapping_add():仅执行寄存器级别的加法;它完全不检查结果指针是否合法\越界;即使结果发生了地址溢出(回绕到0),也会执行
3. 在RedOps中,在解析DLL这种非Rust管理的内存时,不能信任Rust的标准边界规则.只需要cpu执行ADD RAX,RBX汇编指令.wrapping_add()提供了这种最接近硬件\最少逻辑干扰的加法实现
4. Rust 的安全检查（如 ptr.add() 的内部逻辑）是建立在 “所有权（Ownership）”基础上的:在rust中分配一个vec或数组,编译器完全掌握这块内存的边界,如果add()越界了,编译器知道在非法操作;但在uwd中kernelbase.dll的内存是windows加载器(ldr)映射到进程空间的,不是rust分配的
    * 对rustc,它并不真正拥有kernelbase.dll的内存块.如果使用add()rust会尝试根据其内置的指针溯源规则来验证这个偏移是否合法,但由于内存是外部注入的,这种验证在复杂的内存布局下可能误判,甚至导致rustc在优化阶段产生非预期的行为.
    * 对wrapping_add(),显示告诉rustc,不检查这块内存所有权/是否合法,只需要做加法,生成地址的安全性有自己负责
5. cpu寄存器层面,地址运算本质上是加减法.当执行ADD RAX,RBX时,硬件不会检查边界/所有权,只管把bit位相加;对uwd项目,需要伪造win的内核行为,而在win内核中执行的就是这种原始的回绕的加法.即为了让payload在二进制中趋近原生系统组件,需要这种原始的/最少语言层面干扰的操作
6. 在uwd中通过`let pos = memchr::memmem::find(bytes, pattern)?;`保证pos一定在.text段中


## let mut seed = unsafe { core::arch::x86_64::_rdtsc() }

**core::arch::x86_64::_rdtsc()**  
1. _rdtsc():读取cpu时间戳计数器Time_Stamp Counter.返回自cpu上次重置以来的时钟周期数,这是一个精度极高\随时间不断变化的数值.在RedOps中,使用硬件指令获取随机源,比调用SystemTime/GetTickCount根隐蔽,且不需链接任何外部库

## Fisher-Yates洗牌算法

该算法目的:让数组中每个元素,都有相等的概率出现在任何一个位置

