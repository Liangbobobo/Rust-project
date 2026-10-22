- [Collection And Iter](#collection-and-iter)
  - [Collection](#collection)
  - [Iter](#iter)
    - [Organization](#organization)
    - [iterator](#iterator)
      - [The three forms of iteration](#the-three-forms-of-iteration)
      - [Implementing Iterator](#implementing-iterator)
    - [Methods And Associate Functions In Iter](#methods-and-associate-functions-in-iter)
  - [扩展-About Consume Itertor](#扩展-about-consume-itertor)
    - [Rust 迭代器方法：`next()` 与 `nth()` 的所有权与消耗行为分析](#rust-迭代器方法next-与-nth-的所有权与消耗行为分析)
      - [1. 方法签名：`&mut self` 的含义](#1-方法签名mut-self-的含义)
      - [2. "消耗"的真正含义：消耗迭代器中的元素](#2-消耗的真正含义消耗迭代器中的元素)
        - [`next()` 的行为](#next-的行为)
        - [`nth(n)` 的行为](#nthn-的行为)
      - [3. 代码示例](#3-代码示例)
      - [4. 总结](#4-总结)
  - [扩展-Resume and Fused](#扩展-resume-and-fused)

# Collection And Iter

资料：  
<https://doc.rust-lang.org/std/collections/index.html>

## Collection

## Iter

std moudle iter  
<https://doc.rust-lang.org/std/iter/index.html#traits>

Iterators are heavily used in idiomatic Rust code, so it’s worth becoming familiar with them.

对于一个类型，只需要实现核心的trait iterator及next方法，就会自动获得map、filter等方法，极大的减少工作量。

### Organization

This module is largely organized by type:

1. Traits are the core portion: these traits define what kind of iterators exist and what you can do with them. The methods of these traits are worth putting some extra study time into.
2. Functions provide some helpful ways to create some basic iterators.
3. Structs are often the return types of the various methods on this module’s traits. You’ll usually want to look at the method that creates the struct, rather than the struct itself.

### iterator

在 Rust 中，“迭代器”不是一个具体的、单一的类型，而是一个“概念”或“角色”，这个角色由 `Iterator` 这个 trait来定义。任何实现了 `Iterator` trait 的具体类型（struct 或enum），都可以被称作一个迭代器，并享有迭代器世界的所有功能

The heart and soul of this module is the Iterator trait. The core of Iterator looks like this:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

1. An iterator has a method, next, which when called, returns an `Option<Item>`. Calling next will return Some(Item) as long as there are elements, and once they’ve all been exhausted, will return None to indicate that iteration is finished.
2. Individual个别的 iterators may choose to resume iteration, and so calling next again may or may not eventually start returning Some(Item) again开始再次返回 at some point (for example, see TryIter).[扩展-Resume and Fused](#扩展-resume-and-fused)
3. **Iterator’s full definition includes a number of other methods as well, but they are default methods, built on top of next, and so you get them for free.**
4. Iterators are also composable, and it’s common to chain them together to do more complex forms of processing.

#### The three forms of iteration

There are three common methods which can create iterators from a collection:

1. iter(), which iterates over &T.
2. iter_mut(), which iterates over &mut T.
3. into_iter(), which iterates over T.

Various things in the standard library may implement one or more of the three, where appropriate.

#### Implementing Iterator

Creating an iterator of your own involves two steps:

1. creating a struct to hold the iterator’s state,
2. and then implementing Iterator for that struct.
  
This is why there are so many structs in this module: there is one for each iterator and iterator adapter.

Also note that Iterator provides a default implementation of methods such as nth and fold which call next internally.
However, it is also possible to write a custom implementation of methods like nth and fold if an iterator can compute them more efficiently without calling next.

[扩展-About Consume Itertor](#扩展-about-consume-itertor)

### Methods And Associate Functions In Iter

next

nth

fold

map  

filte

## 扩展-About Consume Itertor

### Rust 迭代器方法：`next()` 与 `nth()` 的所有权与消耗行为分析

我们来澄清一下：从所有权的角度看，`next()` 和 `nth()` 都不会消耗迭代器本身。但从迭代器内部状态的角度看，`nth()` 会消耗掉更多的元素。

让我们从方法签名开始，这是最精确的证据。

#### 1. 方法签名：`&mut self` 的含义

- `next()` 的签名是：`fn next(&mut self) -> Option<Self::Item>`
- `nth()` 的签名是：`fn nth(&mut self, n: usize) -> Option<Self::Item>`

请注意，这两个方法都接收 `&mut self` 作为第一个参数。

- `&mut self` 代表对 `self`（也就是迭代器实例）的可变借用（mutable borrow）。
- 这意味着方法可以修改迭代器的内部状态。
- 但方法在执行完毕后，会将借用归还。它并不获取迭代器实例的所有权。

因此，在调用 `next()` 或 `nth()` 之后，你仍然拥有该迭代器变量的所有权，可以继续对其调用其他方法。

如果一个方法真的"消耗"了迭代器本身，它的参数会是 `self`（而不是 `&self` 或 `&mut self`），这意味着它获取了实例的所有权。典型的例子是 `collect()` 方法：`fn collect<B>(self) -> B`。调用 `iterator.collect()` 之后，`iterator` 变量就因为所有权被移走而失效了。

#### 2. "消耗"的真正含义：消耗迭代器中的元素

现在我们来讨论"消耗"的第二层含义，也就是您问题的关键点：消耗的是迭代器序列中的元素。

##### `next()` 的行为

`next()` 方法只做一件事：尝试从序列中获取下一个元素。

- 如果成功，它返回 `Some(element)`，并把迭代器的内部指针（或状态）向前移动一步。
- 如果序列已经结束，它返回 `None`。

`next()` 每次调用最多只会消耗序列中的一个元素。

##### `nth(n)` 的行为

`nth(n)` 的目标是获取序列中第 `n` 个（从 0 开始计数）的元素。为了做到这一点，它的默认实现必须：

1. 调用 `next()` `n` 次，来跳过前面的 0 到 n-1 号元素。这些被跳过的元素就被"消耗"掉了，因为迭代器已经越过了它们。
2. 再调用一次 `next()` 来获取第 n 号元素并返回它。

所以，`nth(n)` 一次调用会消耗掉序列中最多 `n + 1` 个元素。

#### 3. 代码示例

下面的代码清晰地展示了 `next()` 和 `nth()` 的区别：

```rust
fn main() {
    let mut my_iter = vec!['A', 'B', 'C', 'D', 'E'].into_iter();
    
    // 1. 调用 next()
    // 它消耗了元素 'A'，并返回 Some('A')
    // 迭代器的下一个元素现在是 'B'
    println!("第一次调用 next(): {:?}", my_iter.next()); // 输出: Some('A')
    
    // 2. 调用 nth(1)
    // 这不是获取整个序列的第1个元素('B')
    // 而是获取 *剩余序列* 的第1个元素
    // 剩余序列是 ['B', 'C', 'D', 'E']
    // 为了获取第1个元素('C')，它必须：
    // - 消耗掉第0个元素 'B' (跳过)
    // - 获取并返回第1个元素 'C'
    println!("调用 nth(1): {:?}", my_iter.nth(1)); // 输出: Some('C')
    
    // 3. 再次调用 next()
    // 此时，'A', 'B', 'C' 都已经被消耗掉了
    // 迭代器的下一个元素是 'D'
    println!("在 nth(1) 之后调用 next(): {:?}", my_iter.next()); // 输出: Some('D')
}
```

#### 4. 总结

| 方法 | 所有权 | 元素消耗 | 行为描述 |
|------|--------|----------|----------|
| `next()` | 不消耗迭代器 (接收 `&mut self`) | 消耗 1 个元素 | 返回序列中的下一个元素，并将迭代器状态推进一格。 |
| `nth(n)` | 不消耗迭代器 (接收 `&mut self`) | 消耗 **最多 n+1** 个元素 | 跳过 n 个元素，然后返回下一个元素。所有被跳过的元素和返回的元素都被消耗。 |

所以，当说"`nth` 会消耗一个迭代器"时，这是一种简略但不精确的说法。更精确的描述是："`nth` 会消耗迭代器中的多个元素"。而 `next` 只会消耗一个元素。从所有权角度看，两者都不会消耗迭代器本身。

## 扩展-Resume and Fused

常规迭代器 (Fused / 熔断的)， Trait ：FusedIterator，当取出最后一个值的时候，再怎么调用next()都会返回none，一旦
None，就永远 None  
可恢复迭代器 (Non-Fused / 非熔断的)，没有实现Trait ：FusedIterator的类型，它们返回 None可能仅仅意味着“我现在是空的”，而不是“我永远结束了”。这种迭代器返回 None 是一个临时状态，代表“暂时没有可用项”，而不是一个永久的结束信号  

- 非阻塞的并发通信 (MPSC Channel)，。std::sync::mpsc::Receiver 的 try_iter()，方法返回的就是一个可恢复迭代器。它会立即清空通道中所有已存在的消息。如果通道为空，它就返回 None并结束。但之后其他线程仍然可以向通道发送消息，你下一次再使用这个迭代器时，又能取到新的消息。  
- 异步网络编程，想象一个服务器正在监听一个网络端口。一个非阻塞的迭代器可以被用来轮询新接入的客户端连接或收到的数据包。connections.next() 尝试接受一个新连接。如果没有客户端在此时连接，它返回None，服务器主循环可以继续处理其他任务（比如更新游戏状态、处理已连接的客户端等）。当一个新客户端发起连接时，下一次 connections.next() 就会返回 Some(new_connection)。
- 轮询硬件或用户输入，在游戏引擎或 GUI 应用中，可以用一个迭代器来获取一帧内发生的所有用户输入事件（键盘、鼠标）。在一帧的开始，你遍历这个迭代器来处理所有事件。如果在这一帧内用户没有任何操作，迭代器立即就返回 None。在下一帧，如果用户按下了键盘，迭代器就会返回 Some(KeyPressEvent)。  

对于想要实现熔断行为的类型，就应该去实现 `FusedIterator` 这个 trait。  
FusedIterator 是一个标记 trait (marker trait)。和其他像 Send, Sync, Copy 一样的标记 trait 类似，FusedIterator本身是空的，里面没有任何需要你去编写代码的方法。如标准库中的定义`pub trait FusedIterator: Iterator {}`  
实现即是声明：因此，为你的迭代器实现 FusedIterator，你所需要做的仅仅是写下一行空的 impl 块：`impl FusedIterator for MyIterator {}` 这行代码的意义不是“添加熔断功能”，而是“向编译器和使用者郑重声明：我的 `next` 方法已经具备了熔断行为”。  

这称为先有行为，后有声明  
首先，你必须在你的 Iterator 实现中，确保 next() 方法的行为是熔断的。也就是说，你得自己写代码保证，一旦next() 返回了一次 None，它在后续的所有调用中都只会返回 None。  
然后，你通过 impl FusedIterator for ... {} 来将这个事实“公之于众”。

既然行为已经实现了，为什么还要多此一举去实现一个空的 trait 呢？  
代码优化： 标准库中的某些函数或适配器可以利用这个信息来进行优化。最典型的例子就是 iterator.fuse() 方法。.fuse() 方法可以把任何迭代器（无论是否熔断）包装成一个保证熔断的新迭代器。  
但是，如果它发现被调用的迭代器已经实现了 `FusedIterator`，它就知道没必要再进行一次额外的包装了，可以直接返回原迭代器本身，从而避免了不必要的开销。

更强的契约和可预测性：当一个迭代器实现了 FusedIterator，它的使用者就可以放心地编写代码，而不必担心在 None之后迭代器会“复活”。例如，在某些复杂的循环逻辑中，可以省去一些不必要的检查。

可以多次调用 next，是因为：

1. next 只借用迭代器，不消耗它本身。
2. next 每次都返回一个全新的、包含着被移出元素所有权的 `Option`。
