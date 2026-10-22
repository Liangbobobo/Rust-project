- [Tokio](#tokio)
  - [overview](#overview)
    - [Advantages of Tokio](#advantages-of-tokio)
      - [Fast and scalable](#fast-and-scalable)
      - [Reliable](#reliable)
      - [Easy](#easy)
      - [Flexible](#flexible)
    - [Tokio's role in your project](#tokios-role-in-your-project)
    - [When not to use Tokio](#when-not-to-use-tokio)
  - [tokio的多线程和单线程](#tokio的多线程和单线程)
  - [Setup](#setup)
  - [Hello Tokio](#hello-tokio)
    - [toml文件依赖和其他文件中use的命名不同之处](#toml文件依赖和其他文件中use的命名不同之处)
    - [解构代码](#解构代码)
      - [tokio::join!（Future并发）和tokio::spawn（Task并发）的本质区别](#tokiojoinfuture并发和tokiospawntask并发的本质区别)
    - [What is asynchronous programming?](#what-is-asynchronous-programming)
    - [Compile-time green-threading](#compile-time-green-threading)
    - [Async main function](#async-main-function)
    - [Cargo features](#cargo-features)
  - [Spawning](#spawning)
    - [Accepting sockets](#accepting-sockets)
      - [GOT的输出与unwrap()的错误传播](#got的输出与unwrap的错误传播)
      - [避免使用unwrap](#避免使用unwrap)
      - [frame和bytes](#frame和bytes)
      - [目录examples和cargo --example](#目录examples和cargo---example)
    - [Tasks](#tasks)
    - ['static bound](#static-bound)
      - [关于\&`static T、T:`static和\&·static mut r、mut r:\&'static T](#关于static-ttstatic和static-mut-rmut-rstatic-t)
        - [gemini的解释](#gemini的解释)
        - [\&\`static mut r、mut r:\&'static T](#static-mut-rmut-rstatic-t)
          - [\&\`static mut r、mut r:\&'static T中的\&T为什么是共享引用](#static-mut-rmut-rstatic-t中的t为什么是共享引用)
    - [Send bound](#send-bound)
    - [Store values](#store-values)
  - [Shared state](#shared-state)
    - [Strategies](#strategies)
      - [Update process()](#update-process)
    - [Tasks, threads, and contention](#tasks-threads-and-contention)
  - [Channels](#channels)
    - [Message passing](#message-passing)
      - [Tokio's channel primitives](#tokios-channel-primitives)
    - [mpsc](#mpsc)
    - [Spawn manager task](#spawn-manager-task)
    - [Receive responses](#receive-responses)
  - [I/O](#io)
    - [AsyncRead and AsyncWrite](#asyncread-and-asyncwrite)
      - [async fn read()](#async-fn-read)
      - [async fn read\_to\_end()](#async-fn-read_to_end)
      - [async fn write()](#async-fn-write)
      - [async fn write\_all()](#async-fn-write_all)
  - [Framing](#framing)
    - [Buffered reads](#buffered-reads)
      - [Parsing](#parsing)
  - [Async in depth](#async-in-depth)
    - [Futures](#futures)
    - [Executors](#executors)
      - [Mini Tokio](#mini-tokio)
  - [Bridging with sync code](#bridging-with-sync-code)
    - [What `#[tokio::main]` expands to](#what-tokiomain-expands-to)
    - [A synchronous interface to mini-redis](#a-synchronous-interface-to-mini-redis)
    - [Other approaches](#other-approaches)
      - [Spawning things on a runtime](#spawning-things-on-a-runtime)
        - [代码核心展示](#代码核心展示)
        - [代码逐段详解](#代码逐段详解)
          - [1. 创建运行时](#1-创建运行时)
          - [2. 派生后台任务](#2-派生后台任务)
          - [3. 主线程执行同步任务](#3-主线程执行同步任务)
          - [4. 等待所有任务完成](#4-等待所有任务完成)
          - [5. 异步任务函数](#5-异步任务函数)
        - [详细执行顺序](#详细执行顺序)
        - [总结](#总结)
  - [Graceful Shutdown](#graceful-shutdown)
    - [Figuring out when to shut down](#figuring-out-when-to-shut-down)
    - [Telling things to shut down](#telling-things-to-shut-down)
    - [Waiting for things to finish shutting down](#waiting-for-things-to-finish-shutting-down)
  - [Getting started with Tracing](#getting-started-with-tracing)
    - [Setup](#setup-1)
    - [Subscribing to Traces](#subscribing-to-traces)
    - [Subscriber Configuration](#subscriber-configuration)
  - [扩展-关于`tokio={version="1",feature=["full"]}`](#扩展-关于tokioversion1featurefull)
      - [1. 核心运行时 (Runtime)](#1-核心运行时-runtime)



# Tokio

**资料：**  
官方文档：https://tokio.rs/tokio/tutorial  
Tokio maintainer ：https://ryhl.io/  
社区：  
https://discord.com/invite/tokio   
https://github.com/tokio-rs/tokio/discussions

## overview  

It provides the building blocks needed for writing networking applications.  
It gives the flexibility to target a wide range of systems, from large servers with dozens of cores to small embedded devices.

At a high level, Tokio provides a few major components:
1. A multi-threaded runtime for executing asynchronous code.
2. An asynchronous version of the standard library.
3. A large ecosystem of libraries.
### Advantages of Tokio

特性方面值得牢记，可以根据自己的需求发挥Tokio的长处
#### Fast and scalable

Tokio is scalable, built on top of the async/await language feature, which itself is scalable. 
#### Reliable

#### Easy

Tokio follows the standard library's naming convention when it makes sense. This allows easily converting code written with only the standard library to code written with Tokio. With the strong type system of Rust, the ability to deliver correct code easily is unparalleled无与伦比的、空前的.
#### Flexible

Tokio provides multiple variations of the runtime. Everything from a multi-threaded, work-stealing runtime to a light-weight, single-threaded runtime. 



### Tokio's role in your project

When writing asynchronous code, you cannot use the ordinary blocking APIs provided by the Rust standard library, and must instead use asynchronous versions of them. These alternate versions are provided by Tokio, mirroring the API of the Rust standard library where it makes sense.

### When not to use Tokio

Tokio is not a good fit：  
1. Speeding up加速 CPU-bound computations by running them in parallel on several threads.  
Tokio is designed for IO-bound applications where each individual task spends most of its time waiting for IO.   
**If the only thing your application does is run computations in parallel, you should be using rayon.**   
That said, it is still possible to "mix & match" if you need to do both.（https://ryhl.io/blog/async-what-is-blocking/#the-rayon-crate）
2. Reading a lot of files.   
Although it seems like Tokio would be useful for projects that simply need to read a lot of files, Tokio provides no advantage here compared to an ordinary threadpool. This is because operating systems generally do not provide asynchronous file APIs.？how about windows and liunx
3. Sending a single web request.   
The place where Tokio gives you an advantage is when you need to do many things at the same time.  
If you need to use a library intended for asynchronous Rust such as reqwest, but you don't need to do a lot of things at once, you should prefer the blocking version of that library, as it will make your project simpler.   
Using Tokio will still work, of course, but provides no real advantage over the blocking API. If the library doesn't provide a blocking API,

## tokio的多线程和单线程

 特性 | 多线程运行时 (Multi-Thread) | 单线程运行时 (Current-Thread) |
  | :--- | :--- | :--- |
  | 启用方式 | #[tokio::main] (默认) | #[tokio::main(flavor = "current_thread")] |
  | 线程模型 | 创建一个 OS 线程池 (N 个线程) | 使用当前的单个 OS 线程 (1 个线程) |
  | Task 跨线程? | 会，通过工作窃取机制 | 不会，所有 Task 都在同一线程 |
  | `Send` 约束 | 强制要求 | 通常可以配合 LocalSet 放宽 |
  | 性能特点 | 高吞吐量，充分利用多核 | 低调度开销，有时延迟更低 |
  | 主要用途 | 高性能网络服务 | 嵌入式、与非 Send 数据交互 |


## Setup

实现一个简单的redis（mini-redis）  
source：https://github.com/tokio-rs/mini-redis

production-ready Redis也有rust的实现，可以用于数据库的中间层
## Hello Tokio

### toml文件依赖和其他文件中use的命名不同之处

在tomal文件中dependencies中如果有-，如mini-redis  
在其他文件中引入这个依赖时，需要将-改为_  
如mini_redis  
或在toml文件中，使用`redis = { package = "mini-redis", version = "0.4" }`,去掉连字符，在引入依赖时使用redis这个名字  
因为：  
运算符不能成为标识符的一部分
### 解构代码

```rust
use mini_redis::{client, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Open a connection to the mini-redis address.
    let mut client = client::connect("127.0.0.1:6379").await?;

    // Set the key "hello" with value "world"
    client.set("hello", "world".into()).await?;

    // Get key "hello"
    let result = client.get("hello").await?;

    println!("got value from the server; result={:?}", result);

    Ok(())
}
```
`let mut client = client::connect("127.0.0.1:6379").await?;`  
The client::connect function is provided by the mini-redis crate.   
The client::connect function asynchronously establishes a TCP connection with the specified remote address. Once the connection is established, a client handle is returned.   
上述虽然是异步代码，但并未使用task等实现真正的异步执行，因为逻辑上set需要等待clientconnect建立后才能执行，而get又需要set执行后才能执行  
上述代码是任务内串行（Intra-Task Sequentiality），而tokio::spawn用于创建新的并发任务（New Concurrent Task），实现任务间并发（Inter-Task Concurrency）的。下面是使用task的并发：  
```rust
 以下代码将创建两个并发任务。每个任务都会在开始和结束时打印信息，并在中间通过tokio::time::sleep（一个异步操作
  ）让出控制权。这将清晰地展示当一个任务在.await处挂起时，另一个任务是如何获得执行机会的。

    1 use std::time::Duration;
    2 use tokio::time::sleep;
    3
    4 #[tokio::main]
    5 async fn main() {
    6     println!("Main: Spawning tasks...");
    7
    8     // 创建第一个任务。它将睡眠200毫秒。
    9     let task_a = tokio::spawn(async {
   10         println!("Task A: Started. Will sleep for 200ms.");
   11         // 在.await处，Task A将让出控制权
   12         sleep(Duration::from_millis(200)).await;
   13         println!("Task A: Finished.");
   14     });
   15
   16     // 创建第二个任务。它将睡眠100毫秒。
   17     // main函数不会等待task_a，而是立即spawn task_b
   18     let task_b = tokio::spawn(async {
   19         println!("Task B: Started. Will sleep for 100ms.");
   20         // 在.await处，Task B也将让出控制权
   21         sleep(Duration::from_millis(100)).await;
   22         println!("Task B: Finished.");
   23     });
   24
   25     println!("Main: All tasks spawned. Waiting for completion...");
   26
   27     // 等待两个任务都执行完毕
   28     // 如果不等待，main函数可能会提前退出，导致spawn的任务被终止
   29     let _ = tokio::join!(task_a, task_b);
   30
   31     println!("Main: All tasks completed.");
   32 }

  预期输出

  您会观察到类似下面这样的输出（由于调度的不确定性，前两行和后两行的顺序可能微调，但中间部分的顺序是关键）：

   1 Main: Spawning tasks...
   2 Task A: Started. Will sleep for 200ms.
   3 Task B: Started. Will sleep for 100ms.
   4 Main: All tasks spawned. Waiting for completion...
   5 Task B: Finished.
   6 Task A: Finished.
   7 Main: All tasks completed.

  输出的底层解析

   1. main函数打印 "Spawning tasks..."。
   2. tokio::spawn创建task_a。执行器开始轮询task_a。
   3. task_a打印 "Started..."，然后遇到sleep(...).await。sleep
      Future返回Poll::Pending，并将一个Waker注册到Tokio的计时器子系统。task_a被挂起。
   4. 由于task_a被挂起，执行器可以继续执行main函数（或者其他就绪任务）。
   5. main函数继续执行，tokio::spawn创建task_b。执行器开始轮询task_b。
   6. task_b打印 "Started..."，然后遇到它的sleep(...).await。它同样返回Poll::Pending并被挂起。
   7. main函数继续执行，打印 "All tasks
      spawned..."，然后遇到tokio::join!。main函数自己也被挂起，等待task_a和task_b完成。
   8. 关键点: 此时，执行器有两个挂起的任务task_a和task_b。大约100毫秒后，计时器通知task_b的Waker。
   9. task_b被唤醒，再次被轮询时，其sleep Future返回Poll::Ready。task_b继续执行，打印 "Finished."，然后任务完成。
   10. 大约又过了100毫秒（总共200毫秒），计时器通知task_a的Waker。
   11. task_a被唤醒，其sleep Future返回Poll::Ready。task_a继续执行，打印 "Finished."，然后任务完成。
   12. task_a和task_b都完成后，main函数中的tokio::join!
       Future也完成，main函数继续执行并打印最后一条信息，程序结束
```
另一种情况,不使用task：
```rust
1 use std::time::Duration;
    2 use tokio::time::sleep;
    3
    4 #[tokio::main]
    5 async fn main() {
    6     println!("Main: Defining futures...");
    7
    8     // 定义第一个Future。注意，此时它还未开始执行。
    9     let future_a = async {
   10         println!("Future A: Execution started. Will sleep for 200ms.");
   11         sleep(Duration::from_millis(200)).await;
   12         println!("Future A: Finished.");
   13     };
   14
   15     // 定义第二个Future。
   16     let future_b = async {
   17         println!("Future B: Execution started. Will sleep for 100ms.");
   18         sleep(Duration::from_millis(100)).await;
   19         println!("Future B: Finished.");
   20     };
   21
   22     println!("Main: Driving futures concurrently with tokio::join!...");
   23
   24     // tokio::join! 会并发地轮询 future_a 和 future_b。
   25     // main任务会在此处挂起，直到两个future都完成。
   26     tokio::join!(future_a, future_b);
   27
   28     println!("Main: All futures completed.");
   29 }

   // output

   1 Main: Defining futures...
   2 Main: Driving futures concurrently with tokio::join!...
   3 Future A: Execution started. Will sleep for 200ms.
   4 Future B: Execution started. Will sleep for 100ms.
   5 Future B: Finished.
   6 Future A: Finished.
   7 Main: All futures completed.
```
 底层机制解析tokio::join!宏是如何在不创建新任务的情况下实现并发的？  

   1. 宏的展开: tokio::join!(fut1, fut2)宏在编译时会生成一个特殊的、匿名的“主Future”。这个主Future包含了它所要驱动
      的所有子Future（future_a和future_b）。

   2. 并发轮询（Concurrent Polling）: 当执行器poll这个由join!生成的主Future时，它的内部逻辑大致如下：
       * 轮询future_a。
       * 如果future_a返回Poll::Pending（因为它在sleep），join!并不会立刻返回`Pending`。
       * 它会继续轮询future_b。
       * 如果future_b也返回Poll::Pending（因为它也在sleep），那么此时join!生成的主Future才会向执行器返回Poll::Pen
         ding。

   3. 执行权的转移与恢复:
       * future_a和future_b在sleep时都注册了各自的Waker。
       * 当future_b的100ms计时器先到期时，它的Waker被调用，执行器被告知main任务已就绪。
       * 执行器再次poll main任务，进而poll join!的主Future。
       * join!再次轮询future_a（它仍然是Pending），然后轮询future_b。此时future_b的sleep已完成，它会继续执行直到
         完成，并返回Poll::Ready。
       * join!记录下future_b已完成，但由于future_a仍未完成，join!主Future整体还是返回Poll::Pending。
       * 当future_a的200ms计时器到期，上述过程重复，future_a最终也执行完毕。
       * 直到所有子Future都返回Poll::Ready，join!生成的主Future才会向执行器返回Poll::Ready，main函数得以继续执行
         。

  这种机制确保了只要join!内的任何一个Future可以取得进展，它就会被驱动，从而实现了在单一任务内的并发。

#### tokio::join!（Future并发）和tokio::spawn（Task并发）的本质区别

  1. 调度模型与执行单元 (Scheduling Model & Execution Unit)

  这是两者最根本的区别，它决定了运行时如何看待和处理这些并发操作。

   * `tokio::join!`:
       * 单一执行单元: 所有被join!的Future都属于同一个父任务。对于Tokio的调度器（Scheduler）而言，它只看到了一个
         需要被调度的单元——即包含join!的那个任务。
       * 内部轮询: join!宏创建了一个“元Future”，当这个元Future被执行器poll时，它会在内部依次poll所有子Future。这
         些子Future的执行是紧耦合的。如果父任务因为某种原因（如优先级、调度策略）得不到执行，那么它内部的所有子Fu
         ture都会一起“挨饿”。

   * `tokio::spawn`:
       * 独立执行单元: spawn会创建一个全新的、独立的顶层任务。每个被spawn出的任务都和创建它的父任务一样，是调度器
         眼中的一个“一级公民”。
       * 调度器驱动: 这些任务被提交到调度器的全局或工作线程的本地队列中，由调度器自主决定何时以及在哪个物理线程上
         poll它们。它们的执行是松耦合的，一个任务的阻塞或高计算负载不会直接阻止其他任务被调度和执行（尤其是在多线
         程运行时中）。


  2. 错误处理与恐慌隔离 (Error Handling & Panic Isolation)

  系统健壮性的关键在于故障隔离。

   * `tokio::join!`:
       * 无隔离: 所有Future在同一个任务的上下文中执行。如果其中任何一个Future内部发生了panic，它会像普通的同步代
         码一样导致整个任务panic，进而可能终止当前线程或整个程序。
       * 短路行为 (Short-circuiting): 如果join!等待的多个Future都返回Result，一旦其中一个返回Err，join!会立即返回
         这个Err，而不再继续轮询其他尚未完成的Future。

   * `tokio::spawn`:
       * 恐慌边界 (Panic Boundary): 每个Task都是一个恐慌隔离区。如果一个spawn出的任务panic了，Tokio运行时会捕获这
         个panic，并终止该任务，但不会影响其他任务（包括创建它的父任务）。父任务在.await对应的JoinHandle时会收到
         一个Err，表明其子任务因panic而异常终止。这提供了强大的故障隔离能力。
       * 错误独立: 一个任务返回Err仅仅是它自身的结果，不会直接影响其他并发任务的执行。

  3. 所有权与生命周期 (Ownership & Lifetimes)

  这是Rust语言特性带来的核心差异，直接影响代码的写法。

   * `tokio::join!`:
       * 可借用局部数据: 由于所有Future都在当前函数的作用域内被等待和完成，它们可以安全地借用当前函数的局部变量。
         Rust的借用检查器可以静态地保证被借用的数据活得比Future更久。这使得编码非常灵活。

   * `tokio::spawn`:
       * `'static`生命周期约束: spawn出的任务可能会比创建它的函数活得更久（父函数执行完毕，子任务仍在后台运行）。
         因此，Rust编译器强制要求被spawn的Future必须拥有'static生命周期。
       * 所有权转移: 这意味着该Future不能包含任何对父函数栈上数据的引用。所有需要的数据都必须通过所有权转移（move
         ）的方式进入Future。如果多任务需要共享数据，则必须使用Arc等原子引用计数类型来包裹。

  4. 资源管理与取消 (Resource Management & Cancellation)

   * `tokio::join!`:
       * 结构化并发 (Structured Concurrency): join!代表了一种结构化的并发模型。父任务的生命周期与子Future的生命周
         期是绑定的。如果父任务被取消（例如，它位于一个select!宏的某个分支中，而另一个分支先完成了），所有被join!
         的子Future也会被立即drop，从而被隐式取消。

   * `tokio::spawn`:
       * 非结构化并发 (Unstructured Concurrency): spawn创建的任务是独立的。父任务可以“发射后不管”（fire-and-forge
         t），不等待JoinHandle。即使父任务结束，子任务依然会继续运行。
       * 显式取消:
         spawn返回的JoinHandle上有一个.abort()方法，允许你从程序的任何地方显式地、异步地取消一个正在运行的任务。

  总结

  为了更直观地对比，请看下表：

   1 <table><tr><th>分析维度</th><th><code>tokio::join!</code> (任务内并发)</th><th><code>tokio::spawn
     </code> (任务间并发)</th></tr><tr><td><strong>调度模型</strong></td><td>
     单一调度单元，内部轮询，执行紧耦合。</td><td>多个独立调度单元，调度器驱动，执行松耦合。</td></tr>
     <tr><td><strong>故障隔离</strong></td><td>无。一个<code>panic</code>会终止整个任务。</td><td>
     有。任务是<code>panic</code>的边界，不影响其他任务。</td></tr><tr><td><strong>生命周期</strong></
     td><td>可借用局部变量，无需<code>'static</code>。</td><td>必须为<code>'static</code>
     ，数据需转移所有权或使用<code>Arc</code>。</td></tr><tr><td><strong>并发结构</strong></td><td>
     结构化并发，生命周期绑定。</td><td>非结构化并发，生命周期解耦，支持显式取消。</td></tr><tr><td><
     strong>性能开销</strong></td><td>极低。本质是单个<code>Future</code>内的状态机转换。</td><td>
     有少量开销（任务元数据分配、调度器交互），但对多数I/O任务可忽略。</td></tr></table>

  核心思想:
   * 使用tokio::join!当你有一组固定的、需要共同完成才能继续下一步的异步操作时，特别是当它们需要共享本地栈上数据时
     。这被称为结构化并发。
   * 使用tokio::spawn当你需要创建独立的、生命周期解耦的后台工作，或者需要动态地管理一组任务，或者需要故障隔离时。
     这被称为非结构化并发。


### What is asynchronous programming?

Most computer programs are executed in the same order in which they are written. The first line executes, then the next, and so on.  
With synchronous programming, when a program encounters an operation that cannot be completed immediately, it will block until the operation completes. For example, establishing a TCP connection requires an exchange with a peer over the network, which can take a sizeable amount of time. During this time, the thread is blocked.  
With asynchronous programming, operations that cannot complete immediately are suspended暂停 to the background. The thread is not blocked, and can continue running other things. Once the operation completes, the task is unsuspended and continues processing from where it left off离开. **Our example from before only has one task, so nothing happens while it is suspended, but asynchronous programs typically have many such tasks.** 

### Compile-time green-threading

Rust transforms the async fn at compile time into a routine惯例的、常规的 that operates asynchronously.  
Any calls to .await within the async fn yield control back **to the thread**. The thread may do other work while the operation processes in the background.  
Rust's async operations are lazy.

Async functions are called like any other Rust function. However, **calling these functions does not result in the function body executing. Instead, calling an async fn returns a value representing the operation.**  
This is conceptually analogous to a zero-argument closure. To actually run the operation, you should use the .await operator on the return value.  
**The return value of an async fn is an anonymous type that implements the Future trait.**
### Async main function

1. It is an async fn
2. It is annotated with `#[tokio::main]`

An async fn is used as we want to enter an asynchronous context. However, asynchronous functions must be executed by a runtime.   
The runtime contains the **asynchronous task scheduler, provides evented I/O, timers, etc.** The runtime does not automatically start, so the main function needs to start it.

The `#[tokio::main]` function is a macro. It transforms the async fn main() into a synchronous fn main() that initializes a runtime instance and executes the async main function.
```rust
#[tokio::main]
async fn main() {
    println!("hello");
}

// transform into
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    })
}
```
### Cargo features

`tokio = { version = "1", features = ["full"] }`  

[扩展-关于`tokio={version="1",feature=["full"]}`](#扩展-关于tokioversion1featurefull)
## Spawning

We are going to shift gears and start working on the Redis server.  
First, move the client SET/GET code from the previous section to an example file. This way, we can run it against our server.
```rust
$ mkdir -p examples
$ mv src/main.rs examples/hello-redis.rs
cd src
ni mian.rs // powershell创建新文件
```


### Accepting sockets

The first thing our Redis server needs to do is to accept inbound TCP sockets.   
This is done by binding tokio::net::TcpListener to port 6379.
>Many of Tokio's types are named the same as their synchronous equivalent等同、等效 in the Rust standard library. When it makes sense, Tokio exposes the same APIs as std but using async fn.

src/main.rs:
```rust
use tokio::net::{TcpListener, TcpStream};
use mini_redis::{Connection, Frame};

#[tokio::main]
async fn main() {
    // Bind the listener to the address
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        // The second item contains the IP and port of the new connection.
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(socket: TcpStream) {
    // The `Connection` lets us read/write redis **frames** instead of
    // byte streams. The `Connection` type is defined by mini-redis.
    let mut connection = Connection::new(socket);

    if let Some(frame) = connection.read_frame().await.unwrap() {
        println!("GOT: {:?}", frame);

        // Respond with an error
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await.unwrap();
    }
}
```
It processes inbound requests one at a time. When a connection is accepted, the server stays inside the accept loop block until the response is fully written to the socket.  
We want our Redis server to process many concurrent requests. To do this, we need to add some concurrency.
>One of the advantages of using Tokio is that asynchronous code allows you to work on many tasks concurrently, without having to work on them in parallel using ordinary threads. In fact, Tokio can run many tasks concurrently on a single thread!
#### GOT的输出与unwrap()的错误传播

 1. GOT: ... 输出的原因

  这个输出是客户端与服务端之间交互的直接体现，其根本原因在于Redis序列化协议（RESP, REdis Serialization
  Protocol）以及mini-redis库对该协议的解析。

   1. 客户端行为: 当一个Redis客户端（例如您在之前问题中使用的mini-redis客户端）执行client.set("hello",
      "world").await时，它并不会向服务端发送纯文本字符串"SET hello world"。

   2. RESP协议:
      为了清晰、高效地在网络上传输命令和数据，Redis使用RESP协议。一个命令和它的参数被编码为一个RESP数组（Array）。
       * 命令SET hello world包含三个部分：SET、hello、world。
       * 因此，它被编码为一个包含三个元素的数组。
       * 数组中的每个元素本身都是一个RESP体字符串（Bulk String）。

   3. 网络传输: 客户端将SET hello
      world编码为类似*3\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n的字节流，并通过TCP socket发送给服务端。

   4. 服务端解析:
       * 在您的process函数中，let mut connection =
         Connection::new(socket);将原始的TcpStream包装成一个能理解RESP的Connection对象。
       * connection.read_frame().await这个异步方法负责从socket读取字节流，并按照RESP协议规则进行解析。
       * 当它成功解析了客户端发来的完整命令后，会返回一个mini_redis::Frame枚举。对于上述命令，它会构建出一个Frame
         ::Array的变体。
       * 这个Array变体内部包含一个Vec<Frame>，其中有三个元素，每个元素都是Frame::Bulk变体，分别装着"set"、"hello"
         和"world"的字节。

   5. 最终输出: println!("GOT: {:?}",
      frame);这行代码使用Rust的Debug格式化宏（{:?}）来打印这个Frame枚举的内存结构。其输出Array([Bulk(b"set"),
      Bulk(b"hello"), Bulk(b"world")])精确地反映了read_frame方法解析RESP数据后得到的、符合协议规范的数据结构。b"..
      ."表示这是一个字节字符串（&[u8]或Bytes），符合RESP二进制安全的本质。

  ---

  2. 错误传播途径分析

  您这段代码中的错误处理非常直接，主要依赖于.unwrap()方法。这是一个需要特别注意的模式。

  .unwrap()是一个定义在Result<T, E>和Option<T>上的方法。当它被调用时：
   * 如果值是Ok(T)或Some(T)，它会提取出内部的T值。
   * 如果值是Err(E)或None，它会立即引发一个`panic`，导致当前任务异常终止。
核心传播路径:

  `I/O Error` -> `Result::Err` -> `.unwrap()` -> `panic!`

  在这段代码中，任何一个.unwrap()被触发，最终都会导致main任务panic，从而使整个服务器进程崩溃。这是一个非常不健
  壮的设计，因为它缺乏故障隔离（Fault Isolation）。

  改进思路:

   1. 优雅的错误处理: 在process函数中使用match或?操作符（需要process函数返回Result）来处理read_frame和write_frame
      可能返回的Err，而不是panic。例如，可以记录一条错误日志，然后正常结束该连接的处理。

   2. 任务隔离: 在main函数的loop中，使用tokio::spawn来为每个连接创建一个独立的任务：
   ```rust
   1     loop {
   2         let (socket, _) = listener.accept().await.unwrap();
   3         // 为每个连接创建一个独立的Task
   4         tokio::spawn(async move {
   5             process(socket).await;
   6         });
   7     }
   ```
    如果采用这种方式，即使process函数内部因为.unwrap()而panic，也只会终止被spawn出的那个子任务。main函数的loop本身不会受到影响，可以继续accept和处理新的连接，从而大大提高了服务器的健壮性。

#### 避免使用unwrap

在生产级别的Rust代码中，应当完全避免使用.unwrap()。最佳的错误处理实践是一个分层的策略，结合了错误传播、错
  误转换和故障隔离。

  以下是替代.unwrap()的最佳实践，从简单到完善：



  1. 核心原则：返回Result并使用?操作符传播

  这是最直接、最符合语言习惯的改进。其思想是：让可能失败的函数明确地在其签名中返回Result<T,
  E>，然后在函数内部使用?操作符来传播错误。

  ?操作符是match语句的语法糖。当应用于一个Result时：
   * 如果是Ok(value)，它会提取出value。
   * 如果是Err(error)，它会立即从当前函数返回这个Err(error)。

  代码修改示例：

  首先，修改process函数，使其返回一个Result。mini_redis::Result<T>是Result<T, mini_redis::Error>的类型别名。

```rust
    1 use mini_redis::{Connection, Frame, Result};
    2 use tokio::net::TcpStream;
    3
    4 // 1. 修改函数签名，使其返回 Result
    5 async fn process(socket: TcpStream) -> Result<()> {
    6     let mut connection = Connection::new(socket);
    7
    8     // 2. 使用 ? 代替 .unwrap()
    9     if let Some(frame) = connection.read_frame().await? {
   10         println!("GOT: {:?}", frame);
   11
   12         let response = Frame::Error("unimplemented".to_string());
   13
   14         // 3. 同样使用 ? 代替 .unwrap()
   15         connection.write_frame(&response).await?;
   16     }
   17
   18     // 4. 如果函数成功完成，返回 Ok
   19     Ok(())
   20 }

```

现在，process函数不再panic，而是会将I/O或协议错误作为Result::Err返回给它的调用者。

  接下来，在main函数的循环中处理这个Result：
 ```rust
    1 // main函数中的循环
    2 loop {
    3     let (socket, _) = listener.accept().await.unwrap(); //
      这里的unwrap仍需处理，但我们先关注process
    4
    5     // 调用修改后的process函数并处理其返回的Result
    6     if let Err(e) = process(socket).await {
    7         // 如果process返回错误，打印到stderr，但不会让服务器崩溃
    8         eprintln!("failed to process connection: {}", e);
    9     }
   10 }
```

  通过这种方式，单个连接的失败（例如客户端意外断开）只会在日志中留下一条记录，而服务器主循环会继续运行，接受新
  的连接。

  ---

  2. 进阶实践：自定义错误类型

  当一个应用变得复杂时，它可能会遇到来自不同库的多种错误类型（如std::io::Error, mini_redis::Error,
  serde_json::Error等）。为了统一处理，最佳实践是定义一个应用专属的自定义错误类型。

  thiserror crate是实现这一目标的标准工具。

  自定义错误类型示例：
```rust
    1 use thiserror::Error;
    2
    3 // 定义一个应用级别的错误枚举
    4 #[derive(Debug, Error)]
    5 pub enum ServerError {
    6     // `#[from]`属性会自动实现 `From<mini_redis::Error>` trait
    7     // 这使得 `?` 操作符可以自动将 `mini_redis::Error` 转换为 `ServerError::RedisError`
    8     #[error("Redis protocol error")]
    9     RedisError(#[from] mini_redis::Error),
   10
   11     // 也可以定义其他类型的错误
   12     #[error("An unknown error occurred")]
   13     Unknown,
   14 }
   15
   16 // process函数现在返回我们自己的错误类型
   17 async fn process(socket: TcpStream) -> Result<(), ServerError> {
   18     let mut connection = Connection::new(socket);
   19
   20     if let Some(frame) = connection.read_frame().await? { // `?` 在这里会自动转换错误类型
   21         println!("GOT: {:?}", frame);
   22         let response = Frame::Error("unimplemented".to_string());
   23         connection.write_frame(&response).await?;
   24     }
   25
   26     Ok(())
   27 }
```
  使用自定义错误类型，可以为整个应用提供一个统一的、富有上下文信息的错误处理层。

  ---

  3. 最终方案：结合tokio::spawn实现故障隔离

  最健壮的服务器架构结合了上述的优雅错误处理和我们在之前讨论过的任务隔离。
```rust
    1 use tokio::net::{TcpListener, TcpStream};
    2 use mini_redis::{Connection, Frame, Result};
    3
    4 async fn process(socket: TcpStream) -> Result<()> {
    5     // ... 与第一部分的process函数相同，使用?操作符 ...
    6     let mut connection = Connection::new(socket);
    7     if let Some(frame) = connection.read_frame().await? {
    8         println!("GOT: {:?}", frame);
    9         let response = Frame::Error("unimplemented".to_string());
   10         connection.write_frame(&response).await?;
   11     }
   12     Ok(())
   13 }
   14
   15 #[tokio::main]
   16 async fn main() {
   17     let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
   18
   19     loop {
   20         let (socket, _) = listener.accept().await.unwrap();
   21
   22         // 为每个连接创建一个独立的任务
   23         tokio::spawn(async move {
   24             // 在spawn出的任务内部处理错误
   25             if let Err(e) = process(socket).await {
   26                 eprintln!("failed to process connection: {}", e);
   27             }
   28         });
   29     }
   30 }
   ```
  这个最终方案是“最好”的，因为它实现了：
   1. 非恐慌错误处理: 使用Result和?在process函数内部传播错误。
   2. 错误报告: 在spawn出的任务中捕获并记录错误，而不是让它们消失。
   3. 故障隔离: 即使某个process任务因为未被捕获的panic而崩溃（虽然我们已经移除了.unwrap()，但其他代码仍可能panic）
      ，它也只会终止自己的Task，而不会影响服务器主循环。


<table><tr><th>方法</th><th>核心机制</th><th>传播形式</th><th>优点</th><th>缺点</th></tr><tr><td>
     <code>.unwrap()</code></td><td>在<code>Err</code>/<code>None</code>上引发<code>panic</code>。</td
     ><td><strong>栈解旋 (Stack Unwinding) / 任务终止</strong>。错误不作为值返回，而是通过<code>panic
     </code>机制中断正常控制流。</td><td>代码简洁，适用于原型或确定不会失败的场景。</td><td>
     缺乏健壮性，在生产代码中是反模式，会导致程序崩溃。</td></tr><tr><td><code>match</code> / <code>if
     let</code></td><td>显式地对<code>Result</code>的<code>Ok</code>和<code>Err</code>分支进行处理。</
     td><td><strong>本地处理 / 显式控制</strong>。错误本身不自动传播，程序员在<code>Err</code>
     分支中决定是处理、转换还是手动返回。</td><td>控制力最强，可以针对每种错误进行不同处理。</td><td>
     代码冗长，尤其是在多层嵌套调用中。</td></tr><tr><td><code>?</code> 操作符</td><td>
     在函数内部自动传播<code>Err</code>。</td><td><strong>向调用者提前返回 (Early Return)</strong>。将
     <code>Err</code>值立即作为当前函数的返回值，将控制权交还给调用者。</td><td>
     极大地简化了错误传播代码，是Rust中最常用的方式。</td><td>要求函数返回一个兼容的<code>Result</code
     >类型。</td></tr><tr><td>自定义错误类型 + <code>?</code></td><td>定义应用专属的<code>Error</code>
     枚举，并利用<code>#[from]</code>自动转换。</td><td><strong>伴随类型转换的提前返回</strong>
     。在传播前，通过<code>From</code> trait将底层错误类型自动转换为上层自定义错误类型。</td><td>
     统一了应用的错误模型，提供了丰富的上下文，是大型项目的最佳实践。</td><td>需要引入<code>thiserror
     </code>等依赖，并预先设计错误类型。</td></tr></table>

#### frame和bytes

Tcp/IP是面向连接的、可靠的、**基于字节流**的传输层通信协议  

TCP/IP 协议通常被划分为四层（或五层）模型。为了清晰起见，我们使用经典的四层模型：
  1.  应用层 (Application Layer)
  2.  传输层 (Transport Layer)
  3.  网络层 (Internet Layer)
  4.  链路层 (Link Layer)

frame是应用层概念，Bytes：应用层与传输层之间的桥梁  
Frame 的结构和含义是由 Redis 协议 (RESP) 定义的，它是一种应用层协议。就像 HTTP 定义了 GET/index.html，SMTP 定义了 HELO example.com 一样。

#### 目录examples和cargo --example

hello-redis.rs文件在examples文件夹下，但是在src/mian.rs中执行时，使用命令是cargo run --example hello-redis 这其中使用的是example而不是examples  
这是因为：  
examples 目录就是 Cargo 项目结构中一个约定俗成（Convention）的、有特殊含义的文件夹。  
* 约定而非强制：您的项目不是必须有 examples 目录。但如果您创建了这个目录，并把 .rs 文件放进去，Cargo
     就会按“示例”的规则来对待它们。
   * 独立的可执行程序：放在 examples 目录下的每一个 .rs
     文件都被视为一个独立的、小型的可执行程序。这意味着每个文件都应该有自己的 main 函数，可以独立编译和运行。
   * 为库的使用者提供范例：当其他人使用您的库时，他们可以通过阅读和运行 examples
     目录下的代码，快速学习如何使用您的库的各种功能。这是一种非常直观有效的“活文档”。
   * `src`: 存放项目的主要源代码。

cargo run 命令的完整结构可以这样理解：  
cargo run `[--<目标类型>] [<目标名称>]`  
所以，cargo run --example hello-redis命令中的example代表目标的类型，而不是目录




To process connections concurrently, a new task is spawned for each inbound connection. The connection is processed on this task.
```rust
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // A new task is spawned for each inbound socket. The socket is
        // moved to the new task and processed there.
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}
```


### Tasks

**A Tokio task is an asynchronous green thread. They are created by passing an async block to tokio::spawn.**   
The tokio::spawn function returns a JoinHandle, which the caller may use to interact with the spawned task.    
The async block may have a return value. Thecaller may obtain the return value using .await on the JoinHandle.
```rust
#[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        // Do some async work
        "return value"
    });

    // Do some other work

    let out = handle.await.unwrap();
    println!("GOT {}", out);
}
```
Awaiting on JoinHandle returns a Result.  
When a task encounters an error during execution, the JoinHandle will return an Err.   
**This happens when the task either panics, or if the task is forcefully cancelled by the runtime shutting down.**

Tasks are the unit of execution managed by the scheduler.   
**Spawning the task submits it to the Tokio scheduler**, which then ensures that the task executes when it has work to do.   
The spawned task may be executed on the same thread as where it was spawned, or it may execute on a different runtime thread. The task can also be moved between threads after being spawned.

Tasks in Tokio are very lightweight. Under the hood, they **require only a single allocation and 64 bytes of memory**. Applications should feel free to spawn thousands, if not millions of tasks.  
“if not millions of tasks” 不是说“不能超过百万”，而是一种英语中的修辞手法，意思是“尽管去创建成千上万，甚至上百万个任务吧”。它是在鼓励您这样做。

### 'static bound

When you spawn a task on the Tokio runtime, its type's lifetime must be 'static.   
This means that **the spawned task must not contain any references to data owned outside the task**.
>It is a common misconception that 'static always means "lives forever", but this is not the case. Just because a value is 'static does not mean that you have a memory leak. You can read more in Common Rust Lifetime Misconceptions.(https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program)

For example, the following will not compile:
```rust
use tokio::task;

#[tokio::main]
async fn main() {
    let v = vec![1, 2, 3];

    task::spawn(async {
        println!("Here's a vec: {:?}", v);
    });
}

// output
error[E0373]: async block may outlive the current function, but
              it borrows `v`, which is owned by the current function
 --> src/main.rs:7:23
  |
7 |       task::spawn(async {
  |  _______________________^
8 | |         println!("Here's a vec: {:?}", v);
  | |                                        - `v` is borrowed here
9 | |     });
  | |_____^ may outlive borrowed value `v`
  |
note: function requires argument type to outlive `'static`
 --> src/main.rs:7:17
  |
7 |       task::spawn(async {
  |  _________________^
8 | |         println!("Here's a vector: {:?}", v);
9 | |     });
  | |_____^
help: to force the async block to take ownership of `v` (and any other
      referenced variables), use the `move` keyword
  |
7 |     task::spawn(async move {
8 |         println!("Here's a vec: {:?}", v);
9 |     });
  |
```
This happens because, by default, variables are not moved into async blocks.   
The v vector remains owned by the main function. The println! line borrows v. The rust compiler helpfully explains this to us and even suggests the fix! Changing line 7 to `task::spawn(async move {` will instruct the compiler to move v into the spawned task. Now, the task owns all of its data, making it 'static.
#### 关于&`static T、T:`static和&·static mut r、mut r:&'static T

官方推荐的关于lifetime的解释，这里只提取关于static的部分。详细链接：https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program

1. 所有权 (Ownership): 在Rust中，任何一个值都有一个明确的、唯一的“所有者”（Owner）变量。当所有者离开其作用域时，它所拥有的值会被自动销毁（drop）。这是Rust进行内存管理的基础，避免了垃圾回收或手动内存管理。
2. 借用 (Borrowing): 这是一个动作。当你需要访问一个值但又不想转移其所有权时，你就可以向所有者“借用”这个值。借用允许程序的其他部分在不破坏所有权规则的前提下，临时地、有条件地访问数据。
3. 引用 (Reference): 这是一个类型。引用是“借用”这个动作在Rust语法和类型系统中的具体实现。当你创建一个引用时，你就是在执行一次借用。引用本身是一个不拥有其指向数据的值，它仅仅“指向”数据。

“引用其实就是对拥有所有权的数据的借用”是完全正确的。引用是借用的载体。

'static ,描述的是类型的lifetime，其他关于类型的属性，如是否可变，需要使用mut进行修饰

if T: 'static then T must be valid for the entire program  

Misconception Corollaries：
1. T: 'static should be read as "T has a 'static lifetime"
2. &'static T and T: 'static are the same thing
3. if T: 'static then T must be immutable
4. if T: 'static then T can only be created at compile time

Regarding static variables（&`static T ）：  
1. they can only be created at compile-time，这是错误的
2. they should be immutable, mutating them is unsafe
3. they're valid for the entire program
  
Well yes, but a type with a 'static lifetime is different from a type bounded by a 'static lifetime. The latter can be dynamically allocated at run-time, can be safely and freely mutated, can be dropped, and can live for arbitrary durations.（实际上存在&'static mut T：一个可变的静态引用类型）  

It's important at this point to distinguish &'static T from T: 'static.  
1. &'static T is an immutable reference to some T that can be safely held indefinitely long, including up until the end of the program. This is only possible if T itself is immutable and does not move after the reference was created. T does not need to be created at compile-time. It's possible to generate random dynamically allocated data at run-time and return 'static references to it via a memory leak, e.g. 对于&'static mut T，代表指向数据的指针不可变，但是指向的内容可变   
```rust
use rand;

// generate random 'static str refs at run-time
fn rand_str_generator() -> &'static str {
    let rand_string = rand::random::<u64>().to_string();
    Box::leak(rand_string.into_boxed_str())
}
```
2. T: 'static is some T that can be safely held indefinitely long, including up until the end of the program.   
T: 'static includes all &'static T however it also includes all owned types, like String, Vec, etc. The owner of some data is guaranteed that data will never get invalidated as long as the owner holds onto it, therefore the owner can safely hold onto the data indefinitely long, including up until the end of the program. T: 'static should be read as "T can live at least as long as a 'static lifetime" not "T has a 'static lifetime". A program to help illustrate these concepts:
```rust
use rand;

fn drop_static<T: 'static>(t: T) {
    std::mem::drop(t);
}

fn main() {
    let mut strings: Vec<String> = Vec::new();
    for _ in 0..10 {
        if rand::random() {
            // all the strings are randomly generated
            // and dynamically allocated at run-time
            let string = rand::random::<u64>().to_string();
            strings.push(string);
        }
    }

    // strings are owned types so they can
    // live at least as long as 'static
    for mut string in strings {
        // all the strings are mutable
        string.push_str("a mutation");
        // all the strings are droppable
        drop_static(string); // ✅
    }

    // all the strings have been invalidated before the end of the program
    println!("I am the end of the program");
}
```
Key:  
1. T: 'static should be read as "T can live at least as long as a 'static lifetime"
2. if T: 'static then T can be a borrowed type with a 'static lifetime or an owned type
3. since T: 'static includes owned types that means T  
can be dynamically allocated at run-time  
does not have to be valid for the entire program  
can be safely and freely mutated  
can be dynamically dropped at run-time  
can have lifetimes of different durations
##### gemini的解释

if T: 'static then T must be valid for the entire program？  
答案是不是的，T: 'static 并不意味着类型T的每一个实例（instance）都必须在整个程序运行期间有效。

T: 'static 的真正含义：对类型的约束

  T: 'static 是一个泛型约束（Trait Bound）。它约束的是类型`T`本身的构成，而不是该类型的某个具体实例的存活时间。

  它的准确含义是：类型`T`不能包含任何非`'static`的（即临时的、有作用域限制的）引用。

   * 它描述的是一种能力：一个满足T: 'static约束的类型，意味着它的实例有能力在整个程序运行期间都保持有效，因为它不
     依赖于任何可能提前消亡的外部借用。换句话说，它可以被安全地存储任意长的时间。
   * 它不强制实际生命周期：尽管它有这个“能力”，但一个具体的实例完全可以是一个局部变量，在创建后很短的时间内就被销毁。

  最典型的例子就是`String`类型：

   * String类型满足String: 'static约束，因为它是一个拥有其所有数据的类型（Owned
     Type），其内部不包含任何外部引用。
   * 但是，我们可以轻易地创建一个生命周期非常短暂的String实例。
```rust
   1 fn create_and_destroy_string() {
   2     // String 类型满足 T: 'static 约束。
   3     // 我们可以将它传递给一个需要 'static 约束的函数，如 tokio::spawn。
   4     let s = String::from("I am a local variable");
   5
   6     // 但是，变量 s 本身是一个局部变量。
   7     // 它在这里被创建...
   8 } // ...然后在这个函数作用域的末尾，s 被销毁（drop），其内存被释放。
   9   // 显然，这个 s 实例的生命周期远小于 'static。
   
```
  tokio::spawn之所以需要这个约束，是为了确保你传递给它的Future有能力存活那么久，从而避免了它在执行时引用到已经
  被销毁的外部数据。

 ---

  &'static T 的含义：对数据实例的引用

  与T: 'static不同，&'static T是一个具体的引用类型。

  它的含义是：这是一个指向`T`类型某个实例的引用，并且被引用的这个数据实例保证在整个程序的运行期间都有效。

  这里，'static直接描述了被引用数据的存活时间。
```rust
    1 fn reference_static_data() {
    2     // "hello" 是一个字符串字面量，存储在程序的静态数据区。
    3     // 因此，data 指向的数据是 'static 的。
    4     let data: &'static str = "hello";
    5
    6     // 我们不能创建一个指向局部变量的 'static 引用，因为局部变量活不了那么久。
    7     let local_string = String::from("local");
    8     // 下面这行代码无法通过编译，因为 local_string 的生命周期远小于 'static。
    9     // let static_ref_to_local: &'static String = &local_string;
   10 }
```
  ---

  总结对比

  为了清晰地展示区别，请看下表：
    <table><tr><th>对比维度</th><th><code>T: 'static</code></th><th><code>&'static T</code></th></tr>
     <tr><td><strong>本质</strong></td><td>对<strong>类型</strong>的泛型约束 (A bound on a <strong>
     type</strong>)。</td><td>一个具体的<strong>引用类型</strong> (A concrete <strong>reference type</
     strong>)。</td></tr><tr><td><strong>核心含义</strong></td><td>
     “类型T不包含任何非`'static`的引用。”<br>（它是一种“自包含”或“可拥有”的类型）</td><td>
     “这是一个引用，它指向的数据在整个程序运行期间都有效。”</td></tr><tr><td><strong>约束对象</strong>
     </td><td>约束类型的内部构成。</td><td>约束被引用数据实例的存活时间。</td></tr><tr><td><strong>
     对实例生命周期的影响</strong></td><td><strong>无直接影响</strong>
     。类型的实例可以是临时的、有短暂生命周期的局部变量。</td><td><strong>直接规定</strong>
     。被引用的数据实例必须存活于整个程序。</td></tr><tr><td><strong>回答您的问题</strong></td><td><
     strong>否</strong>。一个<code>T: 'static</code>类型的实例不一定在整个程序中有效。</td><td><strong
     >是</strong>。一个<code>&'static T</code>引用的数据必须在整个程序中有效。</td></tr></table>


##### &`static mut r、mut r:&'static T

1. let mut r: &'static T：一个可变的绑定，它持有一个共享的静态引用。
2. &'static mut T：一个可变的静态引用类型。

 ---

  1. let mut r: &'static T (可变的静态引用绑定)

   * 含义：你声明了一个名为 r 的变量，这个变量本身是可变的（因为有 let mut）。这个变量的类型是 &'static T，即一个共享的静态引用。
   * 可变性目标：是变量 `r` 本身。你可以让 r 停止指向一个静态数据，转而指向另一个静态数据。
   * 安全性：完全安全。因为 r 始终指向 'static 的数据，所以不会有生命周期问题。同时，由于它是一个共享引用&T，你不能通过它来修改所指向的数据（除非有内部可变性）。

```rust
    1 fn main() {
    2     let val1: &'static str = "value 1";
    3     let val2: &'static str = "value 2";
    4
    5     // r 这个变量是可变的，可以被重新赋值
    6     let mut r: &'static str = val1;
    7     assert_eq!(*r, "value 1");
    8
    9     // 改变 r，让它指向 val2
   10     r = val2;
   11     assert_eq!(*r, "value 2");
   12 }
```
  ---

2. &'static mut r (静态可变引用)

  这种形式在类型系统上是存在的，但它非常罕见，并且几乎总是与 unsafe 关键字相伴，通常被认为是不良实践。

   * 含义：这是一个可变引用 `&mut r`，同时这个引用的生命周期是 'static 的。
   * 内在矛盾：
       * &mut T 的核心保证是“在我的生命周期内，我是对数据T的唯一且排他的访问者”。
       * 'static 生命周期意味着“我的生命周期是整个程序的运行期间”。
       * 两者结合 &'static mut T 就意味着：“在整个程序的运行期间，我将独占对这个数据的访问权”。这会使得程序的其他
         任何部分都无法再访问该数据，这通常不是我们想要的结果。

   * 如何创建：
      你不能从一个普通的 static 变量安全地创建一个 &'static mut r，因为 static
  变量是全局共享的，如果能创建静态可变引用，多个线程可能会同时创建，从而破坏“唯一性”保证，导致数据竞争。
      因此，&'static mut T 只能从 static mut 变量中创建。

   * `static mut` 和 `unsafe`:
      static mut
  是Rust中的全局可变变量。由于编译器无法在编译时检查对它的所有访问是否安全（防止数据竞争），所以任何对`static
  mut`的读写都必须在`unsafe`块中进行。程序员必须手动保证同步和访问安全。
```rust
    1 // 定义一个全局可变静态变量
    2 static mut COUNTER: u32 = 0;
    3
    4 fn increment() {
    5     // 必须使用 unsafe 块来访问 static mut
    6     // 我们向编译器做出承诺：“我确信这里的操作是线程安全的”
    7     // （在这个单线程例子中是安全的，但在多线程中则会引发数据竞争）
    8     unsafe {
    9         // 从 static mut 创建一个 &'static mut u32
   10         let counter_ref: &'static mut u32 = &mut COUNTER;
   11         *counter_ref += 1;
   12         println!("COUNTER is now: {}", counter_ref);
   13     }
   14 }
   15
   16 fn main() {
   17     increment();
   18     increment();
   19 }
```
  警告和正确实践

  在现代Rust中，应极力避免使用 `static
  mut`。如果你需要一个全局可变的静态状态，正确的、安全的方式是使用一个带有内部可变性的static变量。

  正确示例（使用`Mutex`）:
```rust
    1 use std::sync::Mutex;
    2 use once_cell::sync::Lazy; // 推荐使用 once_cell 来初始化复杂的 static
    3
    4 // 使用 Mutex 来安全地包裹数据，提供内部可变性
    5 static COUNTER: Lazy<Mutex<u32>> = Lazy::new(|| Mutex::new(0));
    6
    7 fn safe_increment() {
    8     // lock() 会在运行时保证独占访问，如果锁被占用则会等待
    9     // 这在多线程环境下是安全的，且不需要 unsafe 块
   10     let mut guard = COUNTER.lock().unwrap();
   11     *guard += 1;
   12     println!("Safe counter is now: {}", *guard);
   13 }
   14
   15 fn main() {
   16     safe_increment();
   17     safe_increment();
   18 }
```
  ---

  总结

<table><tr><th>形式</th><th>含义</th><th>可变性目标</th><th>安全性与实践</th></tr><tr><td><code>
     let mut r: &'static T</code></td><td>一个<strong>可变的绑定</strong>，持有一个共享的静态引用。</
     td><td>绑定<code>r</code>本身，即“指针”可以指向别处。</td><td><strong>安全</strong>
     。这是常规的变量可变性规则。</td></tr><tr><td><code>&'static mut T</code></td><td>一个<strong>
     可变的引用</strong>，其生命周期为<code>'static</code>。</td><td>被引用的数据<code>T</code>本身。
     </td><td><strong>通常需要<code>unsafe</code></strong>。只能从<code>static mut</code>
     创建，极易导致数据竞争，是应避免的反模式。应使用<code>static Mutex&lt;T&gt;</code>等模式替代。</
     td></tr></table>

###### &`static mut r、mut r:&'static T中的&T为什么是共享引用

Rust社区和官方文档更倾向于使用一个更精确的术语：共享引用 (Shared Reference)。

这两者之间有细微但重要的差别，理解它有助于您掌握更复杂的场景，比如内部可变性。

  ---

  “不可变引用” vs “共享引用”

  1. “不可变引用” (Immutable Reference)

  这个术语侧重于引用的能力。它描述了你不能做什么：你不能通过这个引用来改变数据。
```rust
   1 let s = String::from("hello");
   2 let r = &s;
   3 // r.push_str(" world"); // 编译错误！因为 r 是一个“不可变引用”。
  这个理解没有错，它描述了引用的一个重要表现。
```
  2. “共享引用” (Shared Reference)

  这个术语侧重于引用的性质。它描述了你可以做什么：你可以创建任意多个指向同一数据的这种引用，即它们可以被共享。
```rust
   1 let s = String::from("hello");
   2 // &s 的性质是“可共享的”
   3 let r1 = &s;
   4 let r2 = &s; // 我们可以创建多个，在 r1 和 r2 之间“共享”对 s 的访问权。
```
  为什么“共享引用”是更根本的术语？

  因为“不可变”是“共享”性质的必然推论，是为了保证内存安全。

  这回到了我们之前讨论的“别名 xor 可变性”原则：
  > 你可以有别名（多个引用），或者有可变性（修改数据的能力），但不能同时拥有两者。

   1. `&T` 的核心特性是“共享”：当Rust的借用检查器看到&T时，它首先想到的是：“好的，这个引用是可共享的，意味着可能存
      在多个引用指向同一数据（别名）”。

   2. 为了安全，必须“不可变”：因为存在别名的可能性，如果还允许修改数据，就会为数据竞争打开大门。为了从根本上杜绝数
      据竞争，编译器规定：如果一个引用是可共享的，那么它必须是不可变的。

  所以，不是&T恰好不可变，而是因为它可共享，所以它必须不可变。它的“共享”属性是因，“不可变”是果。

  “共享引用”术语在“内部可变性”场景下的优势

  当我们遇到Mutex<T>这样的类型时，“共享引用”这个术语的优越性就体现出来了。
```rust
   1 use std::sync::Mutex;
   2 static DATA: Mutex<String> = Mutex::new(String::new());
   3
   4 fn func() {
   5     let r: &'static Mutex<String> = &DATA; // r 的类型是 &'static Mutex<String>
   6
   7     // 通过 r，我们最终修改了内部的 String
   8     r.lock().unwrap().push_str("modified!");
   9 }
```
   * 如果我们称r为“不可变引用”，这里就会产生语义上的困惑：“我明明通过一个‘不可变引用’修改了数据，这不矛盾吗？”
   * 但如果我们称r为“共享引用”，一切就说得通了：
       * r确实是一个共享引用，我们可以创建很多个&'static Mutex<String>指向DATA。
       * 它本身不允许我们直接修改Mutex这个结构体。
       * 但是，Mutex这个类型的设计，允许我们通过它的共享引用，在运行时通过加锁机制来安全地获得对其内部数据的独占
         访问权，从而进行修改。

  ---

 <table><tr><th>术语</th><th>核心关注点</th><th>含义</th><th>为什么是更优的术语</th></tr><tr><td><
     strong>不可变引用</strong><br>(Immutable Reference)</td><td>引用的<strong>能力</strong>
     （不能做什么）。</td><td>提供对数据的只读访问。</td><td>
     直观，但在遇到“内部可变性”时会产生语义混淆。它描述的是结果，而非原因。</td></tr><tr><td><strong>
     共享引用</strong><br>(Shared Reference)</td><td>引用的<strong>性质</strong>（可以做什么）。</td><
     td>引用可以被自由地复制和共享（产生别名）。</td><td>
     更根本，直接描述了导致其“不可变”的原因。即使在“内部可变性”场景下，该术语依然准确无误。它与“独占引
     用” (Exclusive Reference, <code>&mut T</code>) 形成完美的对立。</td></tr></table>

  结论：是的，您可以继续将&T理解为“不可变引用”，这在90%的情况下都有效。但为了更精确地与Rust的设计哲学对齐，并流
  畅地理解Mutex等高级类型，我们推荐您在心中建立“共享引用”（因此必须是只读的）和“独占引用”（&mutT，因此是可写的）这组更根本的概念。


**If a single piece of data must be accessible from more than one task concurrently, then it must be shared using synchronization primitives such as Arc**.

Note that the error message talks about the argument type outliving the 'static lifetime. This terminology can be rather confusing because the 'static lifetime lasts until the end of the program, so if it outlives it, don't you have a memory leak? The explanation is that it is the type, not the value that must outlive the 'static lifetime, and the value may be destroyed before its type is no longer valid.

When we say that a value is 'static, all that means is that it would not be incorrect to keep that value around forever. This is important because the compiler is unable to reason about how long a newly spawned task stays around. We have to make sure that the task is allowed to live forever, so that Tokio can make the task run as long as it needs to.

我们来看 tokio::spawn 的函数签名（简化后）：
```rust
  pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
  where
      F: Future + Send + 'static,
      F::Output: Send + 'static,
```

  这里有两个 'static 约束：
   1. `F: 'static`: future 本身（也就是你写的 async 块）的类型必须是 'static 的。这意味着 async
      块不能从外部环境中借用非 'static 的数据。如果你想使用外部变量，你需要用 move 关键字将变量的所有权移入
      async 块。
  想象一下，如果没有这个约束，你可以从 spawn 的任务中返回一个 MyBorrowedData<'a>。这个 'a
  可能指向任务内部的某个数据。当任务结束后，它拥有的数据会被销_毁，但你从 handle.await 中拿到的返回值
  MyBorrowedData<'a> 却仍然持有一个指向那块已销毁内存的引用。这是一个典型的悬垂引用，是 Rust
  极力避免的内存不安全问题。

通过强制要求返回值类型 T 满足 T: 'static，Rust编译器保证了你从任务中拿到的返回值要么是完全拥有所有权的数据（如String），要么是指向永不失效的静态数据（如 &'static str），从而根除了内存安全隐患。


The article that the info-box earlier links to uses the terminology "bounded by 'static" rather than "its type outlives 'static" or "the value is 'static" to refer to T: 'static. These all mean the same thing, but are different from "annotated with 'static" as in &'static T.
### Send bound

Tasks spawned by tokio::spawn must implement Send.   
This allows the Tokio runtime to move the tasks between threads while they are suspended at an .await.

**Tasks are Send when all data that is held across .await calls is Send.**当所有跨越 .await 调用的数据都是 Send 时，任务才是 Send 的。   
This is a bit subtle. When .await is called, the task yields back to the scheduler. The next time the task is executed, it resumes from the point it last yielded. To make this work, all state that is used after .await must be saved by the task. If this state is Send, i.e. can be moved across threads, then the task itself can be moved across threads. Conversely, if the state is not Send, then neither is the task.
```rust
// this work
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        // The scope forces `rc` to drop before `.await`.
        {
            let rc = Rc::new("hello");
            println!("{}", rc);
        }

        // `rc` is no longer used. It is **not** persisted when
        // the task yields to the scheduler
        yield_now().await;
    });
}
// this not work
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        let rc = Rc::new("hello");

        // `rc` is used after `.await`. It must be persisted to
        // the task's state.
        yield_now().await;

        println!("{}", rc);
    });
}
```
.await中的数据不能在其离开作用域后再次使用，即引用必须总是可用的

### Store values

有一个例子，未完成
## Shared state

So far, we have a key-value server working. However, there is a major flaw: state is not shared across connections. 
### Strategies

There are a couple of different ways to share state in Tokio.   
1. Guard the shared state with a Mutex.
2. Spawn a task to manage the state and use message passing to operate on it.

About the second：  
  1.  状态独占：将可变的状态（例如一个计数器、一个哈希图等）封装在一个独立的异步任务中。这个任务是该状态的唯一“所有者”。
  2.  消息通道：其他任务不能直接访问或修改这个状态。相反，它们通过一个异步通道（mpsc
  channel）向状态管理任务发送“消息”或“命令”。
  3.  命令处理：状态管理任务在一个循环中等待并处理这些消息，然后根据消息内容来更新自己的内部状态。
  4.  响应（可选）：如果发送方需要从状态管理任务那里获得一个值（例如，获取当前状态），可以在消息中包含一个一次性的返回通道（oneshot channel），状态管理任务通过这个通道将结果发回。

这种模式最大的好处是避免了使用锁（如`Mutex`）。因为只有一个任务可以访问状态，所以从根本上消除了数据竞争（data races）的风险，代码逻辑也更清晰。

Generally you want to use the first approach for simple data, and the second approach for things that require asynchronous work such as I/O primitives. 

In this chapter, the shared state is a HashMap and the operations are insert and get. Neither of these operations is asynchronous, so we will use a Mutex.

1. Instead of using `Vec<u8>`, the Mini-Redis crate uses Bytes from the bytes crate. The goal of Bytes is to provide a robust byte array structure for network programming. The biggest feature it adds over `Vec<u8>` is shallow cloning. In other words, calling clone() on a Bytes instance does not copy the underlying data. Instead, a Bytes instance is a reference-counted handle to some underlying data. The Bytes type is roughly an `Arc<Vec<u8>>` but with some added capabilities.
2. The HashMap will be shared across many tasks and potentially many threads. To support this, it is wrapped in `Arc<Mutex<_>>`.
3. Note that std::sync::Mutex and not tokio::sync::Mutex is used to guard the HashMap. A common error is to unconditionally use tokio::sync::Mutex from within async code. An async mutex is a mutex that is locked across calls to .await.  
A synchronous mutex will block the current thread when waiting to acquire the lock. This, in turn, will block other tasks from processing. However, switching to tokio::sync::Mutex usually does not help as the asynchronous mutex uses a synchronous mutex internally.  
As a rule of thumb, using a synchronous mutex from within asynchronous code is fine as long as contention remains low and the lock is not held across calls to .await.
```rust
// server端代码
use tokio::net::TcpListener;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    println!("Listening");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // Clone the handle to the hash map.
        let db = db.clone();

        println!("Accepted");
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}
```
#### Update process()

```rust
use tokio::net::TcpStream;
use mini_redis::{Connection, Frame};

async fn process(socket: TcpStream, db: Db) {
    use mini_redis::Command::{self, Get, Set};

    // Connection, provided by `mini-redis`, handles parsing frames from
    // the socket
    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }           
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // Write the response to the client
        connection.write_frame(&response).await.unwrap();
    }
}
```
The process function no longer initializes a HashMap. Instead, it takes the shared handle to the HashMap as an argument.process不再初始化hashmap，而是获取一个共享的hashmap作为一个handle当作process函数的参数   
It also needs to lock the HashMap before using it. Remember that the value's type for the HashMap is now Bytes (which we can cheaply clone), so this needs to be changed as well.
### Tasks, threads, and contention

Using a blocking mutex互斥阻塞（互斥锁） to guard short critical sections is an acceptable strategy when contention is minimal. When a lock is contended争夺, the thread executing the task must block and wait on the mutex. This will not only block the current task but it will also block all other tasks scheduled on the current thread.  
By default, the Tokio runtime uses a multi-threaded scheduler. Tasks are scheduled on any number of threads managed by the runtime. If a large number of tasks are scheduled to execute and they all require access to the mutex, then there will be contention.  
On the other hand,**if the current_thread runtime flavor is used, then the mutex will never be contended**.  
这么说并不准确，虽然在单线程中，.await会阻塞当前线程，等待锁被释放，当两个任务同时等待一个琐时，就发生了死锁。在单线程异步代码中，你不应该考虑“线程间”的锁争用，而应该考虑“任务间”的锁争用。并且，绝对不能使用会导致线程阻塞的 std::sync::Mutex，因为它不会导致争用，而是直接导致死锁。  

If contention on a synchronous mutex becomes a problem, the best fix is rarely to switch to the Tokio mutex. Instead, options to consider are to:  
1. Let a dedicated task manage state and use message passing.
2. Shard the mutex.
3. Restructure the code to avoid the mutex.

>The current_thread runtime flavor is a lightweight, single-threaded runtime. It is a good choice when only spawning a few tasks and opening a handful一把/少数 of sockets. For example, this option works well when providing a synchronous API bridge on top of an asynchronous client library.


...很多未完成
## Channels

```rust
use mini_redis::client;

#[tokio::main]
async fn main() {
    // Establish a connection to the server
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // Spawn two tasks, one gets a key, the other sets a key
    let t1 = tokio::spawn(async {
        let res = client.get("foo").await;
    });

    let t2 = tokio::spawn(async {
        client.set("foo", "bar".into()).await;
    });

    t1.await.unwrap();
    t2.await.unwrap();
}
```
This does not compile because both tasks need to access the client somehow.  
As Client does not implement Copy, it will not compile without some code to facilitate this sharing.Additionally, Client::set takes &mut self, which means that exclusive access is required to call it.  
We could open a connection per task, but that is not ideal.   
We cannot use std::sync::Mutex as .await would need to be called with the lock held. We could use tokio::sync::Mutex, but that would only allow a single in-flight request. If the client implements pipelining (in short, sending many commands without waiting for each prior command's response), an async mutex results in underutilizing the connection.
### Message passing

The answer is to use message passing. The pattern involves spawning a dedicated task to manage the client resource. Any task that wishes to issue a request sends a message to the client task. The client task issues the request on behalf of the sender, and the response is sent back to the sender.

Using this strategy, a single connection is established. The task managing the client is able to get exclusive access in order to call get and set.   
Additionally, the channel works as a buffer. Operations may be sent to the client task while the client task is busy. Once the client task is available to process new requests, it pulls the next request from the channel. This can result in better throughput, and be extended to support connection pooling.
#### Tokio's channel primitives

1. mpsc: multi-producer, single-consumer channel. Many values can be sent.
2. oneshot: single-producer, single consumer channel. A single value can be sent.
3. broadcast: multi-producer, multi-consumer. Many values can be sent. Each receiver sees every value.
4. watch: multi-producer, multi-consumer. Many values can be sent, but no history is kept. Receivers only see the most recent value.

If you need a multi-producer multi-consumer channel where only one consumer sees each message, you can use the async-channel crate.  

There are also channels for use outside of asynchronous Rust, such as std::sync::mpsc and crossbeam::channel. These channels wait for messages by blocking the thread, which is not allowed in asynchronous code.
### mpsc

```rust
use bytes::Bytes;

#[derive(Debug)]
enum Command {
    Get {
        key: String,
    },
    Set {
        key: String,
        val: Bytes,
    }
}

use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // Create a new channel with a capacity of at most 32.
    let (tx, mut rx) = mpsc::channel(32);

    // ... Rest comes here
}
```
The mpsc channel is used to send commands to the task managing the redis connection.   
The multi-producer capability allows messages to be sent from many tasks. Creating the channel returns two values, a sender and a receiver. The two handles are used separately. They may be moved to different tasks.

The channel is created with a capacity of 32. If messages are sent faster than they are received, the channel will store them. Once the 32 messages are stored in the channel, calling send(...).await will go to sleep until a message has been removed by the receiver.

Sending from multiple tasks is done by cloning the Sender.
```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel(32);
    let tx2 = tx.clone();

    tokio::spawn(async move {
        tx.send("sending from first handle").await.unwrap();
    });

    tokio::spawn(async move {
        tx2.send("sending from second handle").await.unwrap();
    });

    while let Some(message) = rx.recv().await {
        println!("GOT = {}", message);
    }
}
```
Both messages are sent to the single Receiver handle. It is not possible to clone the receiver of an mpsc channel.

When every Sender has gone out of scope or has otherwise been dropped, it is no longer possible to send more messages into the channel.   
At this point, the recv call on the Receiver will return None, which means that all senders are gone and the channel is closed.

In our case of a task that manages the Redis connection, it knows that it can close the Redis connection once the channel is closed, as the connection will not be used again.
### Spawn manager task

Next, spawn a task that processes messages from the channel. First, a client connection is established to Redis. Then, received commands are issued via the Redis connection.
```rust
use mini_redis::client;
// The `move` keyword is used to **move** ownership of `rx` into the task.
let manager = tokio::spawn(async move {
    // Establish a connection to the server
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // Start receiving messages
    while let Some(cmd) = rx.recv().await {
        use Command::*;

        match cmd {
            Get { key } => {
                client.get(&key).await;
            }
            Set { key, val } => {
                client.set(&key, val).await;
            }
        }
    }
});
```
Now, update the two tasks to send commands over the channel instead of issuing them directly on the Redis connection.
```rust
// The `Sender` handles are moved into the tasks. As there are two
// tasks, we need a second `Sender`.
let tx2 = tx.clone();

// Spawn two tasks, one gets a key, the other sets a key
let t1 = tokio::spawn(async move {
    let cmd = Command::Get {
        key: "foo".to_string(),
    };

    tx.send(cmd).await.unwrap();
});

let t2 = tokio::spawn(async move {
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: "bar".into(),
    };

    tx2.send(cmd).await.unwrap();
});
```
### Receive responses

The final step is to receive the response back from the manager task. The GET command needs to get the value and the SET command needs to know if the operation completed successfully.

To pass the response, a oneshot channel is used. The oneshot channel is a single-producer, single-consumer channel optimized for sending a single value. In our case, the single value is the response.
```rust
use tokio::sync::oneshot;

let (tx, rx) = oneshot::channel();
```
Unlike mpsc, no capacity is specified as the capacity is always one.   
Additionally, neither handle can be cloned.

To receive responses from the manager task, before sending a command, a oneshot channel is created. The Sender half of the channel is included in the command to the manager task. The receive half is used to receive the response.  
First, update Command to include the Sender. For convenience, a type alias is used to reference the Sender.
```rust
use tokio::sync::oneshot;
use bytes::Bytes;

/// Multiple different commands are multiplexed over a single channel.
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}

/// Provided by the requester and used by the manager task to send
/// the command response back to the requester.
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;
```
Now, update the tasks issuing the commands to include the oneshot::Sender.
```rust
let t1 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Get {
        key: "foo".to_string(),
        resp: resp_tx,
    };

    // Send the GET request
    tx.send(cmd).await.unwrap();

    // Await the response
    let res = resp_rx.await;
    println!("GOT = {:?}", res);
});

let t2 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: "bar".into(),
        resp: resp_tx,
    };

    // Send the SET request
    tx2.send(cmd).await.unwrap();

    // Await the response
    let res = resp_rx.await;
    println!("GOT = {:?}", res);
});
```
Finally, update the manager task to send the response over the oneshot channel.
```rust
while let Some(cmd) = rx.recv().await {
    match cmd {
        Command::Get { key, resp } => {
            let res = client.get(&key).await;
            // Ignore errors
            let _ = resp.send(res);
        }
        Command::Set { key, val, resp } => {
            let res = client.set(&key, val).await;
            // Ignore errors
            let _ = resp.send(res);
        }
    }
}
```
Calling send on oneshot::Sender completes immediately and does not require an .await. This is because send on a oneshot channel will always fail or succeed immediately without any form of waiting.

Sending a value on a oneshot channel returns Err when the receiver half has dropped. This indicates the receiver is no longer interested in the response. In our scenario, the receiver cancelling interest is an acceptable event. The Err returned by resp.send(...) does not need to be handled.

未完成
## I/O

I/O in Tokio operates in much the same way as in std, but asynchronously.   
There is a trait for reading (AsyncRead) and a trait for writing (AsyncWrite). Specific types implement these traits as appropriate (TcpStream, File, Stdout).   
AsyncRead and AsyncWrite are also implemented by a number of data structures, such as `Vec<u8>` and `&[u8]`(仍然是在tokio中实现的). This allows using byte arrays where a reader or writer is expected.
### AsyncRead and AsyncWrite

These two traits provide the facilities to asynchronously read from and write to byte streams.  
The methods on these traits are typically not called directly, similar to how you don't manually call the poll method from the Future trait.  
Instead, you will use them through the utility methods provided by AsyncReadExt and AsyncWriteExt.  
All of these functions are async and must be used with .await.
#### async fn read()

AsyncReadExt::read provides an async method for reading data into a buffer, returning the number of bytes read.
>Note: when read() returns Ok(0), this signifies that the stream is closed. Any further calls to read() will complete immediately with Ok(0). With TcpStream instances, this signifies that the read half of the socket is closed.
```rust
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = [0; 10];

    // read up to 10 bytes
    let n = f.read(&mut buffer[..]).await?;

    println!("The bytes: {:?}", &buffer[..n]);
    Ok(())
}
```
#### async fn read_to_end()

AsyncReadExt::read_to_end reads all bytes from the stream until EOF.
```rust
use tokio::io::{self, AsyncReadExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = Vec::new();

    // read the whole file
    f.read_to_end(&mut buffer).await?;
    Ok(())
}
```
#### async fn write()

AsyncWriteExt::write writes a buffer into the writer, returning how many bytes were written.
```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    // Writes some prefix of the byte string, but not necessarily all of it.
    let n = file.write(b"some bytes").await?;

    println!("Wrote the first {} bytes of 'some bytes'.", n);
    Ok(())
}
```
#### async fn write_all()

AsyncWriteExt::write_all writes the entire buffer into the writer.
```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    file.write_all(b"some bytes").await?;
    Ok(())
}
```
未完成
## Framing

Framing is the process of taking a byte stream and converting it to a stream of frames. A frame is a unit of data transmitted between two peers. The Redis protocol frame is defined as follows:
```rust
use bytes::Bytes;

enum Frame {
    Simple(String),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>),
}
```
Note how the frame only consists of data without any semantics. The command parsing and implementation happen at a higher level.
For HTTP, a frame might look like:
```rust
enum HttpFrame {
    RequestHead {
        method: Method,
        uri: Uri,
        version: Version,
        headers: HeaderMap,
    },
    ResponseHead {
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
    },
    BodyChunk {
        chunk: Bytes,
    },
}
```
To implement framing for Mini-Redis, we will implement a Connection struct that wraps a TcpStream and reads/writes mini_redis::Frame values.
```rust
use tokio::net::TcpStream;
use mini_redis::{Frame, Result};

struct Connection {
    stream: TcpStream,
    // ... other fields here
}

impl Connection {
    /// Read a frame from the connection.
    /// 
    /// Returns `None` if EOF is reached
    pub async fn read_frame(&mut self)
        -> Result<Option<Frame>>
    {
        // implementation here
    }

    /// Write a frame to the connection.
    pub async fn write_frame(&mut self, frame: &Frame)
        -> Result<()>
    {
        // implementation here
    }
}
```
You can find the details of the Redis wire protocol here. The full Connection code is found here.?

### Buffered reads

The read_frame method waits for an entire frame to be received before returning.   
A single call to TcpStream::read() may return an arbitrary amount of data. It could contain an entire frame, a partial frame, or multiple frames. If a partial frame is received, the data is buffered and more data is read from the socket. If multiple frames are received, the first frame is returned and the rest of the data is buffered until the next call to read_frame.

To implement this, Connection needs a read buffer field. Data is read from the socket into the read buffer. When a frame is parsed, the corresponding data is removed from the buffer.  
We will use BytesMut as the buffer type. This is a mutable version of Bytes.
```rust
use bytes::BytesMut;
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // Allocate the buffer with 4kb of capacity.
            buffer: BytesMut::with_capacity(4096),
        }
    }
}
```
Next, we implement the read_frame() method.
```rust
use tokio::io::AsyncReadExt;
use bytes::Buf;
use mini_redis::Result;

pub async fn read_frame(&mut self)
    -> Result<Option<Frame>>
{
    loop {
        // Attempt to parse a frame from the buffered data. If
        // enough data has been buffered, the frame is
        // returned.
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // There is not enough buffered data to read a frame.
        // Attempt to read more data from the socket.
        //
        // On success, the number of bytes is returned. `0`
        // indicates "end of stream".
        if 0 == self.stream.read_buf(&mut self.buffer).await? {
            // The remote closed the connection. For this to be
            // a clean shutdown, there should be no data in the
            // read buffer. If there is, this means that the
            // peer closed the socket while sending a frame.
            if self.buffer.is_empty() {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        }
    }
}
```
The read_frame method operates in a loop.  
self.parse_frame() is called. This will attempt to parse a redis frame from self.buffer. If there is enough data to parse a frame, the frame is returned to the caller of read_frame().Otherwise, we attempt to read more data from the socket into the buffer.  

After reading more data, parse_frame() is called again. This time, if enough data has been received, parsing may succeed.  
When reading from the stream, a return value of 0 indicates that no more data will be received from the peer. If the read buffer still has data in it, this indicates a partial frame has been received and the connection is being terminated abruptly. This is an error condition and Err is returned.

When reading from the stream, read_buf is called. This version of the read function takes a value implementing BufMut from the bytes crate.  
First, consider how we would implement the same read loop using read(). Vec<u8> could be used instead of BytesMut.
```rust
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // Allocate the buffer with 4kb of capacity.
            buffer: vec![0; 4096],
            cursor: 0,
        }
    }
}
```
And the read_frame() function on Connection:
```rust
use mini_redis::{Frame, Result};

pub async fn read_frame(&mut self)
    -> Result<Option<Frame>>
{
    loop {
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // Ensure the buffer has capacity
        if self.buffer.len() == self.cursor {
            // Grow the buffer
            self.buffer.resize(self.cursor * 2, 0);
        }

        // Read into the buffer, tracking the number
        // of bytes read
        let n = self.stream.read(
            &mut self.buffer[self.cursor..]).await?;

        if 0 == n {
            if self.cursor == 0 {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        } else {
            // Update our cursor
            self.cursor += n;
        }
    }
}
```
When working with byte arrays and read, we must also maintain a cursor tracking how much data has been buffered.   
We must make sure to pass the empty portion of the buffer to read(). Otherwise, we would overwrite buffered data. If our buffer gets filled up, we must grow the buffer in order to keep reading. In parse_frame() (not included), we would need to parse data contained by `self.buffer[..self.cursor]`.

Because pairing a byte array with a cursor is very common, the bytes crate provides an abstraction representing a byte array and cursor. The Buf trait is implemented by types from which data can be read. The BufMut trait is implemented by types into which data can be written. When passing a T: BufMut to read_buf(), the buffer's internal cursor is automatically updated by read_buf. Because of this, in our version of read_frame, we do not need to manage our own cursor.

Additionally, when using `Vec<u8>`, the buffer must be initialized. vec![0; 4096] allocates an array of 4096 bytes and writes zero to every entry. When resizing the buffer, the new capacity must also be initialized with zeros. The initialization process is not free. When working with BytesMut and BufMut, capacity is uninitialized. The BytesMut abstraction prevents us from reading the uninitialized memory. This lets us avoid the initialization step.
#### Parsing

Now, let's look at the parse_frame() function. Parsing is done in two steps.
1. Ensure a full frame is buffered and find the end index of the frame.
2. Parse the frame.

The mini-redis crate provides us with a function for both of these steps:
1. Frame::check
2. Frame::parse

We will also reuse the Buf abstraction to help. A Buf is passed into Frame::check. As the check function iterates the passed in buffer, the internal cursor will be advanced. When check returns, the buffer's internal cursor points to the end of the frame.
For the Buf type, we will use `std::io::Cursor<&[u8]>`.



未完成，

## Async in depth

### Futures

```rust
use tokio::net::TcpStream;

async fn my_async_fn() {
    println!("hello from async");
    let _socket = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    println!("async TCP operation complete");
}
```
We call the function and it returns some value. We call .await on that value.
```rust
#[tokio::main]
async fn main() {
    let what_is_this = my_async_fn();
    // Nothing has been printed yet.
    what_is_this.await;
    // Text has been printed and socket has been
    // established and closed.
}
```
The value returned by my_async_fn() is a future. A future is a value that implements the std::future::Future trait provided by the standard library. They are values that contain the in-progress asynchronous computation.
The std::future::Future trait definition is:
```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context)
        -> Poll<Self::Output>;
}
```
The associated type Output is the type that the future produces once it completes.  
The Pin type is how Rust is able to support borrows in async functions. See the standard library documentation for more details.  
a Rust future does not represent a computation happening in the background, rather the Rust future is the computation itself.   
The owner of the future is responsible for advancing the computation by polling the future. This is done by calling Future::poll.

未完成

### Executors

Asynchronous Rust functions return futures. Futures must have poll called on them to advance their state. Futures are composed of other futures. So, the question is, what calls poll on the very most outer future?

Recall from earlier, to run asynchronous functions, they must either be passed to tokio::spawn or be the main function annotated with `#[tokio::main]`. This results in submitting the generated outer future to the Tokio executor. The executor is responsible for calling Future::poll on the outer future, driving the asynchronous computation to completion.
#### Mini Tokio

The full code   
https://github.com/tokio-rs/website/blob/master/tutorial-code/mini-tokio/src/main.rs

```rust
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use futures::task;

fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(10);
        let future = Delay { when };

        let out = future.await;
        assert_eq!(out, "done");
    });

    mini_tokio.run();
}

struct MiniTokio {
    tasks: VecDeque<Task>,
}

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

impl MiniTokio {
    fn new() -> MiniTokio {
        MiniTokio {
            tasks: VecDeque::new(),
        }
    }

    /// Spawn a future onto the mini-tokio instance.
    fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.push_back(Box::pin(future));
    }

    fn run(&mut self) {
        let waker = task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        while let Some(mut task) = self.tasks.pop_front() {
            if task.as_mut().poll(&mut cx).is_pending() {
                self.tasks.push_back(task);
            }
        }
    }
}
```














## Bridging with sync code

In most examples of using Tokio, we mark the main function with `#[tokio::main]` and make the entire project asynchronous.

In some cases, you may need to run a small portion of synchronous code. For more information on that, see spawn_blocking.

In other cases, it may be easier to structure the application as largely synchronous, with smaller or logically distinct asynchronous portions. For instance, a GUI application might want to run the GUI code on the main thread and run a Tokio runtime next to it on another thread.

This page explains how you can isolate async/await to a small part of your project.
### What `#[tokio::main]` expands to

The `#[tokio::main]` macro is a macro that replaces your main function with a non-async main function that starts a runtime and then calls your code.
```rust
#[tokio::main]
async fn main() {
    println!("Hello world");
}

// turned into
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            println!("Hello world");
        })
}
```
### A synchronous interface to mini-redis

In this section, we will go through how to build a synchronous interface to mini-redis by storing a Runtime object and using its block_on method.   
In the following sections, we will discuss some alternate approaches and when you should use each approach.



```rust
use tokio::net::ToSocketAddrs;
use tokio::runtime::Runtime;

pub use crate::clients::client::Message;

/// Established connection with a Redis server.
pub struct BlockingClient {
    /// The asynchronous `Client`.
    inner: crate::clients::Client,

    /// A `current_thread` runtime for executing operations on the
    /// asynchronous client in a blocking manner.
    rt: Runtime,
}

impl BlockingClient {
    pub fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<BlockingClient> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        // Call the asynchronous connect method using the runtime.
        let inner = rt.block_on(crate::clients::Client::connect(addr))?;

        Ok(BlockingClient { inner, rt })
    }
}
```
Here, we have included the constructor function as our first example of how to execute asynchronous methods in a non-async context.  
We do this using the block_on method on the Tokio Runtime type, which executes an asynchronous method and returns its result.

One important detail is the use of the current_thread runtime.  
Usually when using Tokio, you would be using the default multi_thread runtime, which will spawn a bunch of background threads so it can efficiently run many things at the same time.  
For our use-case, we are only going to be doing one thing at the time, so we won't gain anything by running multiple threads. This makes the current_thread runtime a perfect fit as it doesn't spawn any threads.

The enable_all call enables the IO and timer drivers on the Tokio runtime. If they are not enabled, the runtime is unable to perform IO or timers.

>Because the current_thread runtime does not spawn threads, it only operates when block_on is called. Once block_on returns, all spawned tasks on that runtime will freeze until you call block_on again. Use the multi_threaded runtime if spawned tasks must keep running when not calling block_on.

Once we have this struct, most of the methods are easy to implement:
```rust
use bytes::Bytes;
use std::time::Duration;

impl BlockingClient {
    pub fn get(&mut self, key: &str) -> crate::Result<Option<Bytes>> {
        self.rt.block_on(self.inner.get(key))
    }

    pub fn set(&mut self, key: &str, value: Bytes) -> crate::Result<()> {
        self.rt.block_on(self.inner.set(key, value))
    }

    pub fn set_expires(
        &mut self,
        key: &str,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        self.rt.block_on(self.inner.set_expires(key, value, expiration))
    }

    pub fn publish(&mut self, channel: &str, message: Bytes) -> crate::Result<u64> {
        self.rt.block_on(self.inner.publish(channel, message))
    }
}
```
The Client::subscribe method is more interesting because it transforms the Client into a Subscriber object. We can implement it in the following manner:
```rust
/// A client that has entered pub/sub mode.
///
/// Once clients subscribe to a channel, they may only perform
/// pub/sub related commands. The `BlockingClient` type is
/// transitioned to a `BlockingSubscriber` type in order to
/// prevent non-pub/sub methods from being called.
pub struct BlockingSubscriber {
    /// The asynchronous `Subscriber`.
    inner: crate::clients::Subscriber,

    /// A `current_thread` runtime for executing operations on the
    /// asynchronous client in a blocking manner.
    rt: Runtime,
}

impl BlockingClient {
    pub fn subscribe(self, channels: Vec<String>) -> crate::Result<BlockingSubscriber> {
        let subscriber = self.rt.block_on(self.inner.subscribe(channels))?;
        Ok(BlockingSubscriber {
            inner: subscriber,
            rt: self.rt,
        })
    }
}

impl BlockingSubscriber {
    pub fn get_subscribed(&self) -> &[String] {
        self.inner.get_subscribed()
    }

    pub fn next_message(&mut self) -> crate::Result<Option<Message>> {
        self.rt.block_on(self.inner.next_message())
    }

    pub fn subscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.subscribe(channels))
    }

    pub fn unsubscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.unsubscribe(channels))
    }
}
```
So, the subscribe method will first use the runtime to transform the asynchronous Client into an asynchronous Subscriber.   
Then, it will store the resulting Subscriber together with the Runtime and implement the various methods using block_on.

Note that the asynchronous Subscriber struct has a non-async method called get_subscribed. To handle this, we simply call it directly without involving the runtime.
### Other approaches

The above section explains the simplest way to implement a synchronous wrapper, but it is not the only way. The approaches are:
1. Create a Runtime and call block_on on the async code.
2. Create a Runtime and spawn things on it.
3. Run the Runtime in a separate thread and send messages to it.
#### Spawning things on a runtime

The Runtime object has a method called spawn. When you call this method, you create a new background task to run on the runtime. For example:
```rust
use tokio::runtime::Builder;
use tokio::time::{sleep, Duration};

fn main() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(10);
    for i in 0..10 {
        handles.push(runtime.spawn(my_bg_task(i)));
    }

    // Do something time-consuming while the background tasks execute.
    std::thread::sleep(Duration::from_millis(750));
    println!("Finished time-consuming task.");

    // Wait for all of them to complete.
    for handle in handles {
        // The `spawn` method returns a `JoinHandle`. A `JoinHandle` is
        // a future, so we can wait for it using `block_on`.
        runtime.block_on(handle).unwrap();
    }
}

async fn my_bg_task(i: u64) {
    // By subtracting, the tasks with larger values of i sleep for a
    // shorter duration.
    let millis = 1000 - 50 * i;
    println!("Task {} sleeping for {} ms.", i, millis);

    sleep(Duration::from_millis(millis)).await;

    println!("Task {} stopping.", i);
}

// output
Task 0 sleeping for 1000 ms.
Task 1 sleeping for 950 ms.
Task 2 sleeping for 900 ms.
Task 3 sleeping for 850 ms.
Task 4 sleeping for 800 ms.
Task 5 sleeping for 750 ms.
Task 6 sleeping for 700 ms.
Task 7 sleeping for 650 ms.
Task 8 sleeping for 600 ms.
Task 9 sleeping for 550 ms.
Task 9 stopping.
Task 8 stopping.
Task 7 stopping.
Task 6 stopping.
Finished time-consuming task.
Task 5 stopping.
Task 4 stopping.
Task 3 stopping.
Task 2 stopping.
Task 1 stopping.
Task 0 stopping.
```

In the above example, we spawn 10 background tasks on the runtime, then wait for all of them.  
As an example, this could be a good way of implementing background network requests in a graphical application because network requests are too time consuming to run them on the main GUI thread.   
Instead, you spawn the request on a Tokio runtime running in the background, and have the task send information back to the GUI code when the request has finished, or even incrementally if you want a progress bar.

In this example, it is important that the runtime is configured to be a multi_thread runtime. If you change it to be a current_thread runtime, you will find that the time consuming task finishes before any of the background tasks start. This is because background tasks spawned on a current_thread runtime will only execute during calls to block_on as the runtime otherwise doesn't have anywhere to run them.



##### 代码核心展示

这段代码的核心是展示如何在 Tokio 异步运行时中：

1. 创建一个（特意配置为单工作线程的）多线程运行时
2. 并发地 spawn（派生）多个异步任务到运行时中去执行
3. 在异步任务后台运行时，主线程可以继续执行自己的（同步）代码
4. 最后，等待所有后台任务执行完毕

##### 代码逐段详解

###### 1. 创建运行时

```rust
let runtime = Builder::new_multi_thread()
    .worker_threads(1)
    .enable_all()
    .build()
    .unwrap();
```

- **`Builder::new_multi_thread()`**: 我们选择创建一个多线程类型的运行时
- **`.worker_threads(1)`**: 这是一个非常关键的配置。虽然是"多线程"运行时，但我们显式地告诉它只使用 1 个工作线程。这意味着所有我们派生的异步任务，都将在这个唯一的工作线程上并发执行（通过时间分片），而不是并行在多个CPU核心上
- **`.enable_all()`**: 启用所有 Tokio 功能，比如 I/O 和计时器（我们的 sleep 需要它）
- **`.build().unwrap()`**: 构建运行时实例

###### 2. 派生后台任务

```rust
let mut handles = Vec::with_capacity(10);
for i in 0..10 {
    handles.push(runtime.spawn(my_bg_task(i)));
}
```

- **`runtime.spawn(my_bg_task(i))`**: 这是将异步任务提交给运行时的核心方法
- **`my_bg_task(i)`** 调用返回一个 Future
- **`runtime.spawn`** 接收这个 Future，并将其交给工作线程去执行
- **`spawn` 是非阻塞的**：它会立即返回一个 `JoinHandle`，而不会等待任务完成。`JoinHandle` 本身也是一个 Future，它在任务完成时才会产出结果
- 这个 for 循环会非常快地执行完毕。它几乎在瞬间就把 10 个任务全部提交给了 Tokio 运行时，并将 10 个 `JoinHandle` 存入了 `handles` 向量中

###### 3. 主线程执行同步任务

```rust
std::thread::sleep(Duration::from_millis(750));
println!("Finished time-consuming task.");
```

> **注意！** 这是标准库中的同步、阻塞的 `sleep`。它会阻塞当前正在执行的线程，也就是主线程

- 在主线程"睡觉"的这 750 毫秒期间，后台的 Tokio 工作线程完全不受影响，它会继续全速地执行那 10 个异步任务

###### 4. 等待所有任务完成

```rust
for handle in handles {
    runtime.block_on(handle).unwrap();
}
```

- 循环遍历之前保存的 `JoinHandle`
- **`runtime.block_on(handle)`**: 主线程在这里调用 `block_on` 来等待一个 `JoinHandle` future 完成
- 如果 `handle` 对应的后台任务已经执行完毕，`block_on` 会立即返回
- 如果后台任务还在执行中（比如还在 sleep），`block_on` 会阻塞主线程，直到该任务完成

###### 5. 异步任务函数

```rust
async fn my_bg_task(i: u64) {
    let millis = 1000 - 50 * i;
    println!("Task {} sleeping for {} ms.", i, millis);

    sleep(Duration::from_millis(millis)).await;

    println!("Task {} stopping.", i);
}
```

- **`async fn`**: 定义一个异步函数
- **`sleep(...).await`**: 

> **注意！** 这是 `tokio::time::sleep`，是异步的 sleep。当代码执行到 `.await` 时，它不会阻塞线程，而是会将当前任务挂起，并把控制权交还给 Tokio 运行时。运行时就可以去执行另一个准备就绪的任务。当 sleep 的时间到了之后，运行时会再次唤醒这个任务，从 `.await` 之后继续执行

---

##### 详细执行顺序

让我们模拟一下时间和事件的流逝：

**T ≈ 0ms:**
1. `main` 函数开始
2. Tokio 运行时被创建，拥有 1 个工作线程（我们称之为 worker-1）。worker-1 处于待命状态

**T ≈ 1ms - 10ms (非常快):**
3. `main` 线程进入第一个 for 循环
4. `runtime.spawn(my_bg_task(0))` 被调用。任务0被提交给运行时。worker-1 线程被唤醒，开始执行任务0
5. **worker-1**: 执行 `my_bg_task(0)`，打印 "Task 0 sleeping for 1000 ms."。然后遇到 `sleep(1000ms).await`。任务0被挂起，worker-1 空闲下来
6. `main` 线程继续循环，`runtime.spawn(my_bg_task(1))`。任务1被提交
7. **worker-1**: 发现自己空闲且队列里有新任务，立即开始执行任务1。打印 "Task 1 sleeping for 950 ms."，然后遇到 `.await`，任务1被挂起
8. ... 这个过程重复...
9. worker-1 会以极快的速度轮流执行所有 10 个任务的开头部分，直到它们各自的 `.await` 点
10. 因此，你会几乎同时看到屏幕上快速打出：
    ```
    Task 0 sleeping for 1000 ms.
    Task 1 sleeping for 950 ms.
    Task 2 sleeping for 900 ms.
    ...
    Task 9 sleeping for 550 ms.
    ```
11. `main` 线程的第一个 for 循环执行完毕

**T ≈ 11ms:**
12. `main` 线程执行 `std::thread::sleep(Duration::from_millis(750))`。主线程开始阻塞，进入睡眠
13. 与此同时，worker-1 线程正在管理 10 个被挂起的、正在计时的 sleep 任务

**T ≈ 561ms (11ms + 550ms):**
14. 任务9的 `sleep(550ms)` 时间到。运行时唤醒任务9
15. **worker-1** 执行任务9的剩余部分，打印 "Task 9 stopping."。任务9执行完毕

**T ≈ 611ms (11ms + 600ms):**
16. 任务8的 `sleep(600ms)` 时间到。worker-1 打印 "Task 8 stopping."

**T ≈ 661ms, 711ms...**
17. 任务7、6... 依次完成并打印 "stopping" 信息

**T ≈ 761ms (11ms + 750ms):**
18. `main` 线程的 `std::thread::sleep` 结束！
19. `main` 线程打印 "Finished time-consuming task."

**T > 761ms:**
20. `main` 线程进入第二个 for 循环，开始等待 `JoinHandle`
21. `runtime.block_on(handles[0])`: 主线程开始等待任务0。此时任务0还在 sleep（它需要总共 1000ms）。所以主线程在这里被阻塞
22. `runtime.block_on(handles[1])`: ...等待任务1...
23. `runtime.block_on(handles[9])`: 当循环到任务9时，由于任务9早已在 T ≈ 561ms 时完成，`block_on` 会几乎立即返回。对于任务8,7,6...也是如此

**T ≈ 1011ms (11ms + 1000ms):**
24. 任务0的 `sleep(1000ms)` 终于结束。worker-1 打印 "Task 0 stopping."。任务0完成
25. `main` 线程的 `block_on(handles[0])` 随即返回
26. `main` 线程继续循环，等待 `handles[1]`（在T≈961ms时完成），`handles[2]`（在T≈911ms时完成）等等，这些等待都会立刻返回

**最后:**
27. 第二个 for 循环结束，`main` 函数退出

##### 总结

- **并发启动**: 10个任务几乎是同时开始的
- **异步执行**: worker-1 线程通过在 `.await` 点切换任务，实现了单线程上的并发
- **主线程与工作线程分离**: 主线程的 `std::thread::sleep` 只影响主线程自己，不影响 worker-1 的工作
- **完成顺序**: 睡眠时间最短的任务最先完成（任务9 → 8 → ...）
- **最终等待**: 主线程最后按顺序等待所有任务，但对于那些已经提前完成的任务，等待会立即结束

## Graceful Shutdown

The purpose of this page is to give an overview of how to properly implement shutdown in asynchronous applications.  
There are usually three parts to implementing graceful shutdown:
1. Figuring out when to shut down.
2. Telling every part of the program to shut down.
3. Waiting for other parts of the program to shut down.

The rest of this article will go through these parts. A real world implementation of the approach described here can be found in mini-redis, specifically the src/server.rs and src/shutdown.rs files.
### Figuring out when to shut down

This will of course depend on the application, but one very common shutdown criteria判断标准 is when the application receives a signal from the operating system.   
This happens e.g. when you press ctrl+c in the terminal while the program is running. To detect this, Tokio provides a tokio::signal::ctrl_c function, which will sleep until such a signal is received. You might use it like this:
```rust
use tokio::signal;

#[tokio::main]
async fn main() {
    // ... spawn application as separate task ...

    match signal::ctrl_c().await {
        Ok(()) => {},
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // we also shut down in case of error
        },
    }

    // send shutdown signal to application and wait
}
```
If you have multiple shutdown conditions, you can use an mpsc channel to send the shutdown signal to one place. You can then select on ctrl_c and the channel. For example:
```rust
use tokio::signal;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();

    // ... spawn application as separate task ...
    //
    // application uses shutdown_send in case a shutdown was issued from inside
    // the application

    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = shutdown_recv.recv() => {},
    }

    // send shutdown signal to application and wait
}
```
### Telling things to shut down

When you want to tell one or more tasks to shut down, you can use Cancellation Tokens.  
These tokens allow you to notify tasks that they should terminate themselves in response to a cancellation request, making it easy to implement graceful shutdowns.

**To share a CancellationToken between several tasks, you must clone it.**  
This is due to the single ownership rule that requires that each value has a single owner.   
When cloning a token, you get another token that's indistinguishable难以区分 from the original; if one is cancelled, then the other is also cancelled. You can make as many clones as you need, and when you call cancel on one of them, they're all cancelled.

explain：  
当你调用 .clone() 时，你只是复制了那个 Arc 智能指针，让引用计数加一。所有的CancellationToken 实例内部的 shared_state 都指向堆上同一个`AtomicBool`。因此，任何一个实例调用 cancel() 修改了那个AtomicBool，所有其他实例都能立刻看到这个变化。

Here are the steps to use CancellationToken in multiple tasks:
1. First, create a new CancellationToken.
2. Then, create a clone of the original CancellationToken by calling the clone method on the original token. This will create a new token that can be used by another task.
3. Pass the original or cloned token to the tasks that should respond to cancellation requests.
4. When you want to shut down the tasks gracefully, call the cancel method on the original or cloned token. Any task listening to the cancellation request on the original or cloned token will be notified to shut down.
```rust
// Step 1: Create a new CancellationToken
let token = CancellationToken::new();

// Step 2: Clone the token for use in another task
let cloned_token = token.clone();

// Task 1 - Wait for token cancellation or a long time
let task1_handle = tokio::spawn(async move {
    tokio::select! {
        // Step 3: Using cloned token to listen to cancellation requests
        _ = cloned_token.cancelled() => {
            // The token was cancelled, task can shut down
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(9999)) => {
            // Long work has completed
        }
    }
});

// Task 2 - Cancel the original token after a small delay
tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Step 4: Cancel the original or cloned token to notify other tasks about shutting down gracefully
    token.cancel();
});

// Wait for tasks to complete
task1_handle.await.unwrap()
```
With Cancellation Tokens, you don't have to shut down a task immediately when the token is cancelled. Instead, you can run a shutdown procedure before terminating the task, such as flushing data to a file or database, or sending a shutdown message on a connection.
### Waiting for things to finish shutting down

Once you have told other tasks to shut down, you will need to wait for them to finish.  
One easy way to do so is to use a task tracker. A task tracker is a collection of tasks. The wait method of the task tracker gives you a future which resolves only after all of its contained futures have resolved and the task tracker has been closed.
```rust
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::task::TaskTracker;

#[tokio::main]
async fn main() {
    let tracker = TaskTracker::new();

    for i in 0..10 {
        tracker.spawn(some_operation(i));
    }

    // Once we spawned everything, we close the tracker.
    tracker.close();

    // Wait for everything to finish.
    tracker.wait().await;

    println!("This is printed after all of the tasks.");
}

async fn some_operation(i: u64) {
    sleep(Duration::from_millis(100 * i)).await;
    println!("Task {} shutting down.", i);
}
```




## Getting started with Tracing

The tracing crate is a framework for instrumenting Rust programs to collect structured, event-based diagnostic information.  
In asynchronous systems like Tokio, interpreting traditional log messages can often be quite challenging. Since individual tasks are multiplexed多路复用 on the same thread, associated events and log lines are intermixed混合、交织 making it difficult to trace the logic flow.

tracing expands upon logging-style diagnostics by allowing libraries and applications to record structured events with additional information about temporality and causality因果 — unlike a log message, a Span in tracing has a beginning and end time, may be entered and exited by the flow of execution, and may exist within a nested tree of similar spans.   
For representing things that occur at a single moment in time, tracing provides the complementary补充的 concept of events.   
Both Spans and Events are structured, with the ability to record typed data as well as textual messages.

You can use tracing to:
1. emit distributed traces发射分布式跟踪 to an OpenTelemetry collector
2. debug your application with Tokio Console
3. log to stdout, a log file or journald
4. profile where your application is spending time
### Setup

add tracing and tracing-subscriber as dependencies:
```rust
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```
The tracing crate provides the API we will use to emit traces. The tracing-subscriber crate provides some basic utilities for forwarding those traces to external listeners (e.g., stdout).
### Subscribing to Traces

If you are authoring创作 an executable (as opposed to a library), you will need to register a tracing subscriber.   
Subscribers are types that process traces emitted by your application and its dependencies, and can perform tasks such as computing metrics, monitoring for errors, and re-emitting traces to the outside world (e.g., journald, stdout, or an open-telemetry daemon).

In most circumstances, you should register your tracing subscriber as early as possible in your main function. For instance, the FmtSubscriber type provided by tracing-subscriber prints formatted traces and events to stdout, can be registered like so:
```rust
#[tokio::main]
pub async fn main() -> mini_redis::Result<()> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;
    ...
}
```
If you run your application now, you may see some trace events emitted by Tokio, but you will need to modify your own application to emit traces to get the most out of tracing.
### Subscriber Configuration

In the above example, we've configured FmtSubscriber with its default configuration. However, tracing-subscriber also provides a number of ways to configure the FmtSubscriber's behavior, such as customizing the output format, including additional information (such as thread IDs or source code locations) in the logs, and writing the logs to somewhere other than stdout.
```rust
// Start configuring a `fmt` subscriber
let subscriber = tracing_subscriber::fmt()
    // Use a more compact, abbreviated log format
    .compact()
    // Display source code file paths
    .with_file(true)
    // Display source code line numbers
    .with_line_number(true)
    // Display the thread ID an event was recorded on
    .with_thread_ids(true)
    // Don't display the event's target (module path)
    .with_target(false)
    // Build the subscriber
    .finish();
```
For details on the available configuration options, see the tracing_subscriber::fmt documentation.(https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html#configuration)




































































## 扩展-关于`tokio={version="1",feature=["full"]}`

在 Rust 的 Cargo 包管理体系中，feature是一种条件编译的机制。它允许一个库（crate）的作者将功能划分为多个可选模块。  
默认情况下，很多功能可能是关闭的。当您作为库的使用者在 Cargo.toml文件中声明需要某个 feature 时，您就打开了对应的开关。Cargo 在编译时会包含与该 feature相关的代码；反之，未开启的 feature 对应的代码将不会被编译。

优化编译时间：只编译您需要的代码，可以显著减少编译整个项目所需的时间。Tokio 的 features 设计得非常精细。下面是一些最常用 features 的说明以及它们的使用场景。
#### 1. 核心运行时 (Runtime)

这是 Tokio 的心脏，负责调度和执行异步任务。您必须至少选择一个。


   * rt：启用一个基本的、单线程的运行时。
   * macros：提供一系列方便的属性宏，最著名的是 #[tokio::main] 和 #[tokio::test]。
   * net：包含所有网络相关的功能。
  1. 时间和定时器 (Time)
   * time：提供与时间相关的功能。
   * sync：提供用于在不同异步任务间同步和通信的工具，是 std::sync 的异步版本。
   * fs：提供异步的文件系统操作。
   * io-util：提供一系列 AsyncReadExt, AsyncWriteExt 等扩展 Trait，它们为异步 I/O 类型添加了很多方便的方法（如
      read_to_end, write_all）。
   * full：这是一个便利的 feature，它会启用上面提到的几乎所有功

示例 2：为一个典型的网络服务器精确选择 `features`


  假设您正在构建一个多线程的 TCP 服务器，它需要处理 Ctrl+C 信号以实现优雅停机，并使用宏来简化 main 函数。


  ```rust
  [dependencies]
  tokio = { version = "1", features = [
      "rt-multi-thread", # 启用多线程运行时
      "macros",          # 启用 #[tokio::main] 等宏
      "net",             # 启用 TCP, UDP 等网络功能
      "signal",          # 启用信号处理
      "sync",            # 如果需要在任务间共享状态，则添加
      "time"             # 如果需要超时处理，则添加
  ] }
  ```

























































































































