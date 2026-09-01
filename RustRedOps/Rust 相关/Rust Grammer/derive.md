

# derive

`#[derive(...)]`属于内置的属性过程宏（Attribute Procedural Macro）。编译器在解析抽象语法树（AST）时，会基于被标记类型的字段结构和内存布局，自动展开并生成相应Trait 的具体实现代码

## `#[derive(Debug)]`

Debug  ( core::fmt::Debug )

1. 底层机制：自动生成fmt方法的实现，允许在上下文中通过{:?}（单行格式化）和  {:#?}（结构化打印）占位符输出该类型的内部状态
2. 前置约束：该数据结构内的所有成员字段都必须已实现  Debug  Trait
3. 语义定位：专用于面向开发者/内部程序的调试检查、日志记录或断言回溯（Panic unwinding），不具有面向终端用户的格式化语义

## `#[derive(Clone)]`

Clone  ( core::clone::Clone )

1. 底层机制：生成  clone(&self) -> Self  方法的实现，赋予数据结构显式复制的能力
2. 前置约束：数据结构内的所有成员字段都必须已实现  Clone  Trait
3. 语义定位：衍生实现会依次对类型内涵的每个字段递归调用  .clone()方法，构造出完整的新实例。其运行时开销与数据结构的嵌套层级及动态分配情况直接相关
4. Clone  完全不干预编译器语义,而是一种显示调用
5. 需要显示调用.clone()


## `#[derive(Copy)]`

Copy  ( core::marker::Copy )

1. 底层机制：属于无方法体的标记特征（Marker Trait）。它的存在会直接改变编译器在词法作用域中的所有权（Ownership）转移语义
2. 前置约束：必须同时实现  Clone  作为 Supertrait;类型内部的任何字段均不可实现  Drop Trait（即不能包含堆分配、文件描述符等涉及自定义析构逻辑的类型）
3. 语义定位：将该类型的传递语义从“所有权移动（Move）”更改为“逐字节内存复制.当进行变量重绑定或作为函数参数传递时，编译器会在栈上执行浅拷贝，原内存地址的实例依然合法且未失效
4. 即Copy  改变了编译器的 Move 语义（隐式拦截）,禁止触发所有权转移（Move）
5. 编译器会自动在栈内存中执行隐式的逐字节复制（Bitwise Copy），操作完成后，原绑定的内存状态依然完全合法可用

## `#[derive(PartiaEq)]`

PartialEq  ( core::cmp::PartialEq )



1. 底层机制和作用：自动生成  eq （及默认提供的  ne ）方法，为该类型提供支持  ==  和  != 运算符的重载解析.它为数据结构自动生成  fn eq  和  fn ne方法的底层逻辑，使得该类型可以通过  ==  和  !=  运算符进行基于控制流的分支判断
2.  Rust 的相等性比较严格基于离散数学中的等价关系（Equivalence Relation）。一个严格的完全等价关系必须满足三个公理：Symmetry对称性( a == b ，则必然  b == a)\Transitivity传递性( a == b  且  b == c ，则必然  a == c)\Reflexivity自反性(任何合法的内存状态  a ，必然有  a == a)
3. PartialEq  仅保证对称性和传递性：它是一个部分等价关系（Partial Equivalence）。在 IEEE 754 浮点数标准中， NaN （Not a Number）在物理内存中存在合法的位模式（Bit Pattern），但逻辑上  NaN != NaN 。因此， f32  和  f64  破坏了自反性，只能实现  PartialEq



## `#[derive(Eq)]`

Eq  ( core::cmp::Eq )

1. Eq  不包含任何方法和关联类型。它在编译时作为一种**静态断言（Static Assertion）**存在。其特征边界  `PartialEq<Self>`  显式剥夺了跨类型比较的可能
2. Eq  强制补全自反性：它向编译器断言该类型的所有合法内存状态都绝对满足完全等价关系（即补全了自反性，保证任何情况下 a == a  必然为真）