//整合了所有sysinfo库能实现的功能，初步想法是每个硬件的信息保存在一个变量或者函数中
//获取硬件使用的是wmi库


use sysinfo::{System};
fn main() {
    let mut sys = System::new_all();
    sys.refresh_all();
//     let os=format!("
//     系统类别{:?} 
//     内核版本:   {:?} 
//     详细内核:   {:?} 
//     操作系统:   {:?}  
//     详细操作系统:{:?}",
//     System::name(),
//     System::kernel_version(),
//     System::kernel_long_version(),
//     System::os_version(),
//     System::long_os_version(),
    
// );
// println!("{}",os);



// println!("{:?}\n{:?}\n{:?}\n{:?}",
// System::host_name(),
// System::cpu_arch(),
// System::physical_core_count(),
// System::open_files_limit());

//process 进程信息
//不refresh
fn process_info( sys: & mut System) ->Vec<String>{
    sys.refresh_all();
    let mut process_pid_name =Vec::new();

    for(_pid,process) in sys.processes(){    
     //这里的错误控制是否正确？
     //这里面有多个函数返回Option，但是在内部不能使用?,因为外部函数返回类型不是Option
     //所以只能使用map_or来处理
     //这同时说明，？时会提前结束并返回
   process_pid_name.push(format!("
  
    {},
   \n",
   //pid,
//    process.name().to_str().unwrap_or_else(|| "无进程名,只有pid")),
        process.exe().and_then(|s|s.to_str()).unwrap_or("路径不存在")
   )); 
}
process_pid_name
} 

println!("{:?}",process_info(& mut sys));


// 搜索包含某一字符的进程
// for process in s.processes_by_name("htop".as_ref()) {
//     println!("{} {:?}", process.pid(), process.name());
// }


//cup信息


//memory信息


}