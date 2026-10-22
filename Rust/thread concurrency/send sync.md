# concurrency's fundamental and concurrency tools


## send and sync

资料：  
1. The Rustonomicon（https://doc.rust-lang.org/nomicon/send-and-sync.html）
2. https://doc.rust-lang.org/std/marker/trait.Sync.html

Not everything obeys inherited mutability, though.   
Some types allow you to have multiple aliases of a location in memory while mutating it. Unless these types use synchronization to manage this access, they are absolutely not thread-safe.   

Rust captures this through the Send and Sync traits.
1. A type is Send if it is safe to send it to another thread.
2. A type is Sync if it is safe to share between threads (T is Sync if and only if &T is Send).

### send and sync特点
Send and Sync are fundamental to Rust's concurrency story.  
As such, a substantial大量的 amount of special tooling exists to make them work right.   
1. First and foremost, they're unsafe traits. This means that they are unsafe to implement, and other unsafe code can assume that they are correctly implemented. Since they're marker traits (they have no associated items like methods), correctly implemented simply means that they have the intrinsic properties an implementor should have. Incorrectly implementing Send or Sync can cause Undefined Behavior.
2. Send and Sync are also automatically derived traits. This means that, unlike every other trait, if a type is composed entirely of Send or Sync types, then it is Send or Sync. Almost all primitives are Send and Sync, and as a consequence pretty much all types you'll ever interact with are Send and Sync.
Major exceptions include:  
- raw pointers are neither Send nor Sync (because they have no safety guards).
- UnsafeCell isn't Sync (and therefore Cell and RefCell aren't).
- Rc isn't Send or Sync (because the refcount is shared and unsynchronized).
- Rc and UnsafeCell are very fundamentally not thread-safe: they enable unsynchronized shared mutable state.Rc和unsafecell没有使用同步机制（如，原子引用或者锁），这会引发data race。以此获得在单线程中的最佳性能 
- However raw pointers are, strictly speaking, marked as thread-unsafe as more of a lint. Doing anything useful with a raw pointer requires dereferencing it, which is already unsafe. In that sense, one could argue that it would be "fine" for them to be marked as thread safe.

**原始指针到底是不是线程安全的？**

* 从编译器的角度看（最重要）： 裸指针 不是 线程安全的。因为它们没有实现 Send 和Sync，编译器会阻止你以安全的方式在线程间共享它们。这是你在编写安全 Rust 代码时必须遵守的规则。
* 从底层的、哲学的角度看： 您引用的那段话是正确的。裸指针本身只是一个内存地址（本质上是一个整数usize）。复制一个整数或共享一个整数本身是原子性的，不会产生数据竞争。危险的操作是解引用 (`*p`)。
* 在实践和类型系统中，它绝对不是线程安全的。 这是你需要记住并遵守的规则。
* 在物理和哲学层面，指针值本身是安全的，但它所代表的“解引用”能力是不安全的。 Rust为了防止这种不安全的能力被滥用，选择从源头上（类型系统）将其标记为不安全

这种设计，强制程序员在需要跨线程使用裸指针时，必须停下来思考并显式地通过 unsafe impl来声明其安全性，从而将潜在的并发 bug 限制在最小的、可审查的范围内。
### 线程不安全、线程安全之间的转换 

However it's important that they aren't thread-safe to prevent types that contain them from being automatically marked as thread-safe.重要的是阻止包含线程不安全的类型标记为线程安全   
These types have non-trivial重要的 untracked ownership, and it's unlikely that their author was necessarily thinking hard about thread safety.  
In the case of Rc, we have a nice example of a type that contains a *mut that is definitely清楚的/明确的 not thread-safe.Types that aren't automatically derived can simply implement them if desired:
```rust
#![allow(unused)]
fn main() {
struct MyBox(*mut u8);

unsafe impl Send for MyBox {}
unsafe impl Sync for MyBox {}
}
```

In the incredibly rare case that a type is inappropriately automatically derived to be Send or Sync, then one can also unimplement Send and Sync:
```rust
#![allow(unused)]
#![feature(negative_impls)]

fn main() {
// I have some magic semantics for some synchronization primitive!
struct SpecialThreadToken(u8);

impl !Send for SpecialThreadToken {}
impl !Sync for SpecialThreadToken {}
}
```













### cell、Refcell

### Rc


### Arc

```rust
pub struct Arc<T, A = Global>
where
    A: Allocator,
    T: ?Sized,
{ /* private fields */ }

pub struct Arc<
    T: ?Sized,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    ptr: NonNull<ArcInner<T>>,
    phantom: PhantomData<ArcInner<T>>,
    alloc: A,
}
```

#### 特性
1. A thread-safe reference-counting pointer. ‘Arc’ stands for ‘Atomically Reference Counted’.
2. The type `Arc<T>` provides shared ownership of a value of type T, allocated in the heap.   
3. Invoking clone on Arc produces a new Arc instance, which points to the same allocation on the heap as the source Arc和源arc相同, while increasing a reference count.   
4. When the last Arc pointer to a given allocation is destroyed, the value stored in that allocation (often referred to as “inner value”) is also dropped.
#### Arc的内部可变性

https://doc.rust-lang.org/std/sync/struct.Arc.html

Shared references in Rust disallow mutation by default, and Arc is no exception: you cannot generally obtain a mutable reference to something inside an Arc. If you do need to mutate through an Arc, you have several options:
1. Use interior mutability with synchronization primitives like Mutex, RwLock, or one of the Atomic types.
2. Use clone-on-write semantics with Arc::make_mut which provides efficient mutation without requiring interior mutability. This approach clones the data only when needed (when there are multiple references) and can be more efficient when mutations are infrequent.
3. Use Arc::get_mut when you know your Arc is not shared (has a reference count of 1), which provides direct mutable access to the inner value without any cloning.

```rust
use std::sync::Arc;

let mut data = Arc::new(vec![1, 2, 3]);

// This will clone the vector only if there are other references to it
Arc::make_mut(&mut data).push(4);

assert_eq!(*data, vec![1, 2, 3, 4]);
```
>Note: This type is only available on platforms that support atomic loads and stores of pointers, which includes all platforms that support the std crate but not all those which only support alloc. This may be detected at compile time using #[cfg(target_has_atomic = "ptr")].
#### Thread Safety

Unlike `Rc<T>, Arc<T>` uses atomic operations for its reference counting.This means that it is thread-safe.   
The disadvantage is that atomic operations are more expensive than ordinary memory accesses.   
If you are not sharing reference-counted allocations between threads, consider using `Rc<T>` for lower overhead. `Rc<T>` is a safe default, because the compiler will catch any attempt to send an `Rc<T>` between threads. However, a library might choose `Arc<T>` in order to give library consumers more flexibility.

`Arc<T>` will implement Send and Sync as long as the T implements Send and Sync.   
Why can’t you put a non-thread-safe type T in an `Arc<T>` to make it thread-safe? This may be a bit counter-intuitive反直觉 at first: after all, isn’t the point要点 of `Arc<T>` thread safety?   
The key is this: `Arc<T>` makes it thread safe to have multiple ownership of the same data, but it doesn’t add thread safety to its data. Consider `Arc<RefCell<T>>`. `RefCell<T>` isn’t Sync, and if `Arc<T>` was always Send, `Arc<RefCell<T>>` would be as well. But then we’d have a problem: `RefCell<T>` is not thread safe; it keeps track of the borrowing count using non-atomic operations.

In the end, this means that you may need to pair `Arc<T>` with some sort of std::sync type, usually `Mutex<T>`.Arc加上mutex对需要修改的数据加锁
#### Breaking cycles with Weak

The downgrade method can be used to create a non-owning Weak pointer.  
A Weak pointer can be upgraded to an Arc, but this will return None if the value stored in the allocation has already been dropped. In other words, Weak pointers do not keep the value inside the allocation alive; however, they do keep the allocation (the backing store for the value) alive.

A cycle between Arc pointers will never be deallocated. For this reason, Weak is used to break cycles. For example, a tree could have strong Arc pointers from parent nodes to children, and Weak pointers from children back to their parents.

#### Cloning references

Creating a new reference from an existing reference-counted pointer is done using the Clone trait implemented for `Arc<T> and Weak<T>`.
```rust
use std::sync::Arc;
let foo = Arc::new(vec![1.0, 2.0, 3.0]);
// The two syntaxes below are equivalent.
let a = foo.clone();
let b = Arc::clone(&foo);
// a, b, and foo are all Arcs that point to the same memory location
```
#### Deref behavior

`Arc<T>` automatically dereferences to T (via the Deref trait), so you can call T’s methods on a value of type `Arc<T>`. To avoid name clashes with T’s methods, the methods of `Arc<T>` itself are associated functions, called using fully qualified syntax:
```rust
use std::sync::Arc;

let my_arc = Arc::new(());
let my_weak = Arc::downgrade(&my_arc);
```
`Arc<T>`’s implementations of traits like Clone may also be called using fully qualified syntax. Some people prefer to use fully qualified syntax, while others prefer using method-call syntax.
```rust
use std::sync::Arc;

let arc = Arc::new(());
// Method-call syntax
let arc2 = arc.clone();
// Fully qualified syntax
let arc3 = Arc::clone(&arc);

```
`Weak<T>` does not auto-dereference to T, because the inner value may have already been dropped.  
必须通过调用 .upgrade() 方法，尝试将其转换为一个临时的 `Arc<T>或Rc<T>`。
#### Examples

Sharing some immutable data between threads:
```rust
use std::sync::Arc;
use std::thread;

let five = Arc::new(5);

for _ in 0..10 {
    let five = Arc::clone(&five);

    thread::spawn(move || {
        println!("{five:?}");
    });
}
```
Sharing a mutable AtomicUsize:
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

let val = Arc::new(AtomicUsize::new(5));

for _ in 0..10 {
    let val = Arc::clone(&val);

    thread::spawn(move || {
        let v = val.fetch_add(1, Ordering::Relaxed);
        println!("{v:?}");
    });
}
```






#### Arc::make_mut 

#### Atomic types
















## Atomics

资料
https://doc.rust-lang.org/nomicon/atomics.html


### Compiler Reordering

Compilers fundamentally want to be able to do all sorts of complicated transformations to reduce data dependencies and eliminate dead code. In particular, they may radically彻底的/极端的 change the actual order of events, or make events never occur! If we write something like:  
```rust
x = 1;
y = 3;
x = 2;
// The compiler may conclude推断 that it would be best if your program did:
x = 2;
y = 3;
```
This has inverted倒转/反向 the order of events and completely eliminated one event.  
From a single-threaded perspective视角/角度 this is completely unobservable不易察觉的: after all the statements have executed we are in exactly the same state.  
But if our program is multi-threaded, we may have been relying on x to actually be assigned to 1 before y was assigned.   
We would like the compiler to be able to make these kinds of optimizations, because they can seriously improve performance. On the other hand, we'd also like to be able to depend on our program doing the thing we said.
### Hardware Reordering

On the other hand, even if the compiler totally understood what we wanted and respected our wishes, our hardware might instead get us in trouble.   
Trouble comes from CPUs in the form of memory hierarchies内存层级（cpu寄存器registers，cpu缓存L1、L2、L3，主内存RAM即共享内存，硬盘storage）.  
There is indeed a global shared memory space somewhere in your hardware, but from the perspective of each CPU core it is so very far away and so very slow.Each CPU would rather work with its local cache of the data and only go through all the anguish痛苦 of talking to shared memory only when it doesn't actually have that memory in cache.每个CPU都宁愿使用其本地缓存中的数据，并且只在缓存中确实没有该数据时，才会经历与共享内存通信的全部痛苦  

After all, that's the whole point of the cache这才是cache的全部意义（即缓存的意义是隐藏内存访问延迟，缓存存放cpu常用数据，让cpu以接近自身的速度进行处理）, right?   
If every read from the cache had to run back to shared memory to double check that it hadn't changed, what would the point be? （假设数据在本地L1中，每次读取之前，cpu先去内存检查其L1中的数据是否有更新，这会通过总线进行通信，可能与一次完整的内存读取相同）  
The end result is that（基于以上两个前提（CPU依赖本地缓存、缓存不能频繁同步），必然会导出以下结论）the hardware doesn't guarantee that events that occur in some order on one thread, occur in the same order on another thread. 

这段话从“为什么需要缓存”出发，解释了“缓存如何工作才能有效”，并最终推导出“多核环境下内存操作顺序无法保证”这一核心结论。它揭示了计算机体系结构中的一个根本性权衡：为了单核的极致性能，牺牲了多核之间操作的默认一致性。  
因此，编写正确的多线程程序，开发者不能再依赖代码的字面顺序。必须主动使用由编程语言或操作系统提供的同步原语(Synchronization Primitives)，如：
* 锁 (Locks)：保证代码块的互斥访问。
* 原子操作 (Atomics)：保证单个操作的不可分割性，并可以附加内存排序约束。
* 内存屏障 (Memory Fences)：显式地告诉编译器和CPU：“在此处，必须将之前的写操作全部刷新，让其他核心可见”，以此来强制建立一个跨核心的顺序点。


To guarantee this, we must issue special instructions to the CPU telling it to be a bit less smart.For instance, say we convince the compiler to emit this logic:
```rust
initial state: x = 0, y = 1

THREAD 1        THREAD 2
y = 3;          if x == 1 {
x = 1;              y *= 2;
                }

```
Ideally this program has 2 possible final states:  
y = 3: (thread 2 did the check before thread 1 completed)  
y = 6: (thread 2 did the check after thread 1 completed)

However there's a third potential state that the hardware enables:  
y = 2: (thread 2 saw x = 1, but not y = 3, and then overwrote y = 3)

It's worth noting that different kinds of CPU provide different guarantees.  
It is common to separate hardware into two categories: strongly-ordered and weakly-ordered.  
Most notably x86/64 provides strong ordering guarantees, while ARM provides weak ordering guarantees. This has two consequences for concurrent programming:  
Asking for stronger guarantees on strongly-ordered hardware may be cheap or even free because they already provide strong guarantees unconditionally. Weaker guarantees may only yield performance wins on weakly-ordered hardware.

Asking for guarantees that are too weak on strongly-ordered hardware is more likely to happen to work, even though your program is strictly incorrect. If possible, concurrent algorithms should be tested on weakly-ordered hardware.
### Data Accesses

The C++ memory model attempts to bridge the gap by allowing us to talk about the causality of our program.   
Generally, this is by establishing a happens before relationship between parts of the program and the threads that are running them.   
This gives the hardware and compiler room to optimize the program more aggressively where a strict happens-before relationship isn't established, but forces them to be more careful where one is established.The way we communicate these relationships are through **data accesses and atomic accesses**.   

Data accesses are fundamentally unsynchronized and compilers are free to aggressively optimize them.   
In particular, data accesses are free to be reordered by the compiler on the assumption that the program is single-threaded. The hardware is also free to propagate the changes made in data accesses to other threads as lazily and inconsistently as it wants. Most critically, data accesses are how data races happen. Data accesses are very friendly to the hardware and compiler, but as we've seen they offer awful semantics to try to write synchronized code with. Actually, that's too weak.

**It is literally确实的/真正的 impossible to write correct synchronized code using only data accesses.**
#### 解决方案 Atomic access 

Atomic accesses are how we tell the hardware and compiler that our program is multi-threaded. Each atomic access can be marked with an ordering that specifies what kind of relationship it establishes with other accesses.  
In practice, this boils down to telling the compiler and hardware certain things they can't do. For the compiler, this largely revolves around re-ordering of instructions. For the hardware, this largely revolves around how writes are propagated to other threads.

The set of orderings Rust exposes are:
1. Sequentially Consistent (SeqCst)
2. Release
3. Acquire
4. Relaxed

#### Sequentially Consistent

最严格、最直观但也性能开销最大的内存排序模型

Sequentially Consistent is the most powerful of all, implying蕴含 the restrictions of all other orderings.如果一个操作满足了顺序一致性，那么它必然也满足所有比它更弱的内存排序模型（如Acquire-Release）的要求 

Intuitively, a sequentially consistent operation cannot be reordered: all accesses on one thread that happen before and after a SeqCst access stay before and after it.  
所有发生在顺序一致性访问之前和之后的操作，都会保持在它之前和之后。
```rust
 // 线程 1
   2         a = 1; // 操作A
   3         b = 2; // 操作B
   4         atomic_x.store(3, Ordering::SeqCst); // 顺序一致的写操作 (屏障)
   5         c = 4; // 操作C
   6         d = 5; // 操作D
     
```
* 编译器和CPU必须保证：操作A和B一定在 atomic_x.store 完成之前完成。操作C和D一定在 atomic_x.store完成之后才开始。这个SeqCst 操作在代码中建立了一个明确的执行顺序分割线。
* 在单个线程的视角里，SeqCst 操作是一个绝对的执行顺序点，禁止了任何跨越它的指令重排


A data-race-free无数据竞争 program that uses only sequentially consistent atomics and data accesses has the very nice property that there is a single global execution of the program's instructions that all threads agree on.  
一个只使用顺序一致性原子操作和数据访问的、无数据竞争的程序，拥有一个非常好的特性：存在一个所有线程都认同的、单一的程序指令全局执行序列  
消除了多线程程序中“不同线程有不同视角”的复杂性，将所有操作统一到了一个单一的、全局一致的时间线上

This execution is also particularly nice to reason about: it's just an interleaving of each thread's individual executions.   
这个执行序列也特别容易拿来分析：它仅仅是每个线程各自执行序列的一个交错  
* 它提供了一个非常直观、简单的心理模型来理解并发。你不需要去思考缓存、存储缓冲区、指令重排等复杂的底层细节
* `interleaving` (交错)：这是理解 SeqCst 的关键。假设：
           * 线程1的执行序列是：A1 -> B1
           * 线程2的执行序列是：A2 -> B2
       * 一个“交错”的全局序列，就是把这两个序列像洗牌一样混合在一起，但 保持每个线程内部的原始顺序。
       * 可能的全局序列：
           * A1 -> B1 -> A2 -> B2
           * A1 -> A2 -> B1 -> B2
           * A2 -> A1 -> B1 -> B2
           * 等等...
       * 不可能的全局序列：
           * B1 -> A1 -> ... (因为破坏了线程1的内部顺序)
       * 顺序一致性保证，你的程序在某一次运行时的实际表现，一定会是这些“有效交错”中的一种。虽然你不知道具体是哪一
         种（非确定性），但它大大缩小了可能性的范围，使得分析和调试成为可能。

SeqCst 允许我们把复杂的并行问题，简化成一个更简单的、可分析的“指令洗牌”问题  
这句话解释了为什么上一句提到的“单一全局执行序列”对程序员来说是天大的好事

This does not hold if you start using the weaker atomic orderings.  
* 当你使用 Acquire-Release 或 Relaxed 时，编译器和CPU会获得更多的重排指令的自由，以换取更高的性能。
       * 代价是，简单的“交错”模型失效了。你可能会遇到一些在任何“交错”模型下都无法解释的现象。例如，不同线程可能会
         对事件的全局顺序有不同的看法。
       * 此时，你必须使用更复杂、更反直觉的内存模型来分析程序的行为，对程序员的心智负担要求呈指数级增长。


放弃顺序一致性是为了追求性能，但代价是失去了那个简单、直观的“全局交错”心智模型，使得并发编程的推理变得异常困难。

The relative developer-friendliness开发者友好 of sequential consistency doesn't come for free. Even on strongly-ordered platforms sequential consistency involves emitting memory fences.  
顺序一致性让编程变简单，但这是以牺牲程序运行速度为代价换来的
1. `strongly-ordered platforms` (强排序平台)
       * 指那些硬件层面本身就不怎么进行指令重排的CPU架构，最典型的例子就是 x86/x64 (Intel,
         AMD)。在x86上，写的顺序是不会被重排的（写后不会乱于写，读后不会乱于读或写）。这与ARM等 弱排序平台
         (weakly-ordered platforms) 形成了对比，后者会进行更激进的重排。

   2. `...sequential consistency involves emitting memory fences.` (...涉及到生成内存屏障)
       * 这是一个非常关键的技术细节。有人可能误以为，既然x86已经是强排序了，那么用 SeqCst
         应该是“免费”的。但事实并非如此。
       * SeqCst 要求的是一个 全局总顺序，所有线程都必须认同这个顺序。而x86架构本身只保证了每个核心自己的写操作不
         会乱序，但为了实现不同核心之间的全局同步，仍然需要使用特殊的指令。
       * 当编译器遇到一个 SeqCst 操作时，它通常会生成一个 内存屏障 (Memory Fence) 指令（在x86上可能是
         MFENCE，或者通过 LOCK 前缀的指令隐式实现）。这个指令会强制CPU：
           1. 清空其“存储缓冲区 (Store Buffer)”，确保所有之前的写操作都已提交到缓存并对其他核心可见。
           2. 确保在该指令之后的读写操作，必须等待该指令完成才能执行。
       * 这个过程会造成CPU流水线停顿，是有明确性能开销的。

  小结第二句：SeqCst 的性能开销是实实在在的，因为它会生成内存屏障指令，这会强制CPU进行同步，从而产生延迟，这个
  代价在任何平台（包括x86）上都存在。


In practice, sequential consistency is rarely necessary for program correctness程序正确性与顺序一致性几乎无关. However sequential consistency is definitely the right choice if you're not confident about the other memory orders. Having your program run a bit slower than it needs to is certainly better than it running incorrectly! It's also mechanically trivial to downgrade atomic operations to have a weaker consistency later on. Just change SeqCst to Relaxed and you're done! Of course, proving that this transformation is correct is a whole other matter.  
 这句话提出了一个非常重要的观点：大多数时候，我们并不需要 SeqCst 这么强的保证。

   1. `In practice... rarely necessary` (在实践中...很少有必要)
       * 大多数并发算法和数据结构的正确性，并不依赖于一个 全局的总顺序。
       * 它们通常只需要一个更局部的 “发生于...之前” (happens-before)
         的关系。例如，生产者线程完成数据生产（一个“释放”操作），然后消费者线程能够看到这些数据（一个“获取”操作）
         。我们只需要保证“生产”发生在“消费”之前，而不需要关心这两个操作与其他不相关的线程操作之间的全局顺序。
       * 使用 Acquire-Release 排序通常就足以建立这种“发生于...之前”的关系，它比 SeqCst 的开销小得多。

  小结第三句：杀鸡焉用牛刀。SeqCst 是“牛刀”，但大部分并发问题只是“鸡”，用更轻量级的 Acquire-Release
  等模型就能解决。

先用 SeqCst 保证正确性，当性能分析显示这确实是瓶颈时，再尝试降级。但降级这个过程，简单的代码修改背后，是极其复杂的正确性证明工作。

描述了“先安全，后优化”的开发路径，并指出了其中的关键难点。

   1. `...mechanically trivial to downgrade...` (...在操作上是微不足道的)
       * “机械上”或“操作上”指的是修改代码这个物理动作。把代码里的 Ordering::SeqCst 批量替换成 Ordering::Acquire
         或 Ordering::Relaxed，只是一个简单的文本编辑工作。

   2. `Just change SeqCst to Relaxed and you're done!` (只需将SeqCst改成Relaxed，你就完成了！)
       * 这句话带有一点讽刺意味。它说的是“改代码”这个动作本身很简单。

   3. `Of course, proving that this transformation is correct is a whole other matter.`
      (当然，证明这个转换是正确的，则完全是另一回事了)
       * 这是整段话的点睛之笔。真正的挑战不是改代码，而是证明改完之后的代码仍然是正确的。
       * 这个“证明”过程需要你：
           * 深入理解你的并发算法。
           * 精确地识别出线程间需要建立什么样的同步关系。
           * 为你选择的每一个弱排序模型，都能清晰地论证它足以提供必要的保证。
       * 这是一个高难度的智力活动，需要深厚的并发理论知识和实践经验。



















































































































