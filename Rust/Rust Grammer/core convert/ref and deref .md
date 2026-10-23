# as_ref and as_deref

rust中为了

rust中带有ref和deref的相关trait或method都是为了从option或result中

在core::convert这个module中,

## as_deref

是option/result的method,其第一个参数必须是self/&self.签名如下:
```rust
pub fn as_deref(&self) -> Option<&<T as Deref>::Target>
where
    T: Deref,
```

作用:Converts from `Option<T> (or &Option<T>)` to Option<&T::Target>.

`Option<&<T as Deref>::Target>`中  
`<T as Deref>::Target`,其中T是一个泛型类型参数(其约束为where T: Deref,);as在这里并不是类型转换(只有在表达式中才是类型转换cast),as 在类型路径里出现就是表示限定路径（qualified path）的意思,因为一个类型可能有多个trait,而不同trait又可能存在重名的关联常量/关联函数/关联类型(trait中的占位符,具体是什么类型,由每个impl自己实现),这个限定路径用于从指定的Deref trait中找指定的关联类型/关联函数.  
以上, `<T as Deref>`是从T实现的Deref trait中查找内容(编译期的类型投影 + 单态化时求值),`<T as Deref>::Target`则是从T实现的Deref trait中找到Target这个关联类型.  
该类型的原始定义为`type Target: ?Sized`,The resulting type after dereferencing.正因为是?Sized,可能是编译期动态大小,而 `Option<T>` 要求 T: Sized，直接写 `Option<<T as Deref>::Target>` 会编译失败。因此必须先用 & 引用包一层——`&<T as Deref>::Target` 是 Sized 的（引用本身大小固定），再放进 Option，才得到合法的 `Option<&<T as Deref>::Target>`,来解决 unsized 的是 &，Option 只负责"可能为空"的语义

源码:
```rust
#[inline]
    #[stable(feature = "option_deref", since = "1.40.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref(&self) -> Option<&T::Target>
    where
        T: [const] Deref,
    {
        self.as_ref().map(Deref::deref)
    }
```
self.as_ref()  →  Option<&T>:Option::as_ref把`&Option<T>`借成 Option<&T>：Some(v) → Some(&v)，None → None。不移动 T，只借引用，因为 as_deref 的接收者是 &self(虽然写作self,但函数签名中接收的参数是&self)，本来就不能移动

.map(Deref::deref),此时变成`Option<&T>.map(Deref::deref)  →  Option<&T::Target>`.而map签名`fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U>`.此时,Deref::deref(签名为`fn deref(&self) -> &Self::Target`),满足map的要求.所以 map 把 Some(&v) 里的 &v 交给 Deref::deref，得到 &*v（&T::Target），重新包成 Some；None 则短路，闭包根本不调 
 
Target 在函数体 self.as_ref().map(Deref::deref) 里字面上根本没出现，它只出现在返回类型 Option<&T::Target> 里。那它到底是怎么参与进来的:Target 是通过 Deref::deref 这个函数项的类型签名被携带进来的，不是靠函数体里显式写出来。 map 完全不认识 Target，它只是根据 Deref::deref 的签名推导出自己该返回什么，而那个签名里就含着 Target
1. Target最初在哪里:它是Deref trait的关联类型,
```rust 
pub trait Deref {
    type Target: ?Sized;                    // ← Target 在这里声明
    fn deref(&self) -> &Self::Target;       // ← deref 的返回类型里含 Target
}
```
Deref::deref 的签名是 fn deref(&self) -> &Self::Target。只要 Target 是 Deref 的关联类型，deref 的返回类型就必然带着它。这就是 Target 参与后续一切的入口
```rust
pub trait Deref {
    type Target: ?Sized;                    // ← Target 在这里声明
    fn deref(&self) -> &Self::Target;       // ← deref 的返回类型里含 Target
}
```
2. self.as_ref()得到的是`option<&T>` 这里和Target无关
3. 之后的.map(Deref::deref)将Deref::deref当作闭包(map签名`fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U>`).注意这里的T和前面出现的T都是代指泛型,并无直接关联.map在这里处理闭包(这里是Deref::deref,作为函数项),将闭包的返回值包装到option中
4. 而Deref::deref的签名为:`fn deref(&self) -> &Self::Target;`这就是 Target 参与的方式：它作为 Deref::deref 返回类型的一部分，被 map 的类型推导进了 U


Deref::deref 而不是 |t| t.deref()因为,Deref::deref 是函数项（function item），可以直接当 Fn 用，等价于闭包，但无额外闭包层：函数项直接作为值传递，不构造闭包环境，代码生成更直接（虽然优化后等价）;避免自动解引用歧义：写 |t| t.deref()，t: &T，方法调用 t.deref() 会先自动解引用 &T → T 再调 T::deref，绕了一圈；写 Deref::deref 直接指定"用 T 的 Deref::deref"，意图精确;标准库里"把已知 trait 方法当函数值传"惯用 UFCS 写法 Trait::method，比闭包短且明确
语法糖展开后
```rust
// 源码的一行
self.as_ref().map(Deref::deref)

// 完全等价的展开
match self {
    Some(v) => Some(Deref::deref(v)),   // v: &T → &T::Target
    None    => None,
}

// 再展开 Deref::deref
match self {
    Some(v) => Some(&**v),              // 或 v.deref()
    None    => None,
}
```



这里是`-> Option<&T::Target>`而as_deref()的签名是`-> Option<&<T as Deref>::Target>`.即这里写的是简写 T::Target.因为:整个 where 子句只有 T: Deref 一个约束;Deref 是唯一能提供 Target 的 trait;编译器解析 T::Target 时，候选唯一，不需要消歧
标准库在"只有单一约束"的地方常写简写，可读性更好；一旦约束可能变多，就会退回全限定。两种写法类型完全相同，只是语法糖与否

`[const]` 是 const trait 的实验语法（旧版写作 ~const）:
运行时调用时，它等价于 T: Deref——普通约束
const 上下文调用时，它额外要求 T: const Deref（即 T 的 Deref impl 本身也是 const 可用的）.这称为条件性 const 约束.同一个函数，两种语境下要求的 trait bound 强度不同。这就是为什么上面 `#[rustc_const_unstable]` 和这里的 `[const]` 总是成对出现——它们共同实现运行时已稳定、const 用还没稳定
>在 stable 用户视角下，这段就等于 where T: Deref。`[const]` 是 nightly 编译器内部语法，你在自己的代码里写不出来


## AsRef trait

as_ref()是AsRef trait实现的必要方法.

rust std及core定义了Trait AsRef(core::convert/std::convert) ,并为多个结构体实现了该AsRef trait.该trait required method as_ref()

该trait作用:Used to do a cheap **reference-to-reference conversion**.If you need to do a costly conversion it is better to implement From with type &T or write a custom function.

AsRef trait定义:
```rust
pub trait AsRef<T: PointeeSized>: PointeeSized {
    // Required method
    fn as_ref(&self) -> &T;
}
```
`AsRef<T: PointeeSized>: PointeeSized`中的`: PointeeSized`是对AsRef trait本身的约束.  
在 trait 定义中，这种写法（在 trait 名后加 : SomeTrait）被称为 Supertrait（超trait）约束。它要求任何实现了 AsRef 的类型 Self，其自身也必须实现 PointeeSized  
这意味着，self 参数的类型（即 &Self）所指向的 Self，也必须是 PointeeSized 的。这确保了 self 本身也是一个可以被引用的、可能无大小的类型  
这整行定义是在说：AsRef 这个 trait 本身，以及它的类型参数 T，都必须是 PointeeSized 的。 这保证了 &self 和返回的 &T 这两种引用都是合法的



>Note: This trait must not fail. If the conversion can fail, use a dedicated method which returns an `Option<T>` or a `Result<T, E>`.  
即,源码中`fn as_ref(&self) -> &T;`这里没有表达转换失败的地方,只返回一个引用,没有 Option 或 Result 来承载错误.如果,有一个可能失败的转换，就不要使用 AsRef，而是构造a dedicated method返回 Option 或 Result来handle error

AsRef auto-dereferences if the inner type is a reference or a mutable reference (e.g.: foo.as_ref() will work the same if foo has type &mut Foo or &&mut Foo).
Note that due to historic reasons, the above currently does not hold generally for all dereferenceable types, e.g. foo.as_ref() will not work the same as Box::new(foo).as_ref(). Instead, many smart pointers provide an as_ref implementation which simply returns a reference to the pointed-to value (but do not perform a cheap reference-to-reference conversion for that value). However, AsRef::as_ref should not be used for the sole purpose of dereferencing; instead ‘Deref coercion’ can be used
1. AsRef 会自动解引用,且引用可以自动穿透
2. due to historic reasons.&T 的自动穿透是 AsRef 特殊实现的，不是 Deref 自动推导的.虽然像 `Box<T>、Rc<T>、Arc<T>` 这些智能指针也实现了 Deref，但 AsRef 并没有自动为它们做引用穿透——它们各自单独实现了 AsRef，返回的是指向被指向值的引用.如:
```rust
let foo = Foo;

// 引用类型：自动穿透
let r = &foo;
r.as_ref();              // ✅ 剥掉 &，调用 Foo::as_ref

// Box：不自动穿透成"引用到引用"，而是 Box 自己的 AsRef 实现
let b = Box::new(foo);
b.as_ref();              // ✅ 但这是 Box<Foo>: AsRef<Foo> 的实现，
                         //    返回 &Foo，而不是"从 &Box<Foo> 剥出 &Foo"

// 关键区别在这里：
Box::new(foo).as_ref()   // 和 foo.as_ref() 不一定"完全一样"
```
官方文档:foo.as_ref() will not work the same as Box::new(foo).as_ref().意思是：`Box<Foo>` 的 AsRef 实现是单独写的（返回 &Foo），不是靠"引用自动穿透"那套规则推导出来的。所以你不能指望"凡是能 Deref 的类型，AsRef 都自动穿透"——只有 &T/&mut T 有特殊实现了穿透引用，Box/Rc/Arc 没有  
However, AsRef::as_ref should not be used for the sole purpose of dereferencing; instead 'Deref coercion' can be used.如果你只是想解引用，不要用 as_ref()，直接用 Deref 强制转换（deref coercion）

>Types which implement Deref should consider implementing `AsRef<T>` as follows

### Reflexivity自反性
Ideally, AsRef would be reflexive, i.e. there would be an `impl<T: ?Sized> AsRef<T>` for T with as_ref simply returning its argument unchanged. Such a blanket implementation is currently not provided due to technical restrictions of Rust’s type system (it would be overlapping with another existing blanket implementation for &T where T: `AsRef<U>` which allows AsRef to auto-dereference, see “Generic Implementations” above).

A trivial implementation of `AsRef<T>` for T must be added explicitly for a particular type T where needed or desired. Note, however, that not all types from std contain such an implementation, and those cannot be added by external code due to orphan rules.
自反性:一个东西能变成它自己,如 UserId → &UserId，String → &String，i32 → &i32 非自反：String → &str（变成别的类型），`Vec<u8> → &[u8]`（变成切片），这些是跨类型转换.  
任何类型 T 都应该能 `AsRef<T>` 到它自己，as_ref() 原样返回 &self.即AsRef 现在只保证“跨类型转换”能用，不保证“自己转自己”能用  
如果想要实现这样自己转自己,需要手动实现,如:
```rust
impl AsRef<UserId> for UserId {
    fn as_ref(&self) -> &UserId { self }   // 原样返回，就这么简单
}
```
这行代码就是“自反实现”——它做的事只是把 &self 原样返回，没有任何转换逻辑，但没有它，AsRef 就是认不出“UserId 能当 &UserId 用”

这本应是很自然的功能,但std就是没有实现.是因为,std对所有类型实现了引用穿透.假设又实现了AsRef的通用功能,那么对于任一类型,编译器就不知道应该选择万能引用穿透还是万能自反.所以只能对每个类型手写 `impl AsRef<Self> for Self`。结果是：你手里有一个 X，传进要 `AsRef<X>` 的函数，不一定能编译——除非有人给 X 补过那行自反 impl.  
如 对于`fn f<T: AsRef<X>>(v: T)` ，如果调用方手里有一个类型为 X 的值，直接传进去不一定能编译，因为需要 X: `AsRef<X>` 成立，而标准库没有为所有类型提供这条自反实现.  
如果是自定义的类型需要给 X 手写 `impl AsRef<X> for X`，不是靠传引用（传引用同样需要底层 X: `AsRef<X>`）   
如果是std里的类型缺少自反impl,受限于孤儿规则,不能为std里的类型补上这个impl.只能自己包一个newtype来实现.   

