
use std::fs::File;
use std::io::{Read, ErrorKind};

fn main() {
    // =============================================================================
    // !! 重要 !!
    // 请将这里的路径修改为您使用 WPS "另存为"功能后，新保存的那个文件的路径
    // =============================================================================
    let path_str = r"C:\Users\liang\Desktop\1.xlsx"; // <--- 请务必使用您新保存的文件路径

    println!("正在对文件进行底层字节分析: {}", path_str);

    let mut file = match File::open(path_str) {
        Ok(f) => f,
        Err(e) => {
            println!("打开文件失败: {}", e);
            return;
        }
    };

    let mut buffer = [0u8; 16]; // 我们准备读取文件的前 16 个字节
    match file.read_exact(&mut buffer) {
        Ok(_) => {
            println!("\n文件的前 16 个字节 (十六进制表示):");
            for byte in buffer.iter() {
                print!("{:02X} ", byte); // 以十六进制格式打印每个字节，例如 50 4B ...
            }
            println!();

            println!("\n--- 分析结果 ---");
            // 开始比对"魔数"
            if &buffer[0..4] == b"PK\x03\x04" { // "PK" 开头的是 ZIP 压缩包
                println!("诊断: 这是一个标准的 .xlsx (ZIP) 文件。");
                println!("推论: 如果 calamine 仍无法读取，可能是 WPS 生成的 ZIP 内部 XML 结构有问题，或者 calamine 库与 WPS 生成的文件存在兼容性问题。");
            } else if &buffer[0..8] == b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1" {
                println!("诊断: 这是一个标准的 .xls (OLE) 文件。");
            } else if String::from_utf8_lossy(&buffer).trim_start().starts_with("<") {
                 println!("诊断: 这是一个基于文本的文件，很可能是 HTML 或 XML。");
                 println!("推论: WPS 可能只是给一个网页文件换了个 .xlsx 的扩展名，这解释了为什么 calamine 会失败。");
            }
            else {
                println!("诊断: 文件类型未知。");
                println!("推论: 它的起始字节不符合任何标准 Office 文档的规范。这很可能是 WPS 保存的一种特殊或非标准格式。这完美解释了为什么 calamine 无法解析它。");
            }
        },
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
             println!("\n读取文件字节失败: 文件大小不足 16 字节，很可能是一个空文件或内容极少。");
        },
        Err(e) => {
            println!("\n读取文件字节时发生未知错误: {}", e);
        }
    }
}
