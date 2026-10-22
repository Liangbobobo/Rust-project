- [Async, Await, Futures, and Streams](#async-await-futures-and-streams)
  - [Parallelism and Concurrency](#parallelism-and-concurrency)
  - [Futures and the Async Syntax](#futures-and-the-async-syntax)
  - [.await、fn await、await{}](#awaitfn-awaitawait)
    - [区分参数是函数还是闭包](#区分参数是函数还是闭包)
    - [async {}、fn async{}](#async-fn-async)
    - [.await](#await)
      - [编译时](#编译时)
    - [运行时：poll、Waker 与执行器的协作](#运行时pollwaker-与执行器的协作)
      - [步骤 1: 轮询内部 Future](#步骤-1-轮询内部-future)
      - [步骤 2: 检查 poll 的结果](#步骤-2-检查-poll-的结果)
  - [async实际的执行过程](#async实际的执行过程)
      - [第一次 poll](#第一次-poll)
    - [Racing Our Two URLs Against Each Other](#racing-our-two-urls-against-each-other)
  - [Applying Concurrency with Async](#applying-concurrency-with-async)
    - [Counting Up递增计数 on Two Tasks Using Message Passing](#counting-up递增计数-on-two-tasks-using-message-passing)
  - [Working with Any Number of Futures](#working-with-any-number-of-futures)
  - [Streams: Futures in Sequence](#streams-futures-in-sequence)
  - [A Closer Look at the Traits for Async](#a-closer-look-at-the-traits-for-async)
  - [The Future Trait](#the-future-trait)
    - [The Pin and Unpin Traits](#the-pin-and-unpin-traits)
      - [Pin](#pin)
- [Asynchronous Programming in Rust](#asynchronous-programming-in-rust)
  - [Task、future](#taskfuture)
  - [async和ownership、lifetime](#async和ownershiplifetime)



# Async, Await, Futures, and Streams

Modern computers offer two techniques for working on more than one operation at a time: parallelism and concurrency.

Say you’re exporting导出 a video you’ve created of a family celebration, an operation that could take anywhere from minutes to hours.   
The video export will use as much CPU and GPU power as it can. If you had only one CPU core and your operating system didn’t pause that export until it completed—that is, if it executed the export synchronously同步的—you couldn’t do anything else on your computer while that task was running.  
That would be a pretty frustrating沮丧的 experience.Fortunately, your computer’s operating system can, and does, invisibly interrupt不可见的打断 the export often enough to let you get other work done simultaneously同时的.

Now say you’re downloading a video shared by someone else, which can also take a while需要一段时间 but does not take up as much CPU time.  
In this case, the CPU has to wait for data to arrive from the network. While you can start reading the data once it starts to arrive, it might take some time for all of it to show up.  
Even once the data is all present, if the video is quite large, it could take at least a second or two to load it all. That might not sound like much, but it’s a very long time for a modern processor, which can perform billions of operations every second. Again, your operating system will invisibly interrupt your program to allow the CPU to perform other work while waiting for the network call to finish.

The video export is an example of a CPU-bound or compute-bound operation.  
It’s limited by the computer’s potential data processing speed within the CPU or GPU, and how much of that speed it can dedicate to the operation.   
The video download is an example of an IO-bound operation, because it’s limited by the speed of the computer’s input and output; it can only go as fast as the data can be sent across the network.

In both of these examples, the operating system’s invisible interrupts provide a form of concurrency.  
That concurrency happens only at the level of the entire program, though: the operating system interrupts one program to let other programs get work done.   
In many cases, because we understand our programs at a much more granular颗粒度 level than the operating system does, we can spot看见/注意到 opportunities for concurrency that the operating system can’t see.  
For example, if we’re building a tool to manage file downloads, we should be able to write our program so that starting one download won’t lock up the UI, and users should be able to start multiple downloads at the same time. Many operating system APIs for interacting with the network are blocking, though; that is, they block the program’s progress until the data they’re processing is completely ready.  
`
Note: This is how most function calls work, if you think about it. However, the term blocking is usually reserved for function calls that interact with files, the network, or other resources on the computer, because those are the cases where an individual program would benefit from the operation being non-blocking.
`

We could avoid blocking our main thread by spawning a dedicated thread专用线程 to download each file. However, the overhead of those threads would eventually become a problem. It would be preferable更好的 if the call didn’t block in the first place. It would also be better if we could write in the same direct style we use in blocking code, similar to this:
```
let data = fetch_data_from(url).await;
println!("{data}");
```
## Parallelism and Concurrency

A concurrent workflow, switching between Task A and Task B  
![](https://doc.rust-lang.org/book/img/trpl17-01.svg)  

A parallel workflow, where work happens on Task A and Task B independently
![](https://doc.rust-lang.org/book/img/trpl17-02.svg)

A partially parallel workflow, where work happens on Task A and Task B independently until Task A3 is blocked on the results of Task B3. 
![](https://doc.rust-lang.org/book/img/trpl17-03.svg)
Likewise同样的, you might realize that one of your own tasks depends on another of your tasks. 

On a machine with a single CPU core, the CPU can perform only one operation at a time, but it can still work concurrently. Using tools such as threads, processes, and async, the computer can pause one activity and switch to others before eventually cycling back to that first activity again.   
On a machine with multiple CPU cores, it can also do work in parallel. One core can be performing one task while another core performs a completely unrelated one, and those operations actually happen at the same time.

When working with async in Rust, we’re always dealing with concurrency. **Depending on the hardware, the operating system, and the async runtime we are using** (more on async runtimes shortly稍后), that **concurrency may also use parallelism under the hood**.
## Futures and the Async Syntax

A future is a value that may not be ready now but will become ready at some point in the future.   
Rust provides a **Future trait** as a building block so that different async operations can be implemented with different data structures but with a common interface. In Rust, futures are types that implement the Future trait. Each future holds its own information about the progress that has been made and what “ready” means.

await 可以等待任何实现了 Future Trait的类型。这意味着，只要一个操作可以被建模为“在未来某个时刻会完成”，它就可以被 await。常见的场景有：  
1. I/O-bound 操作
2. 时间相关的操作
3. 同步与通信操作（任务间，如：通道为空、通道已满、锁）
4. CPU-bound 操作（需要特别处理）



You can apply the async keyword to **blocks and functions** to specify that they can be interrupted and resumed.   
Within an async block or async function, you can use the await keyword to await a future (that is, wait for it to become ready). **Any point where you await a future within an async block or function is a potential spot for that async block or function to pause and resume.** The process of checking with a future to see if its value is available yet is called polling.


When writing async Rust, we use the async and await keywords most of the time. Rust compiles them into equivalent code using the Future trait, much as it compiles for loops into equivalent code using the Iterator trait. 

Futures in Rust are lazy: they don’t do anything until you ask them to with the await keyword. (In fact, Rust will show a compiler warning if you don’t use a future.)  
Like,Iterators do nothing unless you call their next method—whether directly or by using for loops or methods such as map that use next under the hood.

When Rust sees a block marked with the async keyword, it compiles it into **a unique, anonymous data type that implements the Future trait**.   
When Rust sees a function marked with async, it compiles it into a non-async function whose body is an async block. An async function’s return type is the type of the anonymous data type the compiler creates for that async block.

**Remember that blocks are expressions. This whole block is the expression returned from the function.**

The reason **main can’t be marked async** is that async code needs a runtime: a Rust crate that manages the details of executing asynchronous code. A program’s main function can initialize a runtime, but it’s not a runtime itself. (We’ll see more about why this is the case in a bit.) Every Rust program that executes async code has at least one place where it sets up a runtime and executes the futures.

Most languages that support async bundle a runtime异步绑定运行时, but Rust does not. Instead, there are many different async runtimes available, each of which makes different tradeoffs suitable to the use case it targets. For example, a high-throughput web server with many CPU cores and a large amount of RAM has very different needs than a microcontroller with a single core, a small amount of RAM, and no heap allocation ability. The crates that provide those runtimes also often supply async versions of common functionality such as file or network I/O.

Each await point—that is, every place where the code uses the await keyword—represents a place where control is handed back to the runtime.   
To make that work, Rust needs to keep track of the state involved in the async block so that the runtime can kick off开始干某事 some other work and then come back when it’s ready to try advancing the first one again.  
This is an invisible state machine, as if you’d written an enum like this to save the current state at each await point:  
```
enum PageTitleFuture<'a> {
    Initial { url: &'a str },
    GetAwaitPoint { url: &'a str },
    TextAwaitPoint { response: trpl::Response },
}

```
the Rust compiler creates and manages the state machine data structures for async code automatically. The normal borrowing and ownership rules around data structures all still apply, and happily, the compiler also handles checking those for us and provides useful error messages.   
Ultimately, something has to execute this state machine, and that something is a runtime.运行时来执行这个状态机 (This is why you may come across references to executors when looking into runtimes: an executor is the part of a runtime responsible for executing the async code.)  

If main were an async function, something else would need to manage the state machine for whatever future main returned, but main is the starting point for the program!在使用 `#[tokio::main]`这个过程宏（procedural macro）。它是一个“代码转换器”，在你编译代码时，它会把你的 async fn main重写成一个完全不同的、符合操作系统要求的同步 `fn main`。
## .await、fn await、await{}

fn await、await{}生成状态机，  
.await，不生成状态机，它在“逻辑上”消费了一个 `Future`，但这个 `Future`本身（即存储它的那块内存）通常仍然存在，直到包含它的外层 `Future` 状态机发生状态转换或被销毁。  
.await 做的就是：不断地 `poll` 一个 `Future`，如果它 `Ready` 了，就取出结果；如果它 `Pending`了，就暂停自己（外层 `Future`）并把 `Pending` 状态报告给上级（执行器）。 它是在 async 状态机中，连接不同Future 的核心“胶水”。

每个 async 块都会生成一个实现了 Future trait 的匿名结构的类型；且类似闭包，可以像其他闭包一样传递  
async 块在语法和设计上借鉴了闭包的便利性，但其编译后的本质是一个状态机，即一个实现了 Future 的类型。
### 区分参数是函数还是闭包

```rust
// 1. 普通数据参数：明确的具体类型
fn example1(param: i32) {} // ← 普通参数

// 2. 函数指针参数：fn 关键字
fn example2(param: fn() -> ()) {} // ← 函数指针参数

// 3. 闭包参数：泛型 + Fn trait 约束
fn example3<F: Fn() -> ()>(param: F) {} // ← 闭包参数

// 4. 更复杂的闭包参数
fn example4<F>(param: F) 
where
    F: FnOnce(i32) -> bool + Send,
{} // ← 复杂的闭包参数
```

### async {}、fn async{}

async {} 块产生了一个实现了 `std::future::Future` Trait 的、匿名的、惰性的状态机结构体实例。  
1. 结构体实例. 当你写下 let my_future = async { ... }; 时，编译器会在后台为你自动生成一个隐藏的 struct，其中包含了状态机。这个 struct的字段用来保存 async 块在执行过程中需要的所有信息。  
async 块产生的那个匿名 `struct` 本身就是状态机。不存在一个 struct内部再嵌套一个独立的状态机数据结构的情况。
```rust
伪代码
struct __MyFuture_Anonymous_Struct<'a> {
      // 捕获的外部变量
      name: &'a str,

      // 内部的、跨 await 的局部变量
      greeting: Option<String>,

      // 记录执行进度的状态
      state: i32,

      // 存储当前正在 await 的 Future
      // (使用内存复用技术，如 union 或 enum)
      inner_future: ...,
```
async {} 表达式的返回值就是这个 struct 的一个实例，其 state 被初始化为起始状态。  
2. Anonymous  
你作为开发者，永远无法知道这个由编译器生成的结构体的真实名称，也无法直接写出它的类型。你只能通过 impl
Future 来泛指它，或者让编译器通过类型推断来处理   
3. A State Machine  
这个结构体的核心功能是作为一个状态机。async 块中的每一个 .await 都成为了一个潜在的状态。  
**状态：**保存在结构体的 state 字段中，通常是一个整数或 enum。这是它能够被 await 和被执行器驱动的关键。编译器会自动为这个生成的结构体实现 Future Trait，特别是 poll方法。这个 poll 方法的实现本质上是一个大的 match 语句，根据 state 字段来决定执行哪一段代码。  
4. lazy  
async {} 表达式在被求值时，仅仅是创建了这个状态机结构体的实例。它不会执行 async 块内部的任何一行代码。`async` 块中的代码只有在 `Future` 第一次被 `.await` 或被执行器 `poll` 时，才会开始执行。如果你创建了一个 async 块但从不 await 它，那么它里面的代码就永远不会运行，它就是一个“死”的 Future。  
在 `async` 块中，即使是同步代码，也同样是“懒惰的”（lazy），在 `Future` 被 `poll`之前，它们绝对不会被执行。同步代码的执行被绑定在了 Future 的生命周期中，成为了状态机的一部分。  
async 块中的同步代码和 await 表达式一样，都是懒惰的。它们都被打包进了 Future 的 poll方法中，等待着执行器来调用，从而启动和推进整个状态机。


一个async fn或async{}，会被编译器当作一个整体，编译器分析其内部所有的.await调用，将它们作为状态转换点，来构建单一的、统一的状态机。  
“每个 `.await`方法都会等待一个 `Future`” 和 “整个 `async fn` 会生成一个 `Future`”  
这两个陈述是完全兼容的。后者描述了容器，前者描述了容器在运转时如何处理其内容。这正是 Future可以被组合（composable）的强大之处。outerfuture驱动innerfuture

### .await

.await 的核心是实现非阻塞的等待。它允许一个任务在等待某个操作完成时，主动让出 CPU，以便其他任务可以执行。这个过程涉及到编译器、Future Trait和异步运行时的精密协作。 

当我们谈论 .await 时，整个过程可以分为两个层面：编译时发生的事和运行时发生的事。
#### 编译时

当编译器看到一个 async fn 时，它并不会把它编译成一个普通的函数。相反，它会执行一个重要的转换：将整个 `async` 函数转换成一个“状态机” (State Machine)。这个状态机是一个实现了 std::future::Future Trait 的 struct。
* 状态 (State): 函数中的每一个 .await 调用点，都成为了这个状态机的一个潜在的“暂停点”或“状态”简单来说，编译器帮你做了这些事：你的 async 函数代码被重写成了一个复杂的 struct 和一个 poll 方法的实现。poll 方法内部是一个巨大的 match 表达式，根据当前的状态（即上次在哪个 .await暂停的）来决定接下来执行哪一小段代码。 这就是为什么你的函数可以在暂停后恢复执行，并且所有局部变量都还存在——因为它们都保存在这个特殊 `struct` 的字段里。
### 运行时：poll、Waker 与执行器的协作

当你的代码运行时，.await 的真正魔力通过 Future Trait 的 poll 方法展现出来。这个过程由异步运行时 (Executor)（如 Tokio）来驱动。假设执行器正在 poll 你的 async 函数所生成的状态机 Future。当执行到 .await 这一行时，例如 my_future.await;，会发生以下步骤： 
#### 步骤 1: 轮询内部 Future   

.await关键字会调用 my_future 的 poll 方法。此时，它会将一个非常重要的参数 Context（其中包含一个 Waker）传递给 my_future。
#### 步骤 2: 检查 poll 的结果

 * 含义: my_future 已经计算完成，并且它的结果是 value。 
   情况 B: `Poll::Pending` 

 * 含义: my_future 现在无法完成它的计算（例如，正在等待网络数据到达，或等待一个定时器
 * 事件发生: 一段时间后，my_future 等待的事件发生了（例如，操作系统通知网络套接字上有新数据了）。.await 远不止是一个简单的函数调用。它是一个暂停点，是你的 async 函数与异步运行时进行交互的核心枢纽。它触发了一个“轮询-暂停-唤醒-恢复”的循环，通过将函数转换成状态机，并与执行器的 poll/Waker机制深度集成，最终实现了在单线程上高效调度大量并发任务的“魔法”。                       

## async实际的执行过程

在fn async、async{}中，存在同步和异步代码。如下：
 ```
  async {
      // --- State 0 Start ---
      println!("任务开始"); // 同步代码
      let x = 10;           // 同步代码
      let y = 20;           // 同步代码

      some_future.await; // 异步等待点

      // --- State 1 Start ---
      println!("第一个 future 完成"); // 同步代码
      let z = x + y;                // 同步代码

      another_future.await; // 异步等待点

      // --- State 2 Start ---
      println!("任务结束，结果: {}", z); // 同步代码
      // --- State 2 End (Future is Ready) ---
  }
```
这个 async 块生成的 Future 在被 poll 时，其行为如下：  
#### 第一次 poll  
1. Future 从初始状态（State 0）开始执行。假设第一次 poll 时 some_future 返回了 Pending，一段时间后 Future 被唤醒并再次 poll。
2.  Future 从它保存的状态开始，也就是准备再次 poll some_future。
3.  状态转换的“胶水”：这些同步代码可以看作是连接两个异步等待点（.await）的“胶水”。它们负责准备数据、处理结果，以及执行不涉及等待的纯计算。

**对性能的影响**：如果在两个 .await 之间放置了执行时间很长的同步代码（CPU-bound 操作），就会产生问题。因为在poll 执行这段同步代码期间，当前线程是被占用的，无法去 poll其他任务。这会“饿死”在同一个执行器线程上的其他异步任务，破坏异步编程的优势。
```
async {
          // ...
          first_future.await;

          // ！！！警告：这是一个长时间运行的同步操作 ！！！
          for _ in 0..1_000_000_000 {
              // 密集计算
          }

          second_future.await;
          // ...
      }
```
在上面的例子中，当 first_future 完成后，poll
方法会开始执行那个巨大的循环。在这个循环结束之前，当前线程完全被阻塞，无法响应任何 I/O事件或轮询其他任务。这正是 tokio::task::spawn_blocking 需要被用来处理这类问题的原因。  
结论：async 块中的非 await 表达式，就是在 Future 被 poll 时，作为状态转换的一部分被同步执行的普通 Rust代码。它们是构成异步逻辑流的必要组成部分，但需要注意避免在其中执行长时间的阻塞操作。

poll 的管理、开始和结束，完全由异步运行时（如 Tokio, async-std）的执行器来负责。Future本身是被动的，它从不主动 poll 自己。  
执行器本身并不严格限制单次`poll`的执行时长，但它依赖于一个“协作式”的约定，并通过一个“任务预算”（Task Budget）机制来防止单个任务长时间霸占线程。这涉及到runtime的实现原理

  

poll的调用方式？

### Racing Our Two URLs Against Each Other

```
extern crate trpl; // required for mdbook test，用于兼容旧版本的rust，现在已经不用了，主要使用use

use trpl::{Either, Html};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    trpl::run(async {
        let title_fut_1 = page_title(&args[1]);
        let title_fut_2 = page_title(&args[2]);

        let (url, maybe_title) =
            match trpl::race(title_fut_1, title_fut_2).await {
                Either::Left(left) => left,
                Either::Right(right) => right,
            };

        println!("{url} returned first");
        match maybe_title {
            Some(title) => println!("Its page title was: '{title}'"),
            None => println!("It had no title."),
        }
    })
}

async fn page_title(url: &str) -> (&str, Option<String>) {
    let response_text = trpl::get(url).await.text().await;
    let title = Html::parse(&response_text)
        .select_first("title")
        .map(|title| title.inner_html());
    (url, title)
}
```

async处理两个future，执行首先完成的那个。相对于使用match匹配最先完成的future。tokio::select!更加智能

## Applying Concurrency with Async

In this section we’ll focus on what’s different between threads and futures.  
In many cases, the APIs for working with concurrency using async are very similar to those for using threads.   
In other cases, they end up being quite different. Even when the APIs look similar between threads and async, **they often have different behavior—and they nearly always have different performance characteristics**.

```
use std::time::Duration;

fn main() {
    trpl::run(async {
        trpl::spawn_task(async {
            for i in 1..10 {
                println!("hi number {i} from the first task!");
                trpl::sleep(Duration::from_millis(500)).await;
            }
        });

        for i in 1..5 {
            println!("hi number {i} from the second task!");
            trpl::sleep(Duration::from_millis(500)).await;
        }
    });
}
```

**.await 只能在 async 块或者 async fn 中使用，且只能用于实现了future trait的类型中**

第1步: `main` 函数启动
  *   main 函数被操作系统调用，程序开始。


  第2步: `main` 调用 `trpl::run`
  *   main 函数执行 trpl::run(async { ... });。
     此时，`async { ... }` 块（Task A 的代码）并不会*被执行。它被编译器转换成一个 Future 对象。
  *   这个 Future 对象作为参数被传递给 trpl::run 函数。


  第3步: Executor (`trpl::run`) 初始化
  *   trpl::run 函数（我们的 Executor）开始执行。
  *   它接收到 Task A 的 Future，并将其放入自己的“待办任务”列表中。现在列表中只有一个任务。

  第4步: Executor 调度 Task A
  *   Executor 检查“待办任务”列表，发现 Task A，于是开始执行它。


  第5步: [Task A] 执行 `trpl::spawn_task`
  *   Task A 的第一行代码 trpl::spawn_task(async { ... }); 被执行。
  *   async { ... }（Task B 的代码）被转换成一个新的 Future 对象。
  *   trpl::spawn_task 函数将这个新的 Future (Task B) 交给 Executor。
  *   Executor 将 Task B 添加到自己的“待办任务”列表中。`trpl::spawn_task` 函数立即返回，它不会等待 Task B 执行。
  *   关于为什么`trpl::spawn_task` 函数立即返回，它不会等待 Task B 执行。因为，trpl::spawn_task 函数本身根本不关心 async 块里面有什么，也不会去 `poll` 它。spawn的唯一职责是把这个 async 块（它本质上是一个未启动的 Future）打包成一个“任务”，然后把它扔给“执行器”（Executor）的任务队列里，然后 spawn 函数本身就立即返回了。
  * 谨记，async{}中的所有代码，如果async{}调用者没有poll或.await，就绝对不会执行。

  第6步: [Task A] 进入 `for` 循环
  *   Task A 继续执行下一行，进入 for i in 1..5 循环，此时 i 等于 1。


  第7步: [Task A] 执行 `println!`
  *   Task A 执行 println!("hi number 1 from the second task!");。
     控制台输出:* hi number 1 from the second task!


  第8步: [Task A] 遇到 `.await` 并暂停
  *   Task A 执行 trpl::sleep(Duration::from_millis(500)).await;。
  *   trpl::sleep(...) 返回一个 SleepFuture。
  *   .await 关键字被触发。这相当于 Task A 对 Executor 说：“我要在这里等500毫秒，你先去忙别的吧。”
     Task A 的当前状态被保存，任务被标记为“正在等待”（Pending），控制权被交还给 Executor*。


  第9步: Executor 调度 Task B
  *   Executor 拿回控制权。它检查“待办任务”列表：
      *   Task A 正在等待，不能执行。
      *   Task B 处于就绪状态，可以执行。
  *   Executor 选择执行 Task B。

  第10步: [Task B] 进入 `for` 循环
  *   Task B 从头开始执行，进入 for i in 1..10 循环，此时 i 等于 1。


  第11步: [Task B] 执行 `println!`
  *   Task B 执行 println!("hi number 1 from the first task!");。
     控制台输出:* hi number 1 from the first task!


  第12步: [Task B] 遇到 `.await` 并暂停
  *   Task B 执行 trpl::sleep(Duration::from_millis(500)).await;。
     和 Task A 一样，Task B 也告诉 Executor 它需要等待，然后将控制权交还给 Executor*。


  第13步: Executor 等待
  *   Executor 再次拿回控制权。它检查“待办任务”列表：
      *   Task A 正在等待。
      *   Task B 正在等待。
  *   没有可以执行的任务了。Executor 进入休眠状态，等待有任务通过 Waker
  唤醒它。它已经设置了计时器，会在约500毫秒后被唤醒。

  (--- 大约500毫秒过去 ---)


  第14步: Executor 被唤醒，调度 Task A
  *   计时器到期，唤醒 Executor。Executor 知道 Task A 和 Task B 的等待都结束了，将它们都标记为“就绪”。
  *   Executor 选择一个任务执行（顺序不保证，我们假设它先选 Task A）。


  第15步: [Task A] 从 `.await` 处恢复
  *   Task A 从第8步暂停的地方继续。它的 sleep 已经完成。
  *   for 循环进入下一次迭代，i 变为 2。


  第16步: [Task A] 执行 `println!`
  *   Task A 执行 println!("hi number 2 from the second task!");。
     控制台输出:* hi number 2 from the second task!


  第17步: [Task A] 再次遇到 `.await` 并暂停
     Task A 再次执行 `trpl::sleep(...).await;`，将控制权交还给 Executor*。


  第18步: Executor 调度 Task B
  *   Executor 拿回控制权，发现 Task A 在等待，但 Task B 处于就绪状态。
  *   Executor 选择执行 Task B。

  第19步: [Task B] 从 `.await` 处恢复
  *   Task B 从第12步暂停的地方继续。
  *   for 循环进入下一次迭代，i 变为 2。


  第20步: [Task B] 执行 `println!`
  *   Task B 执行 println!("hi number 2 from the first task!");。
     控制台输出:* hi number 2 from the first task!


  第21步: [Task B] 再次遇到 `.await` 并暂停
     Task B 再次执行 `trpl::sleep(...).await;`，将控制权交还给 Executor*。


  第22步: 交替执行阶段
     从第13步到第21步的这个“等待 -> 唤醒 -> 执行Task A -> 暂停A -> 执行Task B -> 暂停B*”的模式会一直重复。
  *   这个过程会为 i = 3 和 i = 4 再发生两次。

  (--- 当 Task A 的循环结束后 ---)


  第23步: [Task A] 完成
  *   当 Task A 的 i 等于 4 的循环执行完毕并从 sleep 唤醒后，它的 for 循环结束。
  *   Task A 的 async 块中没有更多代码了。Task A 执行完毕。

  第24步: Executor 继续调度 Task B
  *   Executor 发现 Task A 已完成，但 Task B 还在“待办任务”列表中。
  *   Executor 会继续只调度 Task B。


  第25步: [Task B] 单独执行
  *   Task B 会继续它的循环，打印 i = 5, 6, 7, 8, 9。
     因为没有其他任务与它交替执行，所以它的执行流程会变成：打印 -> 睡眠500ms -> 唤醒 -> 打印 -> 睡眠500ms
  ...*

  第26步: [Task B] 完成
  *   当 Task B 的 i 等于 9 的循环执行完毕后，它的 async 块也执行完毕。


  第27步: Executor 退出
  *   Executor 检查“待办任务”列表，发现所有任务都已完成。
  *   trpl::run 函数执行完毕，返回。

  第28步: `main` 函数结束
  *   main 函数执行完毕，程序退出。

上述28步解释，前面的是对的，但是后期并不会完全执行1..9完整的循环  
这段代码在任何遵守基本异步执行模型（如 tokio, async-std等）的运行时上，第一个被打印出来的必定是 `hi number 1 from the second task!`。


This code behaves similarly to the thread-based implementation—including the fact that you may see the messages appear in a different order in your own terminal when you run it:  
```
hi number 1 from the second task!
hi number 1 from the first task!
hi number 2 from the first task!
hi number 2 from the second task!
hi number 3 from the first task!
hi number 3 from the second task!
hi number 4 from the first task!
hi number 4 from the second task!
hi number 5 from the first task!
```

the task spawned by spawn_task is shut down when the main function ends.  
If you want it to run all the way to the task’s completion, you will need to use a join handle to wait for the first task to complete. With threads, we used the join method to “block” until the thread was done running.   
Also,we can use await to do the same thing, because the task handle itself is a future. Its Output type is a Result, so we also unwrap it after awaiting it.
```
        let handle = trpl::spawn_task(async {
            for i in 1..10 {
                println!("hi number {i} from the first task!");
                trpl::sleep(Duration::from_millis(500)).await;
            }
        });

        for i in 1..5 {
            println!("hi number {i} from the second task!");
            trpl::sleep(Duration::from_millis(500)).await;
        }

        handle.await.unwrap();

```
it looks like async and threads give us the same basic outcomes, just with different syntax: using await instead of calling join on the join handle, and awaiting the sleep calls.  
The bigger difference is that we didn’t need to spawn another operating system thread to do this. In fact, we don’t even need to spawn a task here.  
Because async blocks compile to anonymous futures, we can put each loop in an async block and have the runtime run them both to completion using the trpl::join function.  
When you give it two futures, it produces a single new future whose output is a tuple containing the output of each future you passed in once they both complete.
```
        let fut1 = async {
            for i in 1..10 {
                println!("hi number {i} from the first task!");
                trpl::sleep(Duration::from_millis(500)).await;
            }
        };

        let fut2 = async {
            for i in 1..5 {
                println!("hi number {i} from the second task!");
                trpl::sleep(Duration::from_millis(500)).await;
            }
        };

        trpl::join(fut1, fut2).await;

```

> 首先由 trpl::run 产生主任务 task A，并同步执行 trpl::channel() 语句。
  >
  > 然后，代码定义了 tx_fut 和 rx_fut 这两个 Future 实例，此时并未创建新任务。
  >
  > 从 trpl::join(tx_fut, rx_fut).await; 语句开始，tx_fut 和 rx_fut 在 task A 的上下文中被并发驱动。
  >
  > 因为 poll 的顺序不保证，如果运行时首先 poll rx_fut，它将在 rx.recv().await 处因通道为空而返回
  Poll::Pending 并暂停。
  >
  > 随后，join 组合器会 poll tx_fut。tx_fut 在 tx.send() 调用后，会唤醒之前暂停的 rx_fut。接着，tx_fut 在
  trpl::sleep(...).await 处返回 Poll::Pending 并暂停。
  >
  > 由于 rx_fut 已被唤醒，调度器会优先再次 poll 它，使其能够接收并处理数据，然后再次因通道为空而暂停。这个由
   send 触发 wake 并导致 recv 被调度的循环会持续进行。
  >
  > 最终，该代码仍然会顺序打印 vec 中的字符串，这是由 channel 的 FIFO（先进先出）特性保证的。

we use trpl::join to wait for both fut1 and fut2 to finish. We do not await fut1 and fut2 but instead the new future produced by trpl::join. We ignore the output, because it’s just a tuple containing two unit values.
```
hi number 1 from the first task!
hi number 1 from the second task!
hi number 2 from the first task!
hi number 2 from the second task!
hi number 3 from the first task!
hi number 3 from the second task!
hi number 4 from the first task!
hi number 4 from the second task!
hi number 5 from the first task!
hi number 6 from the first task!
hi number 7 from the first task!
hi number 8 from the first task!
hi number 9 from the first task!

```
Now, you’ll see the exact same order every time, which is very different from what we saw with threads.  
That is because the trpl::join function is fair, meaning it checks each future equally often, alternating between them, and never lets one race ahead if the other is ready.  
With threads, the operating system decides which thread to check and how long to let it run.   
With async Rust, the runtime decides which task to check. (In practice, the details get complicated because an async runtime might use operating system threads under the hood as part of how it manages concurrency, so guaranteeing fairness can be more work for a runtime—but it’s still possible!)   
Runtimes don’t have to guarantee fairness for any given operation, and they often offer different APIs to let you choose whether or not you want fairness.

### Counting Up递增计数 on Two Tasks Using Message Passing

```
        let (tx, mut rx) = trpl::channel();

        let val = String::from("hi");
        tx.send(val).unwrap();

        let received = rx.recv().await.unwrap();
        println!("received '{received}'");

```

The async version of the API(trpl::channel) is only a little different from the thread-based version: it uses a mutable rather than an immutable receiver rx, and its recv method produces a future we need to await rather than producing the value directly. Now we can send messages from the sender to the receiver. Notice that we don’t have to spawn a separate thread or even a task; we merely need to await the rx.recv call.

The synchronous Receiver::recv method in std::mpsc::channel blocks until it receives a message. The trpl::Receiver::recv method does not, because it is async. Instead of blocking, it hands control back to the runtime until either a message is received or the send side of the channel closes. By contrast, we don’t await the send call, because it doesn’t block. It doesn’t need to, because the channel we’re sending it into is unbounded.
`
Note: Because all of this async code runs in an async block in a trpl::run call, everything within it can avoid blocking. However, the code outside it will block on the run function returning. That’s the whole point of the trpl::run function: it lets you choose where to block on some set of async code, and thus where to transition between sync and async code. In most async runtimes, run is actually named block_on for exactly this reason.
`
Notice two things about this example. First, the message will arrive right away. Second, although we use a future here, there’s no concurrency yet. Everything in the listing happens in sequence, just as it would if there were no futures involved.

```
        let (tx, mut rx) = trpl::channel();

        let vals = vec![
            String::from("hi"),
            String::from("from"),
            String::from("the"),
            String::from("future"),
        ];

        for val in vals {
            tx.send(val).unwrap();
            trpl::sleep(Duration::from_millis(500)).await;
        }

        while let Some(value) = rx.recv().await {
            println!("received '{value}'");
        }

```
In addition除了 to sending the messages, we need to receive them. In this case, because we know how many messages are coming in, we could do that manually by calling rx.recv().await four times.   
In the real world, though, we’ll generally be waiting on some unknown number of messages, so we need to keep waiting until we determine that there are no more messages.

Rust doesn’t yet have a way to write a for loop over an asynchronous series of items, however, so we need to use a loop we haven’t seen before: the while let conditional loop.  
The loop will continue executing as long as the pattern it specifies continues to match the value.

The rx.recv call produces a future, which we await. The runtime will pause the future until it is ready. Once a message arrives, the future will resolve to Some(message) as many times as a message arrives. When the channel closes, regardless of whether any messages have arrived, the future will instead resolve to None to indicate that there are no more values and thus we should stop polling—that is, stop awaiting.

The while let loop pulls all of this together. If the result of calling rx.recv().await is Some(message), we get access to the message and we can use it in the loop body, just as we could with if let. If the result is None, the loop ends. Every time the loop completes, it hits the await point again, so the runtime pauses it again until another message arrives.

The code now successfully sends and receives all of the messages.  
Unfortunately, there are still a couple of problems. For one thing, the messages do not arrive at half-second intervals间隔.此处不是没有执行half-second的发送间隔，而是接收方没有在500ms的间隔收到消息（由于通道缓冲区的存在）  
They arrive all at once, 2 seconds (2,000 milliseconds) after we start the program. For another, this program never exits! Instead, it waits forever for new messages. You will need to shut it down using ctrl-c.

Let’s start by examining why the messages come in all at once after the full delay, rather than coming in with delays between each one. **Within a given async block, the order in which await keywords appear in the code is also the order in which they’re executed when the program runs**.  
There’s still no concurrency. All the tx.send calls happen, interspersed with all of the trpl::sleep calls and their associated await points. Only then does the while let loop get to go through any of the await points on the recv calls.
```
   for val in vals {
            tx.send(val).unwrap();
            trpl::sleep(Duration::from_millis(500)).await;
        }

     while let Some(value) = rx.recv().await {
            println!("received '{value}'");
        }
```
对于上述，send之后立即执行sleep，此时只有这一个task，只有在for循环结束，才能改变状态机的状态进入下一步poll  
在 `for` 循环中，`.await` 暂停的是包含这个 `for`
循环的、当前这唯一的一个任务。因为没有创建任何其他的并发任务（比如通过`spawn`），所以执行器在当前任务暂停时，无事可做，只能等待。  
问题的关键不是 sleep 本身，而是程序的结构。你只定义了一个从头跑到尾的线性流程，即使这个流程中包含了异步的暂停点（.await），它也改变不了其线性的本质。  
要打破这种线性，实现并发，就必须使用像 spawn 这样的工具，显式地创建出多个可以被执行器独立调度的任务。这样，当一个任务暂停时，执行器才能找到另一个可运行的任务来填充 CPU 的空闲时间。

To get the behavior we want, where the sleep delay happens between each message, we need to put the tx and rx operations in their own async blocks
```
        let tx_fut = async {
            let vals = vec![
                String::from("hi"),
                String::from("from"),
                String::from("the"),
                String::from("future"),
            ];

            for val in vals {
                tx.send(val).unwrap();
                trpl::sleep(Duration::from_millis(500)).await;
            }
        };

        let rx_fut = async {
            while let Some(value) = rx.recv().await {
                println!("received '{value}'");
            }
        };

        trpl::join(tx_fut, rx_fut).await;

```
the messages get printed at 500-millisecond intervals, rather than all in a rush after 2 seconds.  
The program still never exits, though, because of the way the while let loop interacts with trpl::join:  
The future returned from trpl::join completes only once both futures passed to it have completed.  
The tx future completes once it finishes sleeping after sending the last message in vals.  
The rx future won’t complete until the while let loop ends.  
The while let loop won’t end until awaiting rx.recv produces None.  
Awaiting rx.recv will return None only once the other end of the channel is closed.  
The channel will close only if we call rx.close or when the sender side, tx, is dropped.  
We don’t call rx.close anywhere, and tx won’t be dropped until the outermost async block passed to trpl::run ends.  
The block can’t end because it is blocked on trpl::join completing, which takes us back to the top of this list.  

We could manually close rx by calling rx.close somewhere, but that doesn’t make much sense. Stopping after handling some arbitrary number of messages would make the program shut down, but we could miss messages. We need some other way to make sure that tx gets dropped before the end of the function.

Right now, the async block where we send the messages only borrows tx because sending a message doesn’t require ownership, but if we could move tx into that async block, it would be dropped once that block ends.  
`pub async fn send(&self, value: T) -> Result<(), SendError<T>>`(send 的签名是不可变引用)  
tx.send() 这个例子中，我们传递的只是一个指针大小的引用，这完全不属于上述任何一种需要担心的
overhead。因此，send 方法采用借用设计，是为了获得最大的灵活性和功能性，而几乎没有带来任何性能损失。

而， 我们使用 async move 是为了一个纯粹的所有权和生命周期管理的目标：确保 `tx` 在发送任务结束时被
`drop`，从而关闭通道。这是一个逻辑上的需求，而不是性能上的优化。
```
        let (tx, mut rx) = trpl::channel();

        let tx_fut = async move {
            let vals = vec![
                String::from("hi"),
                String::from("from"),
                String::from("the"),
                String::from("future"),
            ];

            for val in vals {
                tx.send(val).unwrap();
                trpl::sleep(Duration::from_millis(500)).await;
            }
        };

        let rx_fut = async {
            while let Some(value) = rx.recv().await {
                println!("received '{value}'");
            }
        };

        trpl::join(tx_fut, rx_fut).await;

```

This we run this version of the code, it shuts down gracefully after the last message is sent and received.

```
        let (tx, mut rx) = trpl::channel();

        let tx1 = tx.clone();
        let tx1_fut = async move {
            let vals = vec![
                String::from("hi"),
                String::from("from"),
                String::from("the"),
                String::from("future"),
            ];

            for val in vals {
                tx1.send(val).unwrap();
                trpl::sleep(Duration::from_millis(500)).await;
            }
        };

        let rx_fut = async {
            while let Some(value) = rx.recv().await {
                println!("received '{value}'");
            }
        };

        let tx_fut = async move {
            let vals = vec![
                String::from("more"),
                String::from("messages"),
                String::from("for"),
                String::from("you"),
            ];

            for val in vals {
                tx.send(val).unwrap();
                trpl::sleep(Duration::from_millis(1500)).await;
            }
        };

        trpl::join3(tx1_fut, tx_fut, rx_fut).await;

```
First, we clone tx, creating tx1 outside the first async block.   
Then, later, we move the original tx into a new async block, where we send more messages on a slightly slower delay.  
We happen to put this new async block after the async block for receiving messages, but it could go before it just as well. The key is the order in which the futures are awaited, not in which they’re created.  
Both of the async blocks for sending messages need to be async move blocks so that both tx and tx1 get dropped when those blocks finish. Otherwise, we’ll end up back in the same infinite loop we started out in. Finally, we switch from trpl::join to trpl::join3 to handle the additional future.  
```
received 'hi'
received 'more'
received 'from'
received 'the'
received 'messages'
received 'future'
received 'for'
received 'you'

```
## Working with Any Number of Futures

When we switched from using two futures to three in the previous section, we also had to switch from using join to using join3.注意join3是一个函数。It would be annoying to have to call a different function every time we changed the number of futures we wanted to join.   

instead：`trpl::join!(tx1_fut, tx_fut, rx_fut);`
This is definitely an improvement over swapping between join and join3 and join4 and so on!  
However, even实际上 this macro form only works when we know the number of futures ahead of time.In real-world Rust, though, pushing futures into a collection and then waiting on some or all the futures of them to complete is a common pattern.

To check all the futures in some collection, we’ll need to iterate over and join on all of them. The trpl::join_all function accepts any type that implements the Iterator trait
```
        let futures = vec![tx1_fut, rx_fut, tx_fut];

        trpl::join_all(futures).await;

```
this code doesn’t compile. This might be surprising. After all, none of the async blocks returns anything, so each one produces a `Future<Output = ()>`.  
Remember that Future is a trait, though, and that **the compiler creates a unique enum for each async block**.   
You can’t put two different hand-written structs in a Vec, and the same rule applies to the different enums generated by the compiler.  
To make this work, we need to use trait objects   
Using trait objects lets us treat each of the anonymous futures produced by these types as the same type, because all of them implement the Future trait.
`
Note: In Using an Enum to Store Multiple Values in Chapter 8, we discussed another way to include multiple types in a Vec: using an enum to represent each type that can appear in the vector. We can’t do that here, though. For one thing, we have no way to name the different types, because they are anonymous. For another, the reason we reached for a vector and join_all in the first place was to be able to work with a dynamic collection of futures where we only care that they have the same output type.
`
## Streams: Futures in Sequence

Many concepts are naturally represented as streams: items becoming available in a queue, chunks of data being pulled incrementally from the filesystem when the full data set is too large for the computer’s memory, or data arriving over the network over time.   
Because streams are futures, we can use them with any other kind of future and combine them in interesting ways.For example, we can batch up events批量处理事件 to avoid triggering too many network calls, set timeouts on sequences of long-running operations, or throttle user interface events to avoid doing needless work. 
```rust

use trpl::{ReceiverStream, Stream, StreamExt};

fn main() {
    trpl::run(async {
        let mut messages = get_messages();

        while let Some(message) = messages.next().await {
            println!("{message}");
        }
    });
}

fn get_messages() -> impl Stream<Item = String> {
    let (tx, rx) = trpl::channel();

    let messages = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    for message in messages {
        tx.send(format!("Message: '{message}'")).unwrap();
    }

    ReceiverStream::new(rx)
}
```
The new type: ReceiverStream, which converts the rx receiver from the trpl::channel into a Stream with a next method. Back in main, we use a while let loop to print all the messages from the stream.
未完

## A Closer Look at the Traits for Async

## The Future Trait

The Future trait  
```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```
First, Future’s associated type Output says what the future resolves to. This is analogous to the Item associated type for the Iterator trait.   
Second, Future also has the poll method, which takes a special Pin reference for its self parameter and a mutable reference to a Context type, and returns a `Poll<Self::Output>`
```rust
enum Poll<T> {
    Ready(T),
    Pending,
}
```

### The Pin and Unpin Traits

The cx parameter and its Context type are the key to how a runtime actually knows when to check any given future while still being lazy.

A type annotation for self works like type annotations for other function parameters, but with two key differences:  
1. It tells Rust what type self must be for the method to be called.
2. It can’t be just any type. It’s restricted to the type on which the method is implemented, a reference or smart pointer to that type, or a Pin wrapping a reference to that type.
#### Pin

Pin is a wrapper for pointer-like types such as &, &mut, Box, and Rc. (Technically, Pin works with types that implement the Deref or DerefMut traits, but this is effectively equivalent to working only with pointers.)   Pin is not a pointer itself and doesn’t have any behavior of its own like Rc and Arc do with reference counting; it’s purely a tool the compiler can use to enforce constraints on pointer usage.

Remember from earlier in this chapter a series of await points in(在一个future被编译为状态机中的await points ，不是await points被编译为状态机，await points 不会被编译为状态机) a future get compiled into a state machine, and the compiler makes sure that state machine follows all of Rust’s normal rules around safety, including borrowing and ownership.   
To make that work, Rust looks at what data is needed between one await point and either the next await point or the end of the async block. It then creates a corresponding variant in the compiled state machine. Each variant gets the access it needs to the data that will be used in that section of the source code, whether by taking ownership of that data or by getting a mutable or immutable reference to it.  
So far, so good: if we get anything wrong about the ownership or references in a given async block, the borrow checker will tell us. When we want to move around the future that corresponds to that block—like moving it into a Vec to pass to join_all—things get trickier.

When we move a future—whether by pushing it into a data structure to use as an iterator with join_all or by returning it from a function—that actually means moving the state machine Rust creates for us.  
And unlike most other types in Rust, the futures Rust creates for async blocks can end up with references to themselves in the fields of any given variant, as shown  
![](https://doc.rust-lang.org/book/img/trpl17-04.svg)  
A self-referential data type.

By default, though, any object that has a reference to itself is unsafe to move, because references always point to the actual memory address of whatever they refer to (see Figure 17-5). 
![](https://doc.rust-lang.org/book/img/trpl17-05.svg)
The unsafe result of moving a self-referential data type

If you move the data structure itself, those internal references will be left pointing to the old location. However, that memory location is now invalid. For one thing, its value will not be updated when you make changes to the data structure. For another—more important—thing, the computer is now free to reuse that memory for other purposes! You could end up reading completely unrelated data later.

Theoretically, the Rust compiler could try to update every reference to an object whenever it gets moved, but that could add a lot of performance overhead, especially if a whole web of references needs updating.  
If we could instead make sure the data structure in question doesn’t move in memory, we wouldn’t have to update any references. This is exactly what Rust’s borrow checker requires: in safe code, it prevents you from moving any item with an active reference to it.  
Pin builds on that to give us the exact guarantee we need. When we pin a value by wrapping a pointer to that value in Pin, it can no longer move. Thus, **if you have `Pin<Box<SomeType>>`, you actually pin the SomeType value, not the Box pointer**. Figure 17-6 illustrates this process.
![](https://doc.rust-lang.org/book/img/trpl17-06.svg)
Pinning a `Box` that points to a self-referential future type.

In fact, the Box pointer can still move around freely. **Remember: we care about making sure the data ultimately being referenced stays in place.**  
If a pointer moves around, but the data it points to is in the same place, as in Figure 17-7, there’s no potential problem.The key is that the self-referential type itself cannot move, because it is still pinned.
![](https://doc.rust-lang.org/book/img/trpl17-07.svg)
Moving a `Box` which points to a self-referential future type.

However, most types are perfectly safe to move around, even if they happen to be behind a Pin wrapper.   
We only need to think about pinning when items have internal references.   
Primitive values such as numbers and Booleans are safe because they obviously don’t have any internal references.Neither do most types you normally work with in Rust. You can move around a Vec, for example, without worrying.   
Given only what we have seen so far, if you have a `Pin<Vec<String>>`, you’d have to do everything via the safe but restrictive APIs provided by Pin, even though a `Vec<String>` is always safe to move if there are no other references to it. We need a way to tell the compiler that it’s fine to move items around in cases like this—and that’s where Unpin comes into play.

**`Pin` 不保证它指向的数据不被改变 (mutate)，它只保证数据不被移动 (move)。**
`Pin<&mut T>` 这个类型本身就暗示了这一点：它包裹的是一个 &mut T (可变引用)，其天性就是要允许改变。Pin
只是在这个“可变”的基础上，增加了一个“不可移动”的约束。  
但可变mutate，依然是存在风险的：  
对于一个自引用结构体，改变那些不参与自引用的字段是完全安全的。  
对于
```rust
self.data = "a new string".to_string();
```
这会导致 String 重新分配内存，self.data 的新内容会位于一个全新的内存地址。但 slice_ptr仍然指向旧的、已被释放的内存地址，变成了悬垂指针。  
对 `data` 执行会改变其内存地址的操作:  
   * Pin<&mut T> 不提供直接获取 &mut T 的安全方法。如果你能轻易拿到 &mut T，你就可以调用 std::mem::swap
     或者直接赋值，这等同于移动，Pin 的保证就被打破了。
   * 要从 Pin<&mut T> 获得 &mut T，你必须使用 unsafe：
  ```rust
      // 获取 Pin<&mut T>
      let pinned_ref: Pin<&mut T> = ...;

      // 必须在 unsafe 块中调用，并做出承诺
      let mutable_ref: &mut T = unsafe { Pin::get_unchecked_mut(pinned_ref) };
    ```
这里的 unsafe 关键字是你（程序员）在向编译器做出一个庄严的承诺：“我接下来的操作虽然是可变的，但我保证不会移动这块数据，也不会使其内部的任何指针失效。”
#### Unpin

Unpin is a marker trait, similar to the Send and Sync traits we saw in Chapter 16, and thus has no functionality of its own.Marker traits exist only to tell the compiler it’s safe to use the type implementing a given trait in a particular context.  
Unpin informs the compiler that a given type does not need to uphold any guarantees about whether the value in question can be safely moved.  
Just as with Send and Sync, the compiler implements Unpin automatically for all types where it can prove it is safe. A special case, again similar to Send and Sync, is where Unpin is not implemented for a type. The notation for this is impl !Unpin for SomeType, where SomeType is the name of a type that does need to uphold those guarantees to be safe whenever a pointer to that type is used in a Pin.

There are two things to keep in mind about the relationship between Pin and Unpin.  
1. First, Unpin is the “normal” case, and !Unpin is the special case. 
2. Second, whether a type implements Unpin or !Unpin only matters when you’re using a pinned pointer to that type like `Pin<&mut SomeType>`.

To make that concrete, think about a String: it has a length and the Unicode characters that make it up. We can wrap a String in Pin, as seen in Figure 17-8. However, String automatically implements Unpin, as do most other types in Rust.
![](https://doc.rust-lang.org/book/img/trpl17-08.svg)
Pinning a `String`; the dotted line虚线 indicates that the `String` implements the `Unpin` trait, and thus is not pinned.  

As a result, we can do things that would be illegal if String implemented !Unpin instead, such as replacing one string with another at the exact same location in memory as in Figure 17-9. This doesn’t violate the Pin contract, because String has no internal references(内部引用，即有self-reference的情况) that make it unsafe to move around! That is precisely why it implements Unpin rather than !Unpin.
![](https://doc.rust-lang.org/book/img/trpl17-09.svg)
Replacing the `String` with an entirely different `String` in memory.  
此时，假设string实现了pin，改变pin指向的数据仍然是安全合法的，因为其内部没有self-reference，不违背pin在自引用的情况下，保证指向数据不被移动的初衷

 1. 理解 Unpin 和 !Unpin
   * `Unpin`
   * `!Unpin` (读作 "Not Unpin")
  现在我们来看 String 类型。一个 String 结构体包含一个指向堆内存的指针 (ptr)、字符串的当前长度 (len)
  和堆内存的容量 (capacity)。


  关键点在于：String 的指针 (ptr) 指向的是外部的、堆上的内存，它没有指向 `String` 结构体自身的任何部分（比如
   len 或 capacity 字段）。


   * String (在栈上): [ptr, len, capacity]
  由于 String 内部没有自引用，所以移动一个 String 变量是安全的。移动时，只是把栈上的 [ptr, len, capacity]
  这三个值复制到新的位置，指针 ptr 仍然指向同一块堆内存。因此，`String` 实现了 `Unpin`。

   3. 解释引文中的操作

  > ...我们可以做一些事情，比如在内存中的完全相同的位置用另一个字符串替换一个字符串...

  这句话描述的操作是这样的：

  `rust
  let mut s1 = Box::pin("hello".to_string()); // 我们 Pin 住一个 String

  // 用一个新的 String 替换掉被 Pin 住的旧 String
  // 这实际上是一个 move 操作：new_string 被 move 到了 s1 原来的位置
  // 旧的 "hello" String 被 drop
  *s1 = "world".to_string();

  println!("{}", s1); // 输出 "world"
  `

  这个操作看起来违反了 Pin 的“不可移动”原则，但它是被允许的。


  > ...这没有违反 Pin 的契约，因为 String 没有使其移动变得不安全的内部引用！...


  这里的核心逻辑是：
  1. Pin 的主要目的是保护那些 !Unpin 的类型（比如自引用的 Future），防止它们被移动。
  2. 但对于 String 这种 Unpin 的类型，移动本身就是安全操作。
  3. 因此，Pin 对 Unpin 的类型提供了一个“后门”：如果 T 是 Unpin，那么你可以安全地从 Pin<&mut T>
  得到一个普通的 &mut T。
  4. 一旦你拿到了 &mut String，你就可以对它做任何 &mut String 能做的事情，包括用 = 来替换它的值。
  5. 这个替换操作之所以不违反 Pin 的契约，是因为 Pin 的契约本质上是“不要做任何会使内部指针失效的操作”。对于
  String 来说，移动它或者替换它，都不会产生内部指针失效的问题，所以 Pin 就允许了这些操作。

  4. 反向思考：如果 String 是 !Unpin

  现在我们来理解引文的第一部分：


  > ...如果 String 实现 !Unpin，这些事情就是非法的...


  让我们假设 String 是一个自引用结构体，因此它是 !Unpin 的。那么：
  1. 当你有一个 Pin<&mut String> 时，你将无法安全地从中获得一个 &mut String。
  2. 因此，*s1 = "world".to_string(); 这行代码将无法通过编译。
  3. 编译器会阻止你，因为它知道对一个 !Unpin 的类型进行赋值（移动）是危险的，可能会破坏其内部的自引用。

  总结  
  这句话的完整逻辑链是：
  1. String 的内部结构没有自引用。
  2. 因此，移动 String 是安全的。
  3. 因此，String 被标记为 `Unpin`。
  4. 因为 String 是 Unpin，Pin 对它的限制就放宽了，允许我们从 Pin<&mut String> 安全地获得 &mut String。
  5. 因此，我们可以对一个被 Pin 住的 String 进行替换（一个 move 操作），这不会违反 `Pin` 的核心安全保证。
  6. 反之，如果一个类型（如 Future 状态机）是 !Unpin，Pin 就会严格禁止这种替换/移动操作，以保证内存安全。

Now we know enough to understand the errors reported for that join_all call from back in Listing 17-17. We originally tried to move the futures produced by async blocks into a `Vec<Box<dyn Future<Output = ()>>>`, but as we’ve seen, those futures may have internal references, so they don’t implement Unpin. They need to be pinned, and then we can pass the Pin type into the Vec, confident that the underlying data in the futures will not be moved.

Note: This combination of Pin and Unpin makes it possible to safely implement a whole class of complex types in Rust that would otherwise prove challenging because they’re self-referential.   
Types that require Pin show up most commonly in async Rust today, but every once in a while, you might see them in other contexts, too.  
The specifics of how Pin and Unpin work, and the rules they’re required to uphold, are covered extensively in the API documentation for std::pin, so if you’re interested in learning more, that’s a great place to start.  
If you want to understand how things work under the hood in even more detail, see Chapters 2 and 4 of Asynchronous Programming in Rust.

### The Stream Trait

Stream has no definition in the standard library as of this writing, but there is a very common definition from the futures crate used throughout the ecosystem.

From Iterator, we have the idea of a sequence: its next method provides an `Option<Self::Item>`.   
From Future, we have the idea of readiness over time: its poll method provides a `Poll<Self::Output>`. 

To represent a sequence of items that become ready over time, we define a Stream trait that puts those features together:
```rust
#![allow(unused)]
fn main() {
use std::pin::Pin;
use std::task::{Context, Poll};

trait Stream {
    type Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>>;
}
}
```
This is similar to Iterator, where there may be zero to many items, and unlike Future, where(这里指代future) there is always a single Output, even if it’s the unit type ().  
Stream also defines a method to get those items. We call it poll_next, to make it clear that it polls in the same way Future::poll does and produces a sequence of items in the same way




































































































# Asynchronous Programming in Rust

```rust
pub trait Future {
    type Output;

    // Required method
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

## Task、future

Each time a future is polled, it is polled as part of a "task". Tasks are the top-level futures that have been submitted to an executor.  
Future的产生：  
async {}、async fn {}，产生的是实现了 std::future::Future Trait  的值（一个匿名结构体，本质上是个状态机）。且此时是lazy的，除非调用poll或.await，否则不会执行。  

Task：  
tokio::spawn(my_future); // 假设我们正在一个 tokio 运行时环境中   
会产生一个Task，并发生  
1. 所有权转移 (Ownership Transfer): my_future 的所有权被移交 (move) 给了spawn 函数。从这一刻起，你不能再直接使用 my_future这个变量了。它现在完全由运行时来管理。
2. 包装与调度 (Wrapping and Scheduling): 成为执行单元: 它不再只是一个被动的计划，而是成为了一个调度器可以识别和管理的、独立的、并发的执行单元。  






## async和ownership、lifetime