pub mod asm;
pub mod x86_64;

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
const RANGE: usize = 255;

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
const DOWN: usize = 32;

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
const UP: isize = -32;