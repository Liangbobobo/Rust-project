# Thanatos-Wisconsin

Thanatos是用rust实现的Agent模板,主要研究其和mythic服务器连接,任务分发,mythic界面描述文件

**重构项目应以library为内核,binary为外壳**
1. lib方便测试和调试:发包/加解密/序列化全写在binary(exe程序的main.rs文件中),后续的单元测试(cargo test)或本地mock测试非常别扭.把核心逻辑写在lib.rs中,可随时写测试函数断言发包结果
2. 便于后续变形打包:后期不仅需要.exe文件,还需要把agent编译成动态库.dll去配合dll劫持或反射加载.如果核心是lib,只需要修改cargo.toml中的`crate-type=["cdylib"]`即可编译为dll
3. Rust原生支持二者共存:lib.rs作为核心通信库,main.rs作为可执行文件的入口,只负责调用lib.cargo run时,Rust会自动编译运行main.rs生成exe.
4. 一个 package 可以包含多个 crate(完整的编译单元)：一个 library crate（可选,(根文件通常是src/lib.rs,且只能有一个lib.rs)）和多个 binary crate
5. 一个项目package可以包含一个或多个二进制crate:每个二进制crate对应一个入口文件.默认情况下,src/main.rs是其中一个,但可通过src/bin目录下的多个.rs文件或cargo.toml中的`[[bin]]`配置添加更多二进制目标.即一个package可包含多个binary crate,每个binary crate都有自己的入口文件,这些入口文件通常放在src/bin目录下,并且每个文件都可以包含main函数.



## Thanatos-Wisconsin的网络发包逻辑

Payload_Type/thanatos/thanatos/agent_code/src/profiles/http.rs

其文件组织架构如下:
1. 


## Thanatos-Wisconsin 的任务分发主循环（如何接收命令）

Payload_Type/thanatos/thanatos/agent_code/src/tasking.rs

## Thanatos-Wisconsin 的 Mythic 界面描述文件（直接复制）

Payload_Type/thanatos/thanatos/mythic/agent_functions/builder.py