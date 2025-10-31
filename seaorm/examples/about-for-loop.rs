fn main() {
    let mut s = String::from("ssss");

    //只循环了一次，因为[1..7]是一个数组。
    //这个数组只有一个元素（即 1..7 这个范围对象），所以循环只执行一次
    for _i in [1..7] {
        s.push('a');
    }
    println!("{}", s);

    //循环六次
    for _i in 1..7 {
        s.push('b');
    }
    println!("{}", s)
}
