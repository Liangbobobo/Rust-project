

use tokio_stream::{self, StreamExt};

#[tokio::main]
async fn main() {
    let  v =vec![1, 2, 3]; 
    let  s =&v;
    let mut item =tokio_stream::iter(s); 
    while let Some(a) = item.next().await {
        println!("{:#?}", a);
    }
    println!("Hello, world!{:?}",v);
} 