//Type containing system’s information such as processes, memory and CPU.
//  On newer Android versions, there are restrictions on which system information a non-system application has access to. So CPU information might not be available.

use sysinfo::{IS_SUPPORTED_SYSTEM, ProcessRefreshKind};

fn main() {
    if !IS_SUPPORTED_SYSTEM{
        println!("当前系统不受支持");
    }

    // let mut sys=System::new();

    //
    let r=ProcessRefreshKind::nothing();
    println!("{:?}",r);
}