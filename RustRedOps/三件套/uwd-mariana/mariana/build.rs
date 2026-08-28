// 注意pure rust项目,即全部是.rs代码的项目,不需要build.rs只有在:
// 1. 跨语言与裸机汇编汇编,如Mariana项目中.
// 2. 链接专有外部静态库.lib时.如 需要调用第三方c闭源编译好的.lib库
// 3. 编译期动态生成代码:有点项目需要在编译期生成一个随机密钥,或用bindgen把几千行win的c头文件(.h)自动翻译成rust结构体
// 4. 探测宿主机环境与动态注入cfg:需要探测当前编译机的rust编译期版本是否支某个底层特性.build.rs探测成功后向cargo发送 println!("cargo:rustc-cfg=has_feature")
// rust中,build.rs是构建脚本.
// rustc只识别.rs文件,原生不具备直接编译c/c++/masm的能力.在底层开发中,无法操作硬件寄存器,必须依赖手写手写汇编(synthetic/desync)
// 因此,rust用了预编译外挂机制,如果在项目根目录下存在build.rs,cargo在编译任何src目录下的代码前,会先用宿主机编译器(具体见下文)
// 把build.rs编译为一个独立的临时可执行程序并立即运行.其流程如下
// 1. cargo build
// 第一阶段:先遣准备
// 2. 编译并运行build.rs
// 3. 调用msvc ml64.exe(通过cc)把desync.asm汇编成机器码,打包生成静态库spoof.lib 输出指令 cargo:rustc-link-lib=spoof
// 第二阶段:正式编译
// 4. rustc编译 src/*.rs 业务代码
// 5. link.exe静态链接器,将Mariana与spoof.rs缝合
// 6. 最终生成二进制文件 

// build.rs在Mariana中的作用
// 1. 跨语言调用:在win下,汇编代码必须由visual studio工具链中的ml64.exe(macro assembler)编译为COFF格式的.obj文件
//      build利用cc crate,自动在你的系统环境变量或vs安装路径中找到ml64.exe并发出编译指令
// 2. 符号绑定与静态链接(让 extern "C"落地): 在uwd.rs中的extern "C" 块中写了ffi声明(fn Spoof ,fn SpoofSynthetic).在没有build.rs时,编译器只知道有个符号叫Spoof,但不知道它的机器码在哪里.
//      build.rs通过标准输出stdout打印特殊通信协议(println!("cargo:rustc-link-lib=static=spoof");),告诉msvc链接器 link.exe将之前编译的spoof.lib静态链接进来,符号Spoof的机器码就在其中.
// 3. 动态特性分流(feature 路由):Mariana中feature有两个选项,desync和synthetic.
//      build.rs通过读取cargo传递的环境变量(如 std::env::var("CARGO_FEATURE_DESYNC")),在编译期决定将哪个汇编文件打包进静态库,不浪费体积
// 4. 增量编译守卫(cargo:rerun-if-changed):如果只修改了rust代码而没有动汇编,build.rs会告诉cargo,直接复用上次的spoof.lib


// 这是build的情况,那么在release或cargo run的情况下呢?

//  Rust cargo中，所有的命令本质上只是同一条流水线上的不同停靠站.其流水线如下:
// 1. 编译并执行 build.rs:任何涉及构建/检查的命令都会先运行这一步
// 2. rustc语法/类型检查:cargo check在这里就停下了
// 3. llvm代码生成与链接:cargo build/cargo build --release 这里生成最终的.exe/.lib
// 4. 操作系统启动进程执行:cargo run在第三步之后在这里开始运行程序


// build.rs是跑在开发机上的,其主要是在开发机上读取环境变量,调用ml64.exe进程,必须使用std
// build.rs不会被打包进最终的.exe/lib文件,build.rs的生命周期在编译完成的那刻就终结了
// 真正进入目标机器内存的只有src/下的代码,只要src/lib.rs在no std环境,最终载荷就是pure的裸机产物

// 用于获取cargo在编译期注入的各种路径与目标架构参数
use std::env;

// 构建脚本的入口,在中断输入cargo build时,cargo会在后台首先编译并调用这个main函数
fn main() {

    // rust官方文档服务器docs.rs在为开源库自动生成api文档时,用的是Linux容器环境.该容器没有win的ml64.exe
    // 这里表示检测到当前是docs.rs服务器抓取代码时,输出警告并提前return.防止文档构建因缺少汇编器而报错崩溃
    if env::var("DOCS_RS").is_ok() {
        println!("cargo:warning=Skipping ASM build for docs.rs");
        return;
    }


    let target = env::var("TARGET").expect("Missing TARGET environment variable");
    

    // Supports x86_64 environments only
    if !target.contains("x86_64") {
        panic!("This build script only supports x86_64 targets.");
    }

    if target.contains("msvc") {
        // Use MASM with cc
        cc::Build::new()
            .file("src/asm/msvc/desync.asm")
            .file("src/asm/msvc/synthetic.asm")
            .compile("spoof");
    } else {
        panic!("Unsupported target: {}", target);
    }
}