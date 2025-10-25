use sysinfo;
fn main() {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    const BYTES_TO_MB: u64 = 1024 * 1024 * 1024;
    // const BYTES_TO_GB: u64 = 1024 * 1024 * 1024;

    // 内存信息
    let memory_info = || {
        let total_memory = sys.total_memory() / BYTES_TO_MB;
        let used_memory = sys.used_memory() / BYTES_TO_MB;
        let total_swap = sys.total_swap() / BYTES_TO_MB;
        let used_swap = sys.used_swap() / BYTES_TO_MB;
        format!(
            "Memory: {} MB used / {} MB total, Swap: {} MB used / {} MB total",
            used_memory, total_memory, used_swap, total_swap
        )
    };

    println!("{}", memory_info());
    // CPU 信息
    let cpu_info = || {
        // sys.refresh_cpu_all();

        // 遍历核心
        for (i, cpu) in sys.cpus().iter().enumerate() {
            println!(
                "Core #{:<2} | Usage: {:>5.2}% | Frequency: {:>5} MHz",
                i,
                cpu.cpu_usage(),
                cpu.frequency()
            );
        }
        //输出总体CPU信息
        if let Some(cpu) = sys.cpus().first() {
            format!(
                "CPU: {} cores, {} at {} MHz",
                sys.cpus().len(),
                cpu.brand(),
                cpu.frequency(),
            )
        } else {
            String::from("No CPU information available")
        } // 注意这里没有分号，代表是一个表达式，作为format！的返回值，如果将for循环和some变换顺序，会导致mismatch错误
    };
    // 调用并打印CPU信息
    println!("---> Getting CPU information...");
    println!("{}", cpu_info());
    println!("<--- CPU information complete.");

    // 网络相关数据
    let network_info = || {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let network_string = sysinfo::Networks::new_with_refreshed_list();
        let _network_report = network_string
            .iter()
            .map(|(interface_name, data)| {
                format!(
                    "Interface: {:<15} | Received: {:>10} KB | Transmitted: {:>10} KB",
                    interface_name,
                    data.received() / 1024,
                    data.transmitted() / 1024
                )
            })
            .collect::<Vec<String>>()
            .join("\n");
        format!("Network Information:\n{:?}", network_string)
    };
    println!("{:?}", network_info());
}
