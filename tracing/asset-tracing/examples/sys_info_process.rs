//关于sys_info对process的处理
//不适用与ios due to sandboxing

use sysinfo::{System};
fn main() {
    let mut sys = System::new_all();
    sys.refresh_all();


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



}