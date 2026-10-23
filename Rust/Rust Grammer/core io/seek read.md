## core::io trait seek

```rust
pub trait Seek {
    // Required method
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    // Provided methods
    fn rewind(&mut self) -> Result<()> { ... }
    fn stream_len(&mut self) -> Result<u64> { ... }
    fn stream_position(&mut self) -> Result<u64> { ... }
    fn seek_relative(&mut self, offset: i64) -> Result<()> { ... }
}
```

## std::io trait Read
```rust
pub trait Read {
    // Provided methods
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> { ... }
    fn read_vectored(
        &mut self,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Result<usize, Error> { ... }
    fn is_read_vectored(&self) -> bool { ... }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error> { ... }
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize, Error> { ... }
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> { ... }
    fn read_buf(&mut self, buf: BorrowedCursor<'_, u8>) -> Result<(), Error> { ... }
    fn read_buf_exact(
        &mut self,
        cursor: BorrowedCursor<'_, u8>,
    ) -> Result<(), Error> { ... }
    fn by_ref(&mut self) -> &mut Self
       where Self: Sized { ... }
    fn bytes(self) -> Bytes<Self> ⓘ
       where Self: Sized { ... }
    fn chain<R>(self, next: R) -> Chain<Self, R> ⓘ
       where R: Read,
             Self: Sized { ... }
    fn take(self, limit: u64) -> Take<Self> ⓘ
       where Self: Sized { ... }
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Error>
       where Self: Sized { ... }
    fn read_le<T>(&mut self) -> Result<T, Error>
       where T: FromEndianBytes,
             Self: Sized { ... }
    fn read_be<T>(&mut self) -> Result<T, Error>
       where T: FromEndianBytes,
             Self: Sized { ... }
}
```

seek和read的参数都有&mut self,签名层面就规定了对同一个 reader 的 seek 和 read 必须串行，不能并发.因为不能同时存在两个以上可变引用.

两个 trait 没有任何直接参数关联：read 不接收位置参数，seek 也不返回给 read 任何东西

关于文件的位置是 reader 的内部状态，由实现者自己维护。seek 改它，read 读它并推进它。这个隐式状态就是两个 trait 联动的唯一纽带
1. std::fs::File——位置在内核里
```rust
use std::io::{Read, Seek, SeekFrom};

let mut f = File::open("a.bin")?;

f.seek(SeekFrom::Start(100))?;   // 调 lseek(fd, 100, SEEK_SET)
let mut buf = [0u8; 16];
let n = f.read(&mut buf)?;        // 调 read(fd, ...) ← 内核从 fd 当前偏移 100 读
// 读完内核把偏移推进到 100 + n
```
File 里只有一个 file descriptor（fd），它本身不存偏移。偏移是内核为每个打开的文件对象维护的。seek 就是 lseek，read 就是 read，两者操作同一个 fd → 同一个内核偏移，天然联动


2. `std::io::Cursor<Vec<u8>>`——位置在结构体字段里
```rust
pub struct Cursor<T> {
    inner: T,
    pos: u64,   // ← 位置在这里
}

impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // 计算新位置，写回 self.pos
    }
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // 从 self.pos 开始拷贝数据，然后 self.pos += n
    }
}
```
这里没有内核，纯用户态，pos 字段就是"位置"，seek 改字段，read 读字段并自增

## std::io Enum SeekFrom 
```rust
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
```