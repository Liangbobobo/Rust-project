- [Map](#map)
  - [Option::map](#optionmap)
  - [Result::map / map\_err](#resultmap--map_err)
  - [Iterator::map](#iteratormap)
  - [Result::map\_err](#resultmap_err)


# Map

map在rust中,是一个跨容器的通用操作:把容器里的值做变换，容器本身的结构保持不变。 Some(x) 变完还是 Some(新值)，Ok(x) 变完还是 Ok(新值)，`Vec<T> 变完还是 Vec<U>`——只换里面的值，不换外壳

map 的通用语义是容器结构不变，只变换里面的值，闭包永远收裸值、返回裸值

| 方法 | 适用容器 | 闭包返回 | 结果 | 用途 |
|---|---|---|---|---|
| `map` | `Option`/`Result`/`Iterator` | `U`（裸值） | 同结构容器 | 纯变换 |
| `and_then` | `Option`/`Result` | `Option<U>`/`Result<U,E>` | 展平 | 下一步可能失败 |
| `flat_map` | `Iterator` | `IntoIterator` | 展平迭代器 | 一变多 |
| `filter` | `Iterator` | `bool` | 同迭代器 | 筛选（**闭包返回 bool，不是值**） |
| `map_err` | `Result` | `O` | `Result<T, O>` | 只换错误类型 |
| `or_else` | `Option`/`Result` | `Option<T>`/`Result<T,F>` | 失败时兜底 | 错误恢复 |



## Option::map
Option::map：可能没有值，有才变换.Some(x) → 调 f(x)，返回 Some(f(x));None → 直接返回 None，闭包根本不执行;FnOnce：闭包最多调用一次（因为 Option 里最多一个值），所以闭包可以移动捕获的变量
```rust
pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U>
```

## Result::map / map_err
Result::map / map_err：成功才变换，错误侧单独处理
```rust
pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Result<U, E>
pub fn map_err<O, F: FnOnce(E) -> O>(self, f: F) -> Result<T, O>
```
map作用于ok侧,T作为参数进入闭包,只有在ok时执行;
map_err作用于err侧,E作为参数入参,只有Err时执行

## Iterator::map
Iterator::map：惰性执行
```rust
pub fn map<B, F: FnMut(Self::Item) -> B>(self, f: F) -> Map<Self, F>
```
`->Map<Self, F>`:是一个迭代器适配器,它是惰性的，创建适配器但什么都不算.即map 只登记不执行，必须有 collect/for/sum/count 等消费者才跑


## Result::map_err

map_err是怎么精确定位到`Result<T,E>`中指代错误的E的

1. map_err源码定义
```rust
impl<T, E> Result<T, E> {
    fn map_err<O, F: FnOnce(E) -> O>(self, f: F) -> Result<T, O>
}
```
在定义中,E 是泛型参数.编译器在这里不知道 E 具体是什么，只知“map_err 要求闭包收 E、返回 O 这一步 E 没有指向性

2. 以from_utf8.map_err为例
```rust
pub fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error>
//                                         ↑
//                              注意这里不是泛型 E，是具体类型 Utf8Error
```
from_utf8 没有把错误类型做成泛型——它的返回类型就是 Result<&str, Utf8Error>,Utf8Error 是硬编码在签名里的具体类型。所以 from_utf8(...) 这个表达式的类型，从一开始就是 Result<&str, Utf8Error>，不存在E 待定这回事

3. 调用点：map_err 的 E 被代入成 Utf8Error
```rust
core::str::from_utf8(file_id_bytes)   // 类型：Result<&str, Utf8Error>
    .map_err(|_| ...)
```
这就是指向性的来源：不是 E 本身有指向性，而是调用点接收者的类型把 E 具化（instantiate）了。E 在定义处是变量，在调用处被代入 Utf8Error——同一个符号，两种状态


