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
`<T as Deref>::Target`,其中T是一个泛型类型参数(其约束为where T: Deref,);as在这里并不是类型转换(只有在表达式中才是类型转换cast),as 在类型路径里出现就是表示限定路径（qualified path）的意思,因为一个类型可能有多个trait,而不同trait又可能存在重名的关联常量/关联函数/关联类型(trait中的占位符,具体是什么类型,由每个impl自己实现),这个限定路径用于从指定的trait中找指定的关联类型/关联函数.  
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
>在 stable 用户视角下，这段就等于 where T: Deref。[const] 是 nightly 编译器内部语法，你在自己的代码里写不出来

