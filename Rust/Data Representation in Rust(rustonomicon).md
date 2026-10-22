1. [Data Representation in Rust](#data-representation-in-rust)
   1. [repr(Rust)](#reprrust)



# Data Representation in Rust

资料：  
https://doc.rust-lang.org/nomicon/data.html


## repr(Rust)

All types have an alignment specified对齐方式 in bytes   
The alignment of a type specifies what addresses are valid to store the value at  
A value with alignment n must only be stored at an address that is a multiple of n   
So alignment 2 means you must be stored at an even偶数 address, and 1 means that you can be stored anywhere. Alignment is at least 1, and always a power指数 of 2.

Primitives are usually aligned to their size, although this is platform-specific behavior. For example, on x86 u64 and f64 are often aligned to 4 bytes (32 bits).

A type's size must always be a multiple of its alignment (Zero being a valid size for any alignment). This ensures that an array of that type may always be indexed by offsetting by a multiple of its size.  
Note that the size and alignment of a type may not be known statically in the case of dynamically sized types.

Rust gives you the following ways to lay out布局 composite data:  
1. structs (named product types)，命名的乘积类型
2. tuples (anonymous product types)
3. arrays (homogeneous product types)，同构的乘积类型
4. enums (named sum types -- tagged unions)，命名的和类型--命名的联合体
5. unions (untagged unions)

product types乘积类型（struct、tuple、arrays）：  
表示其内部字段是且的关系，即需要同时拥有所有字段。这种类型的总状态数是其内部所有字段各自状态数的乘积

sum types和类型（enum、union（只用于和c交互，FFI））：  
其内部字段是和的关系，这种类型的状态数是其内部每个字段状态数的和。  
enum，只能是内部字段中的一种状态；An enum is said to be field-less if none of its variants have associated data.

**By default, composite structures have an alignment equal to the maximum of their fields' alignments.**  

Rust will consequently必要地 insert padding where necessary to ensure that all fields are properly aligned and that the overall type's size is a multiple of its alignment.
```rust
struct A {
    a: u8,
    b: u32,
    c: u16,
}
//become
struct A {
    a: u8,
    _pad1: [u8; 3], // to align `b`
    b: u32,
    c: u16,
    _pad2: [u8; 2], // to make overall size multiple of 4
}
//or maybe
struct A {
    b: u32,
    c: u16,
    a: u8,
    _pad: u8,
}

```
There is no indirection间接性/间接的东西 for these types; all data is stored within the struct, as you would expect in C.  
默认情况下，strcut会将其所有字段的数据直接、连续的存储在为该结构体分配的内存块中，中间没有任何隐藏的指针。这是c/c++/rust的内存布局哲学，也是能够实现高性能的原因。  

“Indirection (间接性)” 是什么？  
“Indirection” 在编程中通常指“通过指针或引用来访问数据”。它意味着为了获取数据，你需要进行至少一次“跳转”。  
* 直接访问：CPU 直接去内存地址 A 读取数据。  
* 间接访问 (Indirection)：CPU 先去内存地址 A，读取到那里存储的另一个地址B（这就是指针），然后再去地址B读取最终的数据。这个过程多了一次“跳转”，因此被称为“间接”。

Rust 中的例外：当你明确需要间接性时  
Rust 的美妙之处在于，它让你自己选择是否需要间接性。虽然默认没有，但你可以通过智能指针来显式地创建它。最常见的例子就是 `Box<T>`。
```rust
struct LargeData {
   // 假设这是一个非常大的数据数组
   data: [u8; 1_000_000],
}

// 默认情况：没有间接性
// large_struct 的大小超过 1MB，它会被直接存储在栈上，可能导致栈溢出。
// let large_struct = LargeData { data: [0; 1_000_000] };

// 明确使用间接性
struct SmartStruct {
    // data 字段现在是一个指向堆内存的指针
    data: Box<[u8; 1_000_000]>,
}

// 创建实例
let smart_struct = SmartStruct { data: Box::new([0; 1_000_000]) };
```


在这个 SmartStruct 的例子中：
* smart_struct 本身的大小非常小，因为它只在栈上存储了一个Box（它本质上是一个指针）。
* 那 1MB 的巨大数组数据则被分配在堆上。smart_struct.data 就包含了间接性：你需要通过 Box指针才能访问到堆上的实际数组数据。总结
  
所以，"There is no indirection... all data is stored within the struct"  这句话的含义是：Rust 在设计上追求极致的性能和内存控制，默认采用和 C 语言一样的数据布局方式，将数据成员直接嵌入结构体，避免了不必要的指针跳转。这种设计使得数据在内存中是紧凑和连续的，极大地提高了缓存命中率和访问速度。同时，Rust 也提供了像 `Box<T>` 这样的工具，允许程序员在需要时（如处理大对象或递归类型）显式地选择使用间接性。      















































































































