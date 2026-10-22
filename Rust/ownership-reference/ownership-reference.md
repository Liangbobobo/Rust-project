- [**Ownership**](#ownership)
  - [stack and heap](#stack-and-heap)
  - [Ownership Rules](#ownership-rules)
  - [string literal](#string-literal)
  - [The String Type](#the-string-type)
  - [Ownership and Functions](#ownership-and-functions)
  - [References and Borrowing](#references-and-borrowing)
  - [The Rules of References](#the-rules-of-references)
  - [The Slice Type](#the-slice-type)
    - [Other Slices](#other-slices)
  - [**Ownership**](#ownership-1)
    - [Reference](#reference)
    - [Aliasing](#aliasing)
    - [Why Aliasing Matters](#why-aliasing-matters)
  - [**Lifetimes**](#lifetimes)
    - [Example: references that outlive referents](#example-references-that-outlive-referents)
    - [Example: aliasing a mutable reference](#example-aliasing-a-mutable-reference)
    - [The area covered by a lifetime](#the-area-covered-by-a-lifetime)
    - [Limits of Lifetimes](#limits-of-lifetimes)
      - [Improperly不恰当的 reduced borrows](#improperly不恰当的-reduced-borrows)
- [reborrow](#reborrow)
  - [什么是reborrow](#什么是reborrow)
    - [Subtypeing and variance](#subtypeing-and-variance)
- [Type conversion](#type-conversion)
  - [**Common Rust Lifetime Misconceptions**](#common-rust-lifetime-misconceptions)
    - [The Misconceptions](#the-misconceptions)
      - [T only contains owned types](#t-only-contains-owned-types)
      - [if T: 'static then T must be valid for the entire program?](#if-t-static-then-t-must-be-valid-for-the-entire-program)
      - [\&'a T and T: 'a are the same thing](#a-t-and-t-a-are-the-same-thing)
      - [my code isn't generic and doesn't have lifetimes](#my-code-isnt-generic-and-doesnt-have-lifetimes)
      - [if it compiles then my lifetime annotations are correct](#if-it-compiles-then-my-lifetime-annotations-are-correct)
    - [Iterator trait设计中避免这种情况的方法、使用reminder lifetime的不同（很重要）](#iterator-trait设计中避免这种情况的方法使用reminder-lifetime的不同很重要)
  - [rebroswwing](#rebroswwing)
- [reference](#reference-1)
  - [The dereference operator](#the-dereference-operator)
  - [reference's coercions](#references-coercions)
    - [Type coercions](#type-coercions)
      - [Coercion sites](#coercion-sites)
    - [Deref Coercion](#deref-coercion)



# **Ownership**


Rust's ownership 与 lays data out in memory.紧密相关

memory is managed through a system of ownership with a set of rules that the compiler checks. If any of the rules are violated, the program won’t compile. None of the features of ownership will slow down your program while it’s running.

## stack and heap

来源：The Rust Programming Language

In a systems programming language like Rust, whether a value is on the stack or the heap affects how the language behaves and why you have to make certain decisions.  
Both the stack and the heap are parts of memory available to your code to use **at runtime**, but they are structured in different ways.   
All data stored on the stack **must have a known, fixed size**. Data with an unknown size at compile time or a size that might change must be stored on the heap instead.  
when you put data on the heap, you request a certain amount of space. The memory allocator finds an empty spot in the heap that is big enough, marks it as being in use, and returns a pointer, which is **the address of that location**.This process is called allocating on the heap and is sometimes abbreviated as just allocating (pushing values onto the stack is not considered allocating，为什么？它省略了“分配”中最核心、最昂贵的部分——在空闲池中寻找可用内存的决策过程).     
Because the pointer to the heap is a known, fixed size, you can store the pointer on the stack, but when you want the actual data, you must follow the pointer.

Pushing to the stack is faster than allocating on the heap because the allocator never has to search for a place to store new data; that **location is always at the top of the stack**. Comparatively, allocating space on the heap requires more work because the allocator must first find a big enough space to hold the data and then perform bookkeeping to prepare for the next allocation.  

Accessing data in the heap is generally slower than accessing data on the stack because you have to follow a pointer to get there. **Contemporary processors现代处理器 are faster if they jump around less in memory.**    
a processor can usually do its job better if it works on data that’s close to other data (as it is on the stack) rather than farther away (as it can be on the heap).

When your code calls a function, the values passed into the function (including, potentially, pointers to data on the heap) and the function’s local variables get pushed onto the stack. When the function is over, those values get popped off the stack.  
Keeping track of what parts of code are using what data on the heap, minimizing the amount of duplicate复制 data on the heap, and cleaning up unused data on the heap so you don’t run out of space耗尽空间/内存 are all problems that ownership addresses. Once you understand ownership, you won’t need to think about the stack and the heap very often, but knowing that the main purpose of ownership is to manage heap data can help explain why it works the way it does.
## Ownership Rules

1. Each value in Rust has an owner.
2. There can only be one owner at a time.
3. When the owner goes out of scope, the value will be dropped.
## string literal

`let s = "hello";`  
如上，s是一个字符串切片，类型是&str，它是一个指向只读内存的胖指针（指针 + 长度），它具有静态生命周期（&'static str），是不可变的，是 Rust 中处理文本数据的主要借用形式，常用于函数参数。

s本身存储在栈上，字符串字面量 "hello" 是硬编码在程序代码中的（hardcoded into the text of our program）。当你编译程序时，这个字符串的字节数据（h, e, l, l, o）会被直接嵌入到最终生成的二进制可执行文件中一个叫做只读数据段（或称为静态存储区）的特殊区域。当程序运行时，操作系统会将这个只读数据段加载到内存中，并保护起来防止修改。
```
+-------------------+
|     栈 (Stack)     |  ← 栈指针向下增长
+-------------------+
| ...               |
| 变量 s:            | ← `s` 本身在这里
|   - ptr: 0x1024   |    (一个16字节的胖指针，
|   - len: 5        |     包含地址和长度)
| ...               |
+-------------------+
|     堆 (Heap)      |  ← 动态分配的内存，向上涨
+-------------------+
| ...               |
|                   |  ← "hello" 的数据**不在这里**
| ...               |
+-------------------+
| 只读数据段 (.rodata) | ← 程序二进制文件的一部分
+-------------------+
| ...               |
| 地址 0x1024: 'h'  | ← 字符串数据 "hello" 存储在这里！
|        0x1025: 'e' |
|        0x1026: 'l' |
|        0x1027: 'l' |
|        0x1028: 'o' |
| ...               |
+-------------------+
| 代码段 (.text)      | ← 程序的机器指令
+-------------------+
```

## The String Type

String的引入解决了string literal的以下缺点：  
1. string literal are  immutable
2. not every string value can be known when we write our code: for example, if we want to take user input and store it

String manages data allocated on the heap and as such is able to store an amount of text that **is unknown at compile time**.

when s goes out of scope. When a variable goes out of scope, Rust calls a special function for us. This function is called drop, and it’s where the author of String can put the code to return the memory. Rust calls drop automatically at the closing curly bracket结束的大括号处.

Stack-Only Data copy 移动时执行    
Rust has a special annotation called the Copy trait that we can place on types that are stored on the stack, If a type implements the Copy trait, variables that use it do not move, but rather are trivially copied, making them still valid after assignment to another variable.  
Rust won’t let us annotate a type with Copy if the type, or any of its parts, has implemented the Drop trait. 

what types implement the Copy trait?   
You can check the documentation for the given type to be sure, but as a general rule, any group of **simple scalar values** can implement Copy, and **nothing that requires allocation or is some form of resource can implement Copy**. Here are some of the types that implement Copy:  
1. scalar including integer、bool、char、float
2. tuple， if they only contain types that also implement Copy
## Ownership and Functions

The mechanics of passing a value to a function are similar to those when assigning a value to a variable.Passing a variable to a function will move or copy, just as assignment does. 

Returning values can also transfer ownership.
## References and Borrowing

A reference is like a pointer in that it’s an address we can follow to access the data stored at that address; that data is owned by some other variable.so reference doesn't change the ownership   

`
fn no_dangle() -> String {
    let s = String::from("hello");

    s
}
`
此例中，返回s避免dangling reference，返回&s会发生。虽然此时返回的是s，但heap上的数据不会被复制，不发生深度copy，所有权被转移到调用者。
## The Rules of References

1. At any given time, you can have either one mutable reference or any number of immutable references.
2. References must always be valid.

## The Slice Type

Slices let you reference a contiguous sequence of elements in a collection. A slice is a kind of reference, so it does not have ownership.

### Other Slices

slice可应用于任何连续存储的数据序列，而不仅仅是字符串，不仅可用于string literal，也可用于数组等

Slice 必须保证其引用的所有数据都是同一类型。

slice有两种形式，&[]、&mut [],&[] 是不可变地借用（view）一片数据，而 &mut [] 是可变地借用（view）一片数据。这里的“不可变”和“可变”指的是能否通过这个“视图”来修改它背后的数据，而不是指这个“视图”变量本身能否被重新赋值。  
&mut[],并不意味着 slice 这个概念是“可变的”，而是提供了一种对连续、同质数据块进行安全、可控的批量修改的强大工具，同时严格遵守 Rust 的“共享不可变，可变不共享”的内存安全原则。
## **Ownership**

Source:https://doc.rust-lang.org/nomicon/ownership.html
```rust
#![allow(unused)]
fn main() {
fn as_str(data: &u32) -> &str {
    // compute the string
    let s = format!("{}", data);

    // OH NO! We returned a reference to something that
    // exists only in this function!
    // Dangling pointer! Use after free! Alas!
    // (this does not compile in Rust)
    &s
}
}
```
This is exactly what Rust's ownership system was built to solve.   
Rust knows the scope in which the &s lives, and as such can prevent it from escaping.

Things get more complicated as code gets bigger and pointers get fed through various functions.
```rust
#![allow(unused)]
fn main() {
let mut data = vec![1, 2, 3];
// get an internal reference
let x = &data[0];

// OH NO! `push` causes the backing storage of `data` to be reallocated.
// Dangling pointer! Use after free! Alas!
// (this does not compile in Rust)
data.push(4);

println!("{}", x);
}
```
naive scope analysis would be insufficient to prevent this bug, because data does in fact live as long as we needed. **However it was changed while we had a reference into it. This is why Rust requires any references to freeze the referent and its owners.**
### Reference

There are two kinds of references:
1. Shared reference: &
2. Mutable reference: &mut

The whole model references obey the following rules:
1. A reference cannot outlive its referent
2. A mutable reference cannot be aliased
### Aliasing

First off, let's get some important caveats out of the way:

1. We will be using the broadest possible definition of aliasing for the sake of discussion.Rust's definition will probably be more restricted to factor in mutations and liveness.
2. We will be assuming a single-threaded, interrupt-free, execution. We will also be ignoring things like memory-mapped hardware. Rust assumes these things don't happen unless you tell it otherwise.

With above said, here's our working definition: variables and pointers alias if they refer to overlapping regions of memory.
### Why Aliasing Matters
```rust
#![allow(unused)]
fn main() {
fn compute(input: &u32, output: &mut u32) {
    let cached_input = *input; // keep `*input` in a register
    if cached_input > 10 {
        // If the input is greater than 10, the previous code would set the output to 1 and then double it,
        // resulting in an output of 2 (because `>10` implies `>5`).
        // Here, we avoid the double assignment and just set it directly to 2.
        *output = 2;
    } else if cached_input > 5 {
        *output *= 2;
    }
}
}

//
```
In Rust, this optimization should be sound. For almost any other language, it wouldn't be (barring global analysis). **This is because the optimization relies on knowing that aliasing doesn't occur**, which most languages are fairly liberal自由 with.   
Specifically, we need to worry about function arguments that make input and output overlap, such as compute(&x, &mut x).
With that input, we could get this execution:
```rust
                    //  input ==  output == 0xabad1dea
                    // *input == *output == 20
if *input > 10 {    // true  (*input == 20)
    *output = 1;    // also overwrites *input, because they are the same
}
if *input > 5 {     // false (*input == 1)
    *output *= 2;
}
                    // *input == *output == 1

//explain

 1 // 假设 input 和 output 都指向地址 0xabad1dea
    2 // 该地址上的值是 20
    3 // *input == *output == 20
    4
    5 if *input > 10 {    // 条件为真 (20 > 10)
    6     *output = 1;    // 关键点：因为 output 和 input 指向同一地址，
    7                     // 所以这行代码不仅修改了 *output，也同时修改了 *input 的值。
    8                     // 执行后, *input 和 *output 都变成了 1。
    9 }
   10 if *input > 5 {     // 条件为假 (因为 *input 现在是 1, 1 > 5 不成立)
   11     *output *= 2;   // 这行代码被跳过。
   12 }
   13
   14 // 最终结果: *input == *output == 1
```
这里的“重叠”（overlap）或“别名”（alias）指的是两个或多个引用（references）指向了同一块内存地址。在例子compute(&x, &mut x) 中，input 参数（一个不可变引用 &i32）和 output 参数（一个可变引用 &muti32）都指向了同一个变量 x  
如果你在 Rust 中尝试编译这段代码：
```rust
 1 fn compute(input: &i32, output: &mut i32) {
    2     if *input > 10 {
    3         *output = 1;
    4     }
    5     if *input > 5 {
    6         *output *= 2;
    7     }
    8 }
    9
   10 fn main() {
   11     let mut x = 20;
   12     // 尝试同时创建不可变和可变借用
   13     compute(&x, &mut x);
   14 }

  编译器会立即报错：

   1 error[E0502]: cannot borrow `x` as mutable because it is also borrowed as immutable
   2   --> src/main.rs:14:18
   3    |
   4 14 |     compute(&x, &mut x);
   5    |     ---------^^-------
   6    |     |        |
   7    |     |        mutable borrow occurs here
   8    |     immutable borrow occurs here
   9    |     immutable borrow later used here
```
we used the fact that &mut u32 can't be aliased to prove that writes to *output can't possibly affect *input. This lets us cache *input in a register寄存器, eliminating a read.  
By caching this read, we knew that the write in the > 10 branch couldn't affect whether we take the > 5 branch, allowing us to also eliminate a read-modify-write (doubling *output) when *input > 10.  

我们利用 Rust 的借用规则（&mut 的独占性），向编译器证明了 input 和 output不可能指向同一块内存。因此，写 output 不会影响 input。  
：这个“证明”给了编译器优化的信心。它可以在第一次读取 *input 后，就放心地把这个值存入一个 CPU寄存器（这个行为就是“缓存”）。在后续所有需要读取 *input的地方，都直接使用这个寄存器里的值，而无需一次次地去访问内存。这个过程就“消除”了后续所有对 *input的内存读取操作（eliminating a read）。
我们已经从上一个例子中得知，由于 Rust 的别名规则，编译器可以安全地将 *input 的初始值（例如20）缓存到一个寄存器中。
* 这意味着，后续的两个 if 判断 (if *input > 10 和 if *input > 5)都会使用这个未经改变的、缓存好的初始值（20）来进行决策。

在没有优化的版本中，> 10 分支里的 *output = 1 操作，因为别名关系，会污染 *input的值，从而直接影响到第二个 if 判断的结果。  
但是现在，第二个 if 判断的依据是寄存器里的初始值 20，而不是内存中可能被污染的值。因此，编译器可以得出结论：> 10 分支中的任何操作，都无法影响 `> 5`分支的决策。两个分支的“是否进入”这个决策在逻辑上已经解耦了  

*input 的初始值大于 10 时（例如 20），编译器现在可以同时预知两个 if 的结果：  
20 > 10 为真，所以第一个分支必定会执行。20 > 5 为真，所以第二个分支也必定会执行。

编译器现在面对的是一个确定的操作序列：  
*output = 1;  
*output *= 2;  
这里的 *output *= 2 就是一个典型的 “读取-修改-写入”（Read-Modify-Write, RMW） 操作。CPU 执行它需要三个步骤：  
1. 读取（Read）：从内存中加载 *output 的当前值。
2. 修改（Modify）：在 CPU 内部将该值乘以 2。
3. 写入（Write）：将计算出的新值写回内存中的 *output。
这是一个相对昂贵的操作。但是，编译器现在可以做得更聪明。既然它知道这两个操作会连续发生，它可以在编译时就计算出最终结果：
* “*output 先被设为 1，然后它会变成 1 * 2，也就是 2。”因此，编译器可以将这两个操作合并成一个单一的、简单的写入操作：
* *output = 2;

这个合并过程，就成功地消除了原来那个昂贵的“读取-修改-写入”操作。这就是这段话所描述的、由别名分析带来的深层优化。





以上：
1. 问题：函数参数的别名（输入和输出指向同一内存）会产生违反直觉的程序行为，并阻碍编译器进行有效的优化。
2. 原因：对一个引用的写入可能会意外地改变通过另一个引用读取的值。
3. Rust 的对策：通过其所有权和借用系统，在编译时就静态地保证了可变引用 (&mut T)在其生命周期内是独占的，绝不会与其他任何对同一数据的引用共存。
4.  优势：这种编译时的保证，为编译器提供了一个强大的“不存在别名”的承诺。这使得 Rust 编译器可以安全地进行一系列在C/C++ 中通常不安全的激进优化，从而在不牺牲内存安全的前提下实现极高的性能。这就是 Rust “无畏并发”（Fearless Concurrency）和高性能的基石之一。


**This is why alias analysis is important: it lets the compiler perform useful optimizations!** Some examples:
1. keeping values in registers by proving no pointers access the value's memory
2. eliminating reads by proving some memory hasn't been written to since last we read it
3. eliminating writes by proving some memory is never read before the next write to it
4. moving or reordering reads and writes by proving they don't depend on each other

上述主要是通过alias的一些规则，对register进行的优化，其原理如下：  
- 别名分析（Alias Analysis）：这是编译器的一项技术，用于判断在程序中不同的指针或引用是否可能指向（或“别名”）同一块内存地址。分析的结果通常是“可能别名”（May Alias）或“绝不别名”（No Alias）。
- 核心思想：如果编译器能通过分析证明两个指针“绝不别名”，它就获得了极大的优化自由度，因为对一个指针的操作不会意外影响到另一个。Rust 的借用规则为编译器提供了进行这种证明的强大武器。

---

- 优化点 1：keeping values in registers by proving no pointers access the value's memory
  - 中文翻译：通过证明没有其他指针会访问某个值的内存，从而将该值一直保留在寄存器中。
  - 讲解：这正是我们之前详细讨论过的“寄存器缓存”优化。
    - 场景：程序从内存中读取一个值，稍后需要再次使用它。
    - 无别名证明（No Alias）：编译器通过别名分析，证明在第一次读取和第二次使用之间，没有任何其他指针会写入这块内存。
    - 优化：编译器可以安全地在第一次读取后，就将这个值存入一个超高速的 CPU 寄存器。当再次需要这个值时，直接从寄存器中取用，而无需再次访问缓慢的内存。
  - 例子：
```rust
fn example(a: &i32, b: &mut i32) {
    let val = *a; // 1. 读取 *a 并存入寄存器
    *b = 10;      // 2. 写入 *b。编译器知道这不会影响 *a
    println!("{}", val); // 3. 直接使用寄存器中的 val，无需重读 *a
}
```

---

- 优化点 2：eliminating reads by proving some memory hasn't been written to since last we read it
  - 中文翻译：通过证明某块内存在上次读取后没有被写入过，从而消除（后续的）读取操作。
  - 讲解：这一点与第 1 点密切相关，但侧重点在于消除冗余的读取。
    - 场景：程序连续两次从同一个内存地址读取数据，中间有一些其他操作。
    - 无别名证明（No Alias）：编译器证明在两次读取之间，没有任何写入操作会触及这个内存地址。
    - 优化：编译器可以断定，第二次读取的结果必然与第一次完全相同。因此，第二次读取操作是多余的，可以直接被消除，程序只需使用第一次读取的结果即可。
  - 例子：
```rust
fn example(p: &i32, q: &mut i32) {
    let x = *p; // 第一次读
    *q = 123;   // 写入一个不相干的地址
    let y = *p; // 第二次读。编译器证明 *p 未变，所以 y 和 x 必然相等
    // 优化后，可能变成: let x = *p; *q = 123; let y = x;
    // 从而消除了第二次 `*p` 的内存读取
}
```

---

- 优化点 3：eliminating writes by proving some memory is never read before the next write to it
  - 中文翻译：通过证明某块内存在下一次写入之前从未被读取过，从而消除（当前的）写入操作。
  - 讲解：这被称为死存储消除（Dead Store Elimination）。
    - 场景：程序向一个内存地址写入了一个值，但这个值还从没被任何人读取过，程序就又向同一个地址写入了一个新值。
    - 无别名证明（No Alias）：编译器需要证明，在两次写入操作之间，没有任何其他可能别名的指针会来读取第一次写入的那个值。
    - 优化：如果证明成功，那么第一次的写入操作就是完全无效的“死代码”，因为它的结果从未被使用。编译器可以直接将其消除，只保留最后一次的写入。
  - 例子：
```rust
fn example(p: &mut i32) {
    *p = 10; // 第一次写。如果没人读，这就是一次“死存储”
    // ... 假设中间没有通过其他指针读取 *p 的操作 ...
    *p = 20; // 第二次写
}
// 优化后，`*p = 10;` 这行代码可能被完全删除。
```

---

- 优化点 4：moving or reordering reads and writes by proving they don't depend on each other
  - 中文翻译：通过证明读写操作之间没有相互依赖，从而移动或重排这些操作。
  - 讲解：这关乎指令重排（Instruction Reordering），对于发挥现代 CPU 的并行处理能力至关重要。
    - 场景：代码中有一系列的读和写操作。
    - 无别名证明（No Alias）：编译器证明操作 A 和操作 B 之间没有数据依赖关系（例如，A 写入的地址和 B 读取的地址不同，反之亦然）。
    - 优化：如果 A 和 B 相互独立，编译器就可以自由地重排它们的执行顺序，以达到更好的指令级并行（Instruction-Level Parallelism）。例如，它可以将多个独立的读操作集中在一起，让 CPU 一次性处理，或者将一个耗时长的操作提前开始，用其他不相关的操作来“填充”等待时间。
  - 例子：
```rust
fn example(p: &mut i32, q: &mut i32) {
    let x = *p; // 读 p
    let y = *q; // 读 q
    *p = y;     // 写 p
    *q = x;     // 写 q
}
// 在 C 语言中，如果 p 和 q 可能指向同一个地址，这些操作必须严格按顺序执行。
// 在 Rust 中，编译器知道 p 和 q 不同。如果它认为先执行两次读取 (`let x = *p; let y = *q;`)
// 再执行两次写入 (`*p = y; *q = x;`) 对 CPU 流水线更有利，它就可以安全地进行这种重排。
```

---

别名分析是编译器进行深度优化的基础。Rust 的所有权和借用系统为别名分析提供了坚实可靠的“无别名”信息，使得这些在其他语言中可能不安全或需要复杂分析的优化，在 Rust 中可以被安全、常规地应用，从而在保证内存安全的同时，实现了卓越的性能。




The key thing to remember about alias analysis is that writes are the primary hazard危害 for optimizations.  
That is, the only thing that prevents us from moving a read to any other part of the program is the possibility of us re-ordering it with a write to the same location.

For instance, we have no concern for aliasing in the following modified version of our function, because we've moved the only write to *output to the very end of our function. This allows us to freely reorder the reads of *input that occur before it:
```rust
#![allow(unused)]
fn main() {
fn compute(input: &u32, output: &mut u32) {
    let mut temp = *output;
    if *input > 10 {
        temp = 1;
    }
    if *input > 5 {
        temp *= 2;
    }
    *output = temp;
}
}
```
We're still relying on alias analysis to assume that input doesn't alias temp, but the proof is much simpler:    
the value of a local variable can't be aliased by things that existed before it was declared. This is an assumption every language freely makes, and so this version of the function could be optimized the way we want in any language.

**This is why the definition of "alias" that Rust will use likely involves some notion of liveness and mutation: we don't actually care if aliasing occurs if there aren't any actual writes to memory happening.**

Of course, a full aliasing model for Rust must also take into consideration things like function calls (which may mutate things we don't see), raw pointers (which have no aliasing requirements on their own), and UnsafeCell (which lets the referent of an & be mutated).
## **Lifetimes**

Rust uses lifetimes to track the relationships between borrows and ownership 

Lifetimes are named regions of code that a reference must be valid for.  
Those regions may be fairly complex, as由于 they correspond to paths of execution in the program.There may even be holes in these paths of execution, as it's possible to invalidate a reference as long as it's reinitialized before it's used again.   
Types which contain references (or pretend to) may also be tagged with lifetimes so that Rust can prevent them from being invalidated as well.

In most of our examples, the lifetimes will coincide with一致 scopes.This is because our examples are simple. The more complex cases where they don't coincide are described below.  
1. Within a function body, Rust generally doesn't let you explicitly name the lifetimes involved. This is because it's generally not really necessary to talk about lifetimes in a local context; Rust has all the information and can work out everything as optimally as possible.   
Many anonymous scopes and temporaries that you would otherwise have to write are often introduced to make your code Just Work.  
However once you cross the function boundary, you need to start talking about lifetimes. Lifetimes are denoted with an apostrophe: 'a, 'static.  
To dip our toes with lifetimes编写rust的生涯一直都在使用生命周期, we're going to pretend假装 that we're actually allowed to label scopes with lifetimes, and desugar the examples from the start of this chapter.

One particularly interesting piece of sugar is that each let statement implicitly introduces a scope. For the most part, this doesn't really matter. However it does matter for variables that refer to each other. As a simple example, let's completely desugar this simple piece of Rust code:
```rust
let x = 0;
let y = &x;
let z = &y;
// The borrow checker always tries to minimize the extent范围/长度 of a lifetime, so it will likely desugar to the following:
// NOTE: `'a: {` and `&'b x` is not valid syntax!
'a: {
    let x: i32 = 0;
    'b: {
        // lifetime used is 'b because that's good enough.
        let y: &'b i32 = &'b x;
        'c: {
            // ditto on 'c
            let z: &'c &'b i32 = &'c y; // "a reference to a reference to an i32" (with lifetimes annotated)
        }
    }
}
```
**Actually passing references to outer scopes will cause Rust to infer a larger lifetime**
```rust
let x = 0;
let z;
let y = &x;
z = y;
//desugar
'a: {
    let x: i32 = 0;
    'b: {
        let z: &'b i32;
        'c: {
            // Must use 'b here because the reference to x is
            // being passed to the scope 'b.
            let y: &'b i32 = &'b x;
            z = y;
        }
    }
}
```
### Example: references that outlive referents

```rust
fn as_str<'a>(data: &'a u32) -> &'a str {
    'b: {
        let s = format!("{}", data);
        return &'a s
    }
}

fn main() {
    'c: {
        let x: u32 = 0;
        'd: {
            // An anonymous scope is introduced because the borrow does not
            // need to last for the whole scope x is valid for. The return
            // of as_str must find a str somewhere before this function
            // call. Obviously not happening.
            println!("{}", as_str::<'d>(&'d x));
        }
    }
}
//the right way to write this function is as follows:
fn to_string(data: &u32) -> String {
    format!("{}", data)
}

```
We must produce an owned value inside the function to return it! The only way we could have returned an &'a str would have been if it was in a field of the &'a u32, which is obviously not the case.

(Actually we could have also just returned a string literal, which as a global can be considered to reside at the bottom of the stack; though this limits our implementation just a bit.)
### Example: aliasing a mutable reference

```rust
let mut data = vec![1, 2, 3];
let x = &data[0];
data.push(4);//注意push的第一个参数是&mut self，引用并不牵扯到所有权问题，因为它实际上是借用，不拥有其指向数据的所有权
println!("{}", x);
//desugars
'a: {
    let mut data: Vec<i32> = vec![1, 2, 3];
    'b: {
        // 'b is as big as we need this borrow to be
        // (just need to get to `println!`)
        let x: &'b i32 = Index::index::<'b>(&'b data, 0);
        'c: {
            // Temporary scope because we don't need the
            // &mut to last any longer.
            Vec::push(&'c mut data, 4);
        }
        println!("{}", x);
    }
}
```
We have a live shared reference x to a descendant of data when we try to take a mutable reference to data to push. This would create an aliased mutable reference, which would violate the second rule of references.

However this is not at all how Rust reasons that this program is bad.Rust doesn't understand that x is a reference to a subpath of data. It doesn't understand Vec at all.    
What it does see is that x has to live for 'b in order to be printed.The signature of Index::index subsequently随后的 demands that the reference we take to data has to survive for 'b.  
When we try to call push, it then sees us try to make an &'c mut data. Rust knows that 'c is contained within 'b, and rejects our program because the &'b data must still be alive!

Here we see that the lifetime system is much more coarse粗糙 than the reference semantics we're actually interested in preserving. For the most part, that's totally ok, because it keeps us from spending all day explaining our program to the compiler. However it does mean that several programs that are totally correct with respect to Rust's true semantics are rejected because lifetimes are too dumb.


>引用并不牵扯到所有权问题，因为它实际上是借用，不拥有其指向数据的所有权.  
>当引用作为参数传递给函数时，当函数执行完毕，该引用是否有效取决于其指向的数据是否还存在，因为引用没有所有权，作为参数传递时不牵扯所有权转移问题
>当一个拥有所有权的参数传给函数时，其所有权会转移，导致源数据不能再使用了
```rust
//证明所有权的转移代码
fn main(){
   println!("Main function has started.");
    let owner = String::from("I live inside the inner scope");
    consum(owner);
    println!("Outside inner scope: {:#?}", owner);

    fn consum(v:String){
        println!("Inside function a: {}", v);
    }
       }

```
### The area covered by a lifetime

**A reference (sometimes called a borrow) is alive from the place it is created to its last use.The borrowed value needs to outlive only borrows that are alive. This looks simple, but there are a few subtleties细节.**   

```rust
#![allow(unused)]
fn main() {
let mut data = vec![1, 2, 3];// data拥有vec![1,2,3]，不存在borrowe，不需要遵守引用的规则
let x = &data[0];
println!("{}", x);//这里x的lifetime结束，在编译期就能分析，不是引用计数那种计数，引用计数有代价，这里仅仅依赖lifetime，没有代价
// This is OK, x is no longer needed
data.push(4);
}
```
注意和上一段代码比较，仅仅更换语句的顺序就能通过编译，其精妙之处就是lifetime的原因

However, if the value has a destructor, the destructor is run at the end of the scope. And running the destructor is considered a use ‒ obviously the last one. So, this will not compile.
```rust
#![allow(unused)]
fn main() {
#[derive(Debug)]
struct X<'a>(&'a i32);

impl Drop for X<'_> {
    fn drop(&mut self) {}
}

let mut data = vec![1, 2, 3];
let x = X(&data[0]);
// 在rust-analyzer中,上行被展开为let x:X<`_>=X(&*&(&data)[0]);
println!("{:?}", x);
data.push(4);
// Here, the destructor is run and therefore this'll fail to compile.
}
```
**展开为```rust let x:X<`_>=X(&*&(&data)[0]);```**  

我们来一步步拆解，看看 X(&data[0]) 是如何变成 X(&*&(&data)[0]) 的。

  这里的核心是理解 自动解引用 (Auto-Deref)、`Index` Trait 的工作方式，以及 值 (Value) 与 位置 (Place)
  的区别。

  ---

  简短总结

  这个复杂的表达式本质上是编译器为了调用 index 方法，并最终得到一个符合 X 构造函数所期望的 &i32
  引用，而执行的一系列自动解引用、自动引用和重借用 (reborrowing) 的完整“日志”。虽然看起来有很多 & 和
  *，但很多步骤是相互抵消的，最终结果就是“获取 data 第0个元素的不可变引用”。

  ---

  详细分解

  让我们从内到外分解 &*&(&data)[0] 这一部分。

  第 1 步: (&data)

   - data 的类型是 Vec<i32>。
   - &data 创建了一个对 Vec 的不可变引用，类型是 &Vec<i32>。
   - 这一步通常是编译器在准备调用方法或操作符时，自动进行的“自动引用”的一部分。

  第 2 步: (&data)[0]

   - 这是整个表达式的核心：索引操作 (Indexing)。
   - 在 Rust 中，[] 语法是 std::ops::Index trait 的语法糖。当你写 receiver[index] 时，编译器会去寻找 receiver
     类型上实现的 index 方法。
   - index 方法的签名是 fn index(&self, index: usize) -> &Self::Output。
   - 编译器为了在 &Vec<i32> 上调用 index 方法，会进行自动解引用 (Auto-Deref)：
       1. &Vec<i32> 首先被解引用成 Vec<i32>。
       2. Vec<i32> 实现了 Deref<Target = [i32]>，所以它又被解引用成 [i32] (切片)。
       3. 编译器在 [i32] 上找到了 index 方法。
   - 所以 (&data)[0] 最终调用了切片的 index 方法。这个方法返回一个 &i32。
   - 关键点：[] 索引语法的结果是一个“位置” (Place)，也就是内存中的一个位置，而不是一个临时的“值”(Value)。所以
     (&data)[0] 的结果是 data 内部第一个元素 1 所在的那个内存位置。

  第 3 步: &(&data)[0]

   - 我们已经知道 (&data)[0] 是一个内存位置。
   - 在它前面加上 &，就是对这个位置进行借用，创建一个指向它的引用。
   - 所以，&(&data)[0] 的结果是一个值，类型是 &i32。

  第 4 步: *&(&data)[0]

   - 我们刚得到一个 &i32 类型的值。
   - 在它前面加上 *，就是对这个引用进行解引用。
   - 对一个引用解引用，会得到它所指向的那个位置。
   - 所以，*&(&data)[0] 的结果又变回了第 2 步的那个内存位置。

  第 5 步: &*&(&data)[0]

   - 我们又回到了那个内存位置。
   - 在它前面再次加上 &，就是对这个位置进行再次借用。
   - 所以，&*&(&data)[0] 的最终结果，还是一个类型为 &i32 的值。

  为什么会这么复杂？&* 的意义是什么？

  你可能会问，第 3、4、5 步中的 &*& 看起来很多余，& 和 * 不是会抵消吗？

  是的，从最终结果来看，&*some_ref 等价于
  some_ref。但编译器展示这个过程，是因为它在严格地遵循类型转换和借用规则。这个 &* 的组合通常被称为 “重借用”
  (Reborrowing)。

   - 从“位置”到“值”: 当需要一个引用（值）时，编译器对一个“位置”执行 & 操作。
   - 从“值”到“位置”: 当需要一个“位置”（例如，为了再次借用它）时，编译器对一个引用（值）执行 * 操作。

  编译器内部的算法在处理泛型和各种复杂情况时，会严格地执行这些转换步骤，以确保所有权和借用规则在任何情况下都
  得到遵守。rust-analyzer 所做的，就是把这个内部的、严谨的、一步不落的转换过程忠实地展示给你。

  结论：

  您看到的 X(&*&(&data)[0]) 是对 X(&data[0]) 最最完整、最底层的展开。它揭示了编译器为了实现一个看似简单的索
  引和借用操作，在幕后进行的包括自动引用、自动解引用、`Index` Trait
  解析、以及在“位置”和“值”之间来回转换的所有细节。

  虽然它看起来很复杂，但您只需要理解其最终目标就是为了安全地获取一个 &i32 类型的引用即可。这种复杂性正是
  Rust 内存安全性的基石。



However, if the value has a destructor, the destructor is run at the end of the scope. And running the destructor is considered a use ‒ obviously the last one. So, this will not compile.
```rust
#![allow(unused)]
fn main() {
#[derive(Debug)]
struct X<'a>(&'a i32);

impl Drop for X<'_> {
    fn drop(&mut self) {}
}

let mut data = vec![1, 2, 3];
let x = X(&data[0]);
println!("{:?}", x);
data.push(4);
// Here, the destructor is run and therefore this'll fail to compile.
}
```
To resolve:
1. One way to convince the compiler that x is no longer valid is by using drop(x) before data.push(4).
2. Furthermore, there might be multiple possible last uses of the borrow, for example in each branch of a condition.
```rust
#![allow(unused)]
fn main() {
fn some_condition() -> bool { true }
let mut data = vec![1, 2, 3];
let x = &data[0];

if some_condition() {
    println!("{}", x); // This is the last use of `x` in this branch
    data.push(4);      // So we can push here
} else {
    // There's no use of `x` in here, so effectively the last use is the
    // creation of x at the top of the example.
    data.push(5);
}

```
**And a lifetime can have a pause in it.** Or you might look at it as two distinct borrows just being tied to the same local variable. This often happens around loops (writing a new value of a variable at the end of the loop and using it for the last time at the top of the next iteration).
```rust
#![allow(unused)]
fn main() {
let mut data = vec![1, 2, 3];
// This mut allows us to change where the reference points to
let mut x = &data[0];
// 这段台精妙了，值得反复思考
println!("{}", x); // Last use of this borrow
data.push(4);
x = &data[3]; //，注意这里直接使用了x， We start a new borrow here
println!("{}", x);
}
```

让我们来分解一下 println!("{}", x); 这一行代码。

   1. `x` 的类型:
      首先，我们确定 x 的类型。let x = &data[3]; 这一行让 x 成为了一个 &i32 类型，即“一个对 i32
  整数的不可变引用”。

   2. `println!` 宏与 `{}` 格式化:
      println! 是一个宏，{} 是一个格式化说明符。当你使用 {} 时，你是在告诉 println!：“请使用
  std::fmt::Display 这个 trait 来将参数格式化为人类可读的字符串。”

   3. 寻找 `Display` Trait 的实现:
      现在，编译器（或更准确地说是宏展开后的格式化机制）需要为传递进来的参数 x 找到一个 Display 的实现。
       - 参数 x 的类型是 &i32。
       - 格式化机制首先检查：&i32 类型本身是否实现了 Display trait？答案是没有。
       - 关键步骤：当直接在引用类型上找不到所需的 trait (如此处的 Display)
         时，格式化机制会自动“向里看”，对引用进行自动解引用
         (auto-dereferencing)，去查看它所指向的那个值的类型。
       - x (&i32) 被解引用后，得到它所指向的值，这个值的类型是 i32。
       - 格式化机制接着检查：i32 类型是否实现了 Display trait？答案是是的！Rust
         的标准库为所有基础整数类型都实现了 Display。

   4. 调用实现:
      既然在 i32 上找到了 Display 的实现，格式化机制就会使用这个实现来将 x
  所指向的那个整数值转换成字符串，并最终打印到控制台。

  总结

  所以，这里的“Deref Coercion”具体表现为：

  `println!` 宏的格式化工具在为参数 `x`（类型 `&i32`）寻找 `Display` trait 的实现时，发现 `&i32`
  本身没有实现，于是自动解引用 `x`，转而寻找其指向的值的类型 `i32` 上的 `Display` 实现，并成功找到了它。

  这个过程可以看作是 Deref Coercion 思想的一种应用，但更精确的描述是“格式化 trait 在进行方法解析（trait
  resolution）时的自动解引用行为”。这使得我们无需手动编写 println!("{}", *x); 这样繁琐的代码，极大地提升了
  Rust 的人体工程学。无论是 &String、&i32 还是 &MyCustomType（如果 MyCustomType 实现了
  Display），我们都可以直接把引用丢进 println!，非常方便。

### Limits of Lifetimes

```rust
#[derive(Debug)]
struct Foo;

impl Foo {
    fn mutate_and_share(&mut self) -> &Self { &*self }
    fn share(&self) {}
}

fn main() {
    let mut foo = Foo;
    let loan = foo.mutate_and_share();
    foo.share();
    println!("{:?}", loan);
}

// desugars
struct Foo;

impl Foo {
    fn mutate_and_share<'a>(&'a mut self) -> &'a Self { &'a *self }
    fn share<'a>(&'a self) {}
}

fn main() {
    'b: {
        let mut foo: Foo = Foo;
        'c: {
            let loan: &'c Foo = Foo::mutate_and_share::<'c>(&'c mut foo);
            'd: {
                Foo::share::<'d>(&'d foo);
            }
            println!("{:?}", loan);
        }
    }
}
```
这里面的```rust fn mutate_and_share(&mut self) -> &Self { &*self }```为什么会扩展为```rust { &*&*self } ```?   
第二种方式明显是冗余的，是rust-analyzer的完全的解释

The lifetime system is forced to extend the &mut foo to have lifetime 'c, due to the lifetime of loan and mutate_and_share's signature. Then when we try to call share, it sees we're trying to alias that &'c mut foo and blows up in our face!

This program is clearly correct according to the reference semantics we actually care about, but the lifetime system is too coarse-grained to handle that.

#### Improperly不恰当的 reduced borrows 



















# reborrow

https://stackoverflow.com/questions/65474162/reborrowing-of-mutable-reference  
https://users.rust-lang.org/t/simple-resources-about-reborrowing-for-beginners/129994  
https://doc.rust-lang.org/stable/reference/type-coercions.html#coercion-types

## 什么是reborrow

主要针对mut &，共享引用比较简单
对于可变引用（也叫独占引用），可以同时定义对同一个值的两个可变引用，但是在使用这两个可变引用时，要以栈的方式使用（即先定义的最后再使用）
```rust
#[allow(unused)]
fn main() { 
let mut x  = "hello";
let  a=&mut x;
//通过类型转换，equal to let  b= &mut x;  这里发生了reborrow
let  b= &mut *a;

*b= "rust";
// 改变顺序将*a放在*b之上，会改变这两个变量的lifetime，会报错。
*a= "world";
}

//这里牵扯到reference中type coercions章节引发的reborrow的概念（未经官方确认）
```
this is mostly about type coercion.  
- &mut T to &T
- &T or &mut T to &U if T implements Deref<Target = U>
- &mut T to &mut U if T implements DerefMut<Target = U>

Notably, &U and &mut U both implement Deref<Target = U>, and &mut U also implements DerefMut<Target = U>. Therefore, the second/third rules lead to the following coercions:
<table class="s-table">
<thead>
<tr>
<th>Let <code>T</code> be:</th>
<th>Coerce from</th>
<th>To</th>
</tr>
</thead>
<tbody>
<tr>
<td><code>&amp;U</code></td>
<td><code>&amp;&amp;U</code></td>
<td><code>&amp;U</code></td>
</tr>
<tr>
<td><code>&amp;U</code></td>
<td><code>&amp;mut &amp;U</code></td>
<td><code>&amp;U</code></td>
</tr>
<tr>
<td><code>&amp;mut U</code></td>
<td><code>&amp;&amp;mut U</code></td>
<td><code>&amp;U</code></td>
</tr>
<tr>
<td><code>&amp;mut U</code></td>
<td><code>&amp;mut &amp;mut U</code></td>
<td><code>&amp;mut U</code></td>
</tr>
</tbody>
</table>

In order to write into a reference, it clearly must be a mutable (aka unique) reference. When you write let a = &mut x, now a is the only way to access x. Now when you write let b = &mut *a, it essentially means let b = &mut *(&mut x), which simplifies to let b = &mut x.  
Here, b mutably borrows a, Rust will not let you use a until b is destroyed... or so it seems for now.

So when you write let b = &mut *a, b doesn't actually borrow a! It only borrows the data a points to. This distinction wasn't relevant before, but here it is what allows the code to compile.

这段在StackOverflow上的分析真的太精妙了，再一次证明了Rust的严密的强类型逻辑，即任何deref都是可推倒的，是逻辑通顺的。


































































### Subtypeing and variance 

Variance is the concept that Rust borrows to define relationships about subtypes through their generic parameters

Subtypeing and variance 是一种关于lifetime和generic parameters的coercion，具体内容在type conversion中



# Type conversion

https://doc.rust-lang.org/nomicon/conversions.html#type-conversions    
https://doc.rust-lang.org/reference/subtyping.html#variance














## **Common Rust Lifetime Misconceptions**
https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program

<table>
<thead>
<tr>
<th>Phrase</th>
<th>Shorthand for</th>
</tr>
</thead>
<tbody>
<tr>
<td><code>T</code></td>
<td>1) a set containing all possible types <em>or</em><br>2) some type within that set</td>
</tr>
<tr>
<td>owned type</td>
<td>some non-reference type, e.g. <code>i32</code>, <code>String</code>, <code>Vec</code>, etc</td>
</tr>
<tr>
<td>1) borrowed type <em>or</em><br>2) ref type</td>
<td>some reference type regardless of mutability, e.g. <code>&amp;i32</code>, <code>&amp;mut i32</code>, etc</td>
</tr>
<tr>
<td>1) mut ref <em>or</em><br>2) exclusive ref</td>
<td>exclusive mutable reference, i.e. <code>&amp;mut T</code></td>
</tr>
<tr>
<td>1) immut ref <em>or</em><br>2) shared ref</td>
<td>shared immutable reference, i.e. <code>&amp;T</code></td>
</tr>
</tbody>
</table>

### The Misconceptions

In a nutshell简要的说:  
A variable's lifetime is how long the data it points to can be statically verified by the compiler to be valid at its current memory address.   
####  T only contains owned types

This misconception is more about generics than lifetimes but generics and lifetimes are tightly intertwined缠绕 in Rust so it's not possible to talk about one without also talking about the other. Anyway:

In my newbie Rust mind this is how I thought generics worked:  
<table>
<thead>
<tr>
<th></th>
<th></th>
<th></th>
<th></th>
</tr>
</thead>
<tbody>
<tr>
<td><strong>Type Variable</strong></td>
<td><code>T</code></td>
<td><code>&amp;T</code></td>
<td><code>&amp;mut T</code></td>
</tr>
<tr>
<td><strong>Examples</strong></td>
<td><code>i32</code></td>
<td><code>&amp;i32</code></td>
<td><code>&amp;mut i32</code></td>
</tr>
</tbody>
</table>

T contains all owned types.  
&T contains all immutably borrowed types.   
&mut T contains all mutably borrowed types.   
T, &T, and &mut T are disjoint finite sets不相交的有限集合.

But，all above are wrong  
This is how generics actually work in Rust:
<table>
<thead>
<tr>
<th></th>
<th></th>
<th></th>
<th></th>
</tr>
</thead>
<tbody>
<tr>
<td><strong>Type Variable</strong></td>
<td><code>T</code></td>
<td><code>&amp;T</code></td>
<td><code>&amp;mut T</code></td>
</tr>
<tr>
<td><strong>Examples</strong></td>
<td><code>i32</code>, <code>&amp;i32</code>, <code>&amp;mut i32</code>, <code>&amp;&amp;i32</code>, <code>&amp;mut &amp;mut i32</code>, ...</td>
<td><code>&amp;i32</code>, <code>&amp;&amp;i32</code>, <code>&amp;&amp;mut i32</code>, ...</td>
<td><code>&amp;mut i32</code>, <code>&amp;mut &amp;mut i32</code>, <code>&amp;mut &amp;i32</code>, ...</td>
</tr>
</tbody>
</table>

T, &T, and &mut T are all infinite sets无限集合, since it's possible to borrow a type ad-infinitum复无限集合  

**T is a superset of both &T and &mut T.**  
**&T and &mut T are disjoint sets.**    
**&T（共享引用）和&mut T（可变引用/独占引用），是两个不同的类型**
```rust
trait Trait {}
impl<T> Trait for T {}
impl<T> Trait for &T {} // ❌
impl<T> Trait for &mut T {} // ❌

// compile error

error[E0119]: conflicting implementations of trait `Trait` for type `&_`
 --> src/lib.rs:3:1
  |
2 | impl<T> Trait for T {}
  | ------------------- first implementation here
3 | impl<T> Trait for &T {}
  | ^^^^^^^^^^^^^^^^^^^^ conflicting implementation for `&_`

error[E0119]: conflicting implementations of trait `Trait` for type `&mut _`
 --> src/lib.rs:4:1
  |
2 | impl<T> Trait for T {}
  | ------------------- first implementation here
3 | impl<T> Trait for &T {}
4 | impl<T> Trait for &mut T {}
  | ^^^^^^^^^^^^^^^^^^^^^^^^ conflicting implementation for `&mut _`
```
The compiler doesn't allow us to define an implementation of Trait for &T and &mut T since it would conflict with the implementation of Trait for T which already includes all of &T and &mut T.  
The program below compiles as expected, since &T and &mut T are disjoint:
```rust
trait Trait {}
impl<T> Trait for &T {} // ✅
impl<T> Trait for &mut T {} // ✅
```
But,without the generic(T),for a concrete type：  
```rust
trait Trait {}
struct Struct;
impl Trait for Struct {} // ✅
impl Trait for &Struct {} // ✅
impl Trait for &mut Struct {} // ✅
```
#### if T: 'static then T must be valid for the entire program?

已在tokio中学习
#### &'a T and T: 'a are the same thing

This misconception is a generalized普遍的 version of the one above.  

&'a T requires and implies T: 'a   
since a reference to T of lifetime 'a cannot be valid for 'a if T itself is not valid for 'a. 
一个生命周期为 'a 的对 T 的引用（&'a T），要求并且隐含了 T必须“活得”比 'a 长（T: 'a）。因为如果 T 本身对于生命周期 'a都是无效的，那么一个指向 T 的、生命周期为 'a 的引用也必然是无效的。

For example, the Rust compiler will never allow the construction of the type &'static Ref<'a, T> because if Ref is only valid for 'a we can't make a 'static reference to it.

**T: 'a includes all &'a T but the reverse is not true.**

```rust
// only takes ref types that can outlive 'a
fn t_ref<'a, T: 'a>(t: &'a T) {}

// takes any types that can outlive 'a
fn t_bound<'a, T: 'a>(t: T) {}

// owned type自有类型 which contains a reference
struct Ref<'a, T: 'a>(&'a T);

fn main() {
    let string = String::from("string");

    t_bound(&string); // ✅
    t_bound(Ref(&string)); // ✅
    t_bound(&Ref(&string)); // ✅

    t_ref(&string); // ✅
    t_ref(Ref(&string)); // ❌ - expected ref, found struct
    t_ref(&Ref(&string)); // ✅

    // string can outlive 'static which is longer than 'a
    t_bound(string); // ✅
}
```
Key：  
1. T: 'a is more general and more flexible than &'a T
2. T: 'a accepts owned types, owned types which contain references, and references（T:a`,接受自有类型（自有类型的lifetime被编译器认为是static）、包含引用的自有类型（在自有类型中的引用的声明周期outlive a）、引用(类似上一种情况)）
3. &'a T only accepts references
4. if T: 'static then T: 'a since 'static >= 'a for all 'a
#### my code isn't generic and doesn't have lifetimes

Misconception Corollaries  
it's possible to avoid using generics and lifetimes  
注：lifetime也是generic的一种

The Rust borrow checker will infer them following these rules:
1. every input ref to a function gets a distinct lifetime
2. if there's exactly one input lifetime it gets applied to all output refs
3. if there's multiple input lifetimes but one of them is &self or &mut self then the lifetime of self is applied to all output refs
4. otherwise output lifetimes have to be made explicit
```rust
// elided
fn print(s: &str);

// expanded
fn print<'a>(s: &'a str);

// elided
fn trim(s: &str) -> &str;

// expanded
fn trim<'a>(s: &'a str) -> &'a str;

// illegal, can't determine output lifetime, no inputs
fn get_str() -> &str;

// explicit options include
fn get_str<'a>() -> &'a str; // generic version
fn get_str() -> &'static str; // 'static version

// illegal, can't determine output lifetime, multiple inputs
fn overlap(s: &str, t: &str) -> &str;

// explicit (but still partially elided) options include
fn overlap<'a>(s: &'a str, t: &str) -> &'a str; // output can't outlive s
fn overlap<'a>(s: &str, t: &'a str) -> &'a str; // output can't outlive t
fn overlap<'a>(s: &'a str, t: &'a str) -> &'a str; // output can't outlive s & t
fn overlap(s: &str, t: &str) -> &'static str; // output can outlive s & t
fn overlap<'a>(s: &str, t: &str) -> &'a str; // no relationship between input & output lifetimes

// expanded
fn overlap<'a, 'b>(s: &'a str, t: &'b str) -> &'a str;
fn overlap<'a, 'b>(s: &'a str, t: &'b str) -> &'b str;
fn overlap<'a>(s: &'a str, t: &'a str) -> &'a str;
fn overlap<'a, 'b>(s: &'a str, t: &'b str) -> &'static str;
fn overlap<'a, 'b, 'c>(s: &'a str, t: &'b str) -> &'c str;

// elided
fn compare(&self, s: &str) -> &str;

// expanded
fn compare<'a, 'b>(&'a self, &'b str) -> &'a str;
```
If you've ever written:  
1. a function which takes or returns references
2. a struct method which takes or returns references
3. a generic function
4. a trait object (more on this later)
5. a closure (more on this later)

then your code has generic elided lifetime annotations all over it.

Key:  
almost all Rust code is generic code and there's elided lifetime annotations everywhere
####  if it compiles then my lifetime annotations are correct

Misconception Corollaries推论：  
1. Rust's lifetime elision rules for functions are always right
2. Rust's borrow checker is always right, technically and semantically技术上和语义上借用检查器总是对的
3. Rust knows more about the semantics of my program than I do

```rust
struct ByteIter<'a> {
    remainder: &'a [u8]
}

impl<'a> ByteIter<'a> {
    fn next(&mut self) -> Option<&u8> {
        if self.remainder.is_empty() {
            None
        } else {
            let byte = &self.remainder[0];
            self.remainder = &self.remainder[1..];
            Some(byte)
        }
    }
}

fn main() {
    let mut bytes = ByteIter { remainder: b"1" };
    assert_eq!(Some(&b'1'), bytes.next());
    assert_eq!(None, bytes.next());
}
```
ByteIter is an iterator that iterates over a slice of bytes.  
It seems to work fine, but what if we want to check a couple bytes at a time?
```rust
fn main() {
    let mut bytes = ByteIter { remainder: b"1123" };
    let byte_1 = bytes.next();
    let byte_2 = bytes.next();//这里不是reborrowing
    if byte_1 == byte_2 { // ❌
        // do something
    }
}

//error
error[E0499]: cannot borrow `bytes` as mutable more than once at a time
  --> src/main.rs:20:18
   |
19 |     let byte_1 = bytes.next();
   |                  ----- first mutable borrow occurs here
20 |     let byte_2 = bytes.next();
   |                  ^^^^^ second mutable borrow occurs here
21 |     if byte_1 == byte_2 {
   |        ------ first borrow later used here
```
这里为什么会出现违反同一时间有两个可变借用的情况：  
此处应用了生命周期省略的规则，在`let byte_1 = bytes.next();`这里扩展为`fn next<'b>(&'b mut self) -> Option<&'b u8>`  
在执行`let byte_1 = bytes.next();`时调用了next，返回的lifetime(假设为borrwow_1)与input lifetime相同，为了保证引用的有效性，这个可变的input的lifetime**必须持续到byte_1不再使用为止**  
当程序试图执行 let byte_2 = bytes.next(); 时,需要对bytes再一次新的可变借用，的那此时borrwow_1因为byte_1的存在而被认为still active，这就会引发编译器在同一时间内，一个值不能既存在一个活跃的可变借用，又创建另一个可变借用这个规则

I guess we can copy each byte. Copying is okay when we're working with bytes（通过将fn next(&mut self) -> Option<&u8>，中的->Option<&u8>,改为`Option<u8>`,避免引用导致的限制）   
but if we turned ByteIter into a generic slice iterator that can iterate over any `&'a [T]` then we might want to use it in the future with types that may be very expensive or impossible to copy and clone.对于u8可以copy，但想要迭代更加泛型的T，可能会有copy代价较大的情况  
The code compiles so the lifetime annotations must be right, right?  
代码能编译通过，不代表它的设计就是好用或  符合用户直觉的。这个 ByteIter的实现是一个绝佳的例子，它展示了生命周期省略规则如何导致一个看似无害的 next方法创建出一个功能受限的、不符合人体工程学的API。这个问题的标准解决方案涉及到更高级的 Rust 特性（比如 Iterator trait 的设计），但这个例子本身完美地说明了理解生命周期和借用规则对于设计出好用的API是多么重要。           
### Iterator trait设计中避免这种情况的方法、使用reminder lifetime的不同（很重要）

1. 方式一（“使用 Item”）：通过一个关联类型或泛型参数来间接定义返回类型。其 核心是方法签名本身是抽象的，而返回引用的具体生命周期在别处（如 impl 块）定义
```rust
// 抽象形式 
trait SomeContract<'a> {                                                            type Item; // 在 impl 中可能会被定义为 &'a T
   fn get_ref(&mut self)->Self::Item;

}
```
2. 方式二：在方法签名中，直接、显式地为返回的引用标注生命周期
```rust
 impl<'a> MyType<'a> {
    fn get_ref(&mut self) -> &'a T {
    }
 }
```

这两种方式能够解决问题，其底层的 核心机制是完全相同的它们都达成了同一个目的：  
在方法签名层面，打破了生命周期省略规则的默认推断，明确地将输出引用的生命周期与结构体自身的生命周期（`'a`）绑定，而不是与 `&mut self` 的短暂借用绑定  
是同一个解决方案的两种不同“API设计模式”  
实际上更加推荐适用 type item的模式，item的有效性是在百年一起被静态的严格的证明

Nope, the current lifetime annotations are actually the source of the bug!  
It's particularly hard to spot because the buggy lifetime annotations are elided. Let's expand the elided lifetimes to get a clearer look at the problem:
```rust
struct ByteIter<'a> {
    remainder: &'a [u8]
}

impl<'a> ByteIter<'a> {
    fn next<'b>(&'b mut self) -> Option<&'b u8> {
        if self.remainder.is_empty() {
            None
        } else {
            let byte = &self.remainder[0];
            self.remainder = &self.remainder[1..];
            Some(byte)
        }
    }
}
```
That didn't help at all. I'm still confused.  
Here's a hot tip that only Rust pros know: **give your lifetime annotations descriptive names**. Let's try again:
```rust
struct ByteIter<'remainder> {
    remainder: &'remainder [u8]
}

impl<'remainder> ByteIter<'remainder> {
    fn next<'mut_self>(&'mut_self mut self) -> Option<&'mut_self u8> {
        if self.remainder.is_empty() {
            None
        } else {
            let byte = &self.remainder[0];
            self.remainder = &self.remainder[1..];
            Some(byte)
        }
    }
}
```
Each returned byte is annotated with 'mut_self but **the bytes are clearly coming from 'remainder!** Let's fix it.
```rust
struct ByteIter<'remainder> {
    remainder: &'remainder [u8]
}

impl<'remainder> ByteIter<'remainder> {
    fn next(&mut self) -> Option<&'remainder u8> {
        if self.remainder.is_empty() {
            None
        } else {
            let byte = &self.remainder[0];
            self.remainder = &self.remainder[1..];
            Some(byte)
        }
    }
}

fn main() {
    let mut bytes = ByteIter { remainder: b"1123" };
    let byte_1 = bytes.next();
    let byte_2 = bytes.next();
    std::mem::drop(bytes); // we can even drop the iterator now!
    if byte_1 == byte_2 { // ✅
        // do something
    }
}
```
为什么这里不仅没有违反同一时间存在两个可变借用，以及在drop bytes后byte_1和byte_2仍可以使用？  
drop后仍可以使用根本原因是：  
next 方法的返回类型被 显式地注解 (explicitly       annotated) 了生命周期，从而将返回引用的生命周期与原始数据的生命周期绑定，而不是与 ByteIter 实例的生命周期绑定。  
详解：  
1. next 方法的显式生命周期注解  
方法签名 fn next(&mut self) -> Option<&'remainder u8> 是关键。  
通过 &'remainder u8 明确地告诉编译器，返回的 Option中包含的引用，其生命周期是 'remainder。  
这覆盖了默认的生命周期省略规则。编译器不会再将输出引用的生命周期与输入 &mut self 的生命周期关联起来。
1. 生命周期 'remainder 的来源
- 在 struct ByteIter<'remainder> 的定义中，'remainder是一个泛型生命周期参数。
- 当 let mut bytes = ByteIter { remainder: b"1123" };这行代码执行时，'remainder 被推断为静态字面量 b"1123" （字节字符串字面量，引号内的内容会被解释为字节序列，而不是Unicode字符，其类型是`&[u8;4]`，等价于&[49, 49, 50, 51]  // ASCII 码值）的生命周期。  
由于这是一个静态字面量，其生命周期是'static，它在整个程序的运行期间都有效。
- 因此，next 方法返回的引用类型实际上是 &'static u8。
1. 引用与迭代器实例的解耦
- 当 bytes.next() 被调用时，它返回一个直接指向原始数据 b"1123"内存区域的引用。
- ByteIter 结构体本身只是一个“视图”或“游标”，它包含一个指向原始数据的切片remainder，但它不拥有 这些数据。
- byte_1和 byte_2 中存储的引用，其有效性仅依赖于原始数据 b"1123" 的有效性。它们与 ByteIter 结构体实例的内存或生命周期没有任何关系。

这里byte_1 和 byte_2不是可变借用  
1. byte_1 和 byte_2 是持有 next 方法返回值的变量。根据方法签名 fn next(&mut self) -> Option<&'remainder u8>，这两个变量的类型都是 Option<&'remainder u8>。  
它们持有的是一个包含 不可变引用 (immutable reference)（也称共享引用, shared reference）的 Option。它们自身并不是借用，更不是可变借用
2. 真正的 可变借用 发生在 bytes.next() 的 方法调用期间  
- 发生位置: 可变借用作用的对象是 bytes 这个结构体实例。
- 发生原因: next 方法的签名是 fn next(&mut self)，它需要以可变的方式借用self（即 bytes 实例），以便能够修改 self 的内部状态（具体来说是self.remainder）
- 持续时间: 这个可变借用是 短暂的 (transient)。它在 bytes.next()方法开始执行时创建，并在方法执行完毕、返回值（Option<&'remainder u8>）被交出后立即结束

这里不存在两个同时存在的可变借用。实际发生的是两个 连续的、不重叠的短暂可变借用：
1. `let byte_1 = bytes.next();` 执行时:   
* bytes 被可变借用。
* next 方法内部修改 bytes.remainder。
* next 方法返回一个 Some(&'remainder u8)
* 对 bytes 的可变借用在此时释放。
* 返回值被赋给 byte_1。
2 . `let byte_2 = bytes.next();` 执行时:
* 由于前一个可变借用已经释放，bytes 可以被 再次 可变借用
* next 方法再次修改 bytes.remainder
* next 方法返回另一个 Some(&'remainder u8)
* 对 bytes 的可变借用再次 释放
* 返回值被赋给 byte_2。  


Now that we look back on the previous version of our program it was obviously wrong, so why did Rust compile it? The answer is simple: it was memory safe.  
The Rust borrow checker only cares about the lifetime annotations in a program to the extent it can use them to statically verify the memory safety of the program.   
Rust will happily compile programs even if the lifetime annotations have semantic errors, and the consequence of this is that the program becomes unnecessarily restrictive.

Here's a quick example that's the opposite of the previous example: Rust's lifetime elision rules happen to be semantically correct in this instance but we unintentionally write a very restrictive method with our own unnecessary explicit lifetime annotations.
```rust
#[derive(Debug)]
struct NumRef<'a>(&'a i32);

impl<'a> NumRef<'a> {
    // my struct is generic over 'a so that means I need to annotate
    // my self parameters with 'a too, right? (answer: no, not right)
    fn some_method(&'a mut self) {}//`a，是错误的关键
}

fn main() {
    let mut num_ref = NumRef(&5);
    num_ref.some_method(); // mutably borrows num_ref for the rest of its lifetime
    num_ref.some_method(); // ❌
    println!("{:?}", num_ref); // ❌
}
```
If we have some struct generic over 'a we almost never want to write a method with a &'a mut self receiver. What we're communicating to Rust is "this method will mutably borrow the struct for the entirety of the struct's lifetime". In practice this means Rust's borrow checker will only allow at most one call to some_method before the struct becomes permanently mutably borrowed and thus unusable. The use-cases for this are extremely rare but the code above is very easy for confused beginners to write and it compiles. The fix is to not add unnecessary explicit lifetime annotations and let Rust's lifetime elision rules handle it:
```rust
#[derive(Debug)]
struct NumRef<'a>(&'a i32);

impl<'a> NumRef<'a> {
    // no more 'a on mut self
    fn some_method(&mut self) {}

    // above line desugars to
    fn some_method_desugared<'b>(&'b mut self){}
}

fn main() {
    let mut num_ref = NumRef(&5);
    num_ref.some_method();
    num_ref.some_method(); // ✅
    println!("{:?}", num_ref); // ✅
}
```

Key:  
1. Rust's lifetime elision rules for functions are not always right for every situation
2. Rust does not know more about the semantics of your program than you do give your lifetime annotations descriptive name
3. try to be mindful of where you place explicit lifetime annotations and why


重点：理解各种lifetime的具体含义，才能真正知道为什么会错误

通常情况下，您不需要为 &self 或 &mut self 手动标注生命周期。Rust 的生命周期省略规则（Lifetime Elision  Rules）会自动处理绝大多数情况。   
- &mut self 的生命周期通常由编译器自动推断，并且只持续方法调用的时长。
- 只有在特殊情况下（例如，方法需要返回一个派生自 self 的引用时），才需要手动标注 &self 或 &mut self 的生命周期，以将其与输入或输出的生命周期关联起来。
- &'a mut self 是一个非常强烈的约束，它将借用的生命周期与结构体内部数据的生命周期 'a绑定，这通常不是您想要的结果













## rebroswwing

































































































# reference

A reference sometimes called a borrow    
Rust uses lifetimes to track the relationships between borrows and ownership   
Variance is the concept that Rust borrows to define relationships about subtypes through their generic

There are two kinds of references:
1. Shared reference: &
2. Mutable reference: &mut

The whole model references obey the following rules:
1. A reference cannot outlive its referent
2. A mutable reference cannot be aliased
## The dereference operator

https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-dereference-operator

## reference's coercions

将reference自动deref coercion为其指向的内容，主要涉及到Type Coercion  
资料来源：  
The rust referenc:https://doc.rust-lang.org/reference/type-coercions.html#type-coercions
RFC401、1558：https://github.com/rust-lang/rfcs/blob/master/text/0401-coercions.md
### Type coercions

**Type coercions are implicit operations that change the type of a value.** They happen automatically at specific locations and are highly restricted in what types actually coerce.

Any conversions allowed by coercion can also be explicitly performed by the type cast operator, as.
#### Coercion sites

A coercion can only occur at certain coercion sites in a program; these are typically places where the desired type is explicit or can be derived派生 by propagation from explicit types (**without type inference**)  
1. let statements where an explicit type is given.
```rust
//&mut 42 is coerced to have type &i8 in the following:
let _: &i8 = &mut 42;
```
2. static and const item declarations (similar to let statements).
当你为常量或静态变量显式声明一个类型时，如果初始化表达式的类型可以通过合法的强制转换规则转变为声明的类型，编译器就会自动执行这个转换
```rust
    2     // 声明一个常量，类型为 `&[i32]` (一个 i32 切片的引用)
    3     // 初始化表达式 `&[1, 2, 3]` 的类型是 `&[i32; 3]` (一个包含3个i32的数组的引用)
    4     //
    5     // 编译器在这里执行了强制转换：
    6     // from: &'static [i32; 3]  (数组引用)
    7     // to:   &'static [i32]    (切片引用)
    8     const CONST_SLICE: &[i32] = &[1, 2, 3];
    9
   10     // `static` 变量的规则完全相同
   11     static STATIC_SLICE: &[i32] = &[4, 5, 6, 7];
   12
   13     // 我们可以像使用普通切片一样使用它们
   14     print_slice_info(CONST_SLICE);
   15     print_slice_info(STATIC_SLICE);
```
3. Arguments for function calls
The value being coerced is the actual parameter, and it is coerced to the type of the formal parameter.
```rust
//&mut 42 is coerced to have type &i8
fn bar(_: &i8) { }

fn main() {
    bar(&mut 42);
}
```
4. Function results—either the final line of a block if it is not semicolon-terminated or any expression in a return statement
```rust
//x is coerced to have type &dyn Display
use std::fmt::Display;
fn foo(x: &u32) -> &dyn Display {
    x
}
```
5. For method calls, the receiver (self parameter) type is coerced differente from Arguments for function calls   

When looking up a method call, the receiver may be automatically dereferenced or borrowed in order to call a method. This requires a more complex lookup process than for other functions, since there may be a number of possible methods to call. The following procedure is used:  
- The first step is to build a list of candidate receiver types. Obtain these by repeatedly dereferencing the receiver expression’s type, adding each type encountered to the list, then finally attempting an unsized coercion非定长转换 at the end, and adding the result type if that is successful.
当编译器看到一个方法调用时，它不能只看 receiver 变量本身的类型。因为 Rust 的 Deref特性允许一个类型“表现得像”另一个类型（例如 `Box<T>` 表现得像T）。因此，编译器需要生成一个包含所有潜在可能性的类型列表。这个列表中的每一个类型，都有可能是 method方法真正所属的那个类型。这个过程就是为了给下一步查找方法提供所有可能的“靶子”。  
在 Deref 链条走到底之后，编译器会做最后一次尝试。它会取列表中的最后一个类型，看看这个类型是否能被强制转换为一个“非定长类型”（Unsized Type 或 Dynamically Sized Type, DST）。  
最常见的非定长强制转换就是从数组 `[T; N]` 到切片 `[T]` 的转换。
- Then, for each candidate T, add &T and &mut T to the list immediately after T.  
For instance, if the receiver has type `Box<[i32;2]>`, then the candidate types will be `Box<[i32;2]>`, `&Box<[i32;2]>`, `&mut Box<[i32;2]>`, `[i32; 2]` (by dereferencing), `&[i32; 2]`, `&mut [i32; 2]`, `[i32]` (by unsized coercion), `&[i32]`, and finally`&mut [i32]`.
- Then, for each candidate type T, search for a visible method with a receiver of that type in the following places:  
T’s inherent methods (methods implemented directly on T).  
Any of the methods provided by a visible trait implemented by T. If T is a type parameter, methods provided by trait bounds on T are looked up first. Then all remaining methods in scope are looked up.
>The lookup is done for each type in order, which can occasionally lead to surprising results. The below code will print “In trait impl!”, because &self methods are looked up first, the trait method is found before the struct’s &mut self method is found.
```rust
struct Foo {}

trait Bar {
  fn bar(&self);
}

impl Foo {
  fn bar(&mut self) {
    println!("In struct impl!")
  }
}

impl Bar for Foo {
  fn bar(&self) {
    println!("In trait impl!")
  }
}

fn main() {
  let mut f = Foo{};
  f.bar();
}
```
- If this results in multiple possible candidates, then it is an error, and the receiver must be converted to an appropriate receiver type to make the method call.


This process does not take into account the mutability or lifetime of the receiver, or whether a method is unsafe. Once a method is looked up, if it can’t be called for one (or more) of those reasons, the result is a compiler error.

If a step is reached where there is more than one possible method, such as where generic methods or traits are considered the same, then it is a compiler error. These cases require a disambiguating function call syntax for method and function invocation.
```rust
trait Pretty {
    fn print(&self);
}

trait Ugly {
    fn print(&self);
}

struct Foo;
impl Pretty for Foo {
    fn print(&self) {}
}

struct Bar;
impl Pretty for Bar {
    fn print(&self) {}
}
impl Ugly for Bar {
    fn print(&self) {}
}

fn main() {
    let f = Foo;
    let b = Bar;

    // we can do this because we only have one item called `print` for `Foo`s
    f.print();
    // more explicit, and, in the case of `Foo`, not necessary
    Foo::print(&f);
    // if you're not into the whole brevity thing
    <Foo as Pretty>::print(&f);

    // b.print(); // Error: multiple 'print' found
    // Bar::print(&b); // Still an error: multiple `print` found

    // necessary because of in-scope items defining `print`
    <Bar as Pretty>::print(&b);
}
```
>For trait objects, if there is an inherent method of the same name as a trait method, it will give a compiler error when trying to call the method in a method call expression. Instead, you can call the method using disambiguating function call syntax, in which case it calls the trait method, not the inherent method. There is no way to call the inherent method. Just don’t define inherent methods on trait objects with the same name as a trait method and you’ll be fine.
6. Instantiations实例化 of struct, union, or enum variant fields























































































































































































































































































### Deref Coercion














































































































































