
use thiserror::Error;
use std::io;

//derive用于自动向数据添加相应的trait
// Debug这个triat,让这个枚举(下面定义的枚举importerror)可以用{:?}打印出来
//Error.自动为这个枚举实现std::fmt::Display,std::error::Error,以打印错误
#[derive(Debug,Error)]

//pub,让其他文件如main看到并使用
//将错误定义为一个enum,因为错误是互斥的
pub enum ImportError {
    // ==========================================
    // 第一层：基础设施错误 (Infrastructure Layer)
    // 这一层通常代表"程序跑不下去了"，属于技术性硬错误
    // ==========================================


    //#[error("... {0}")]：这是定义报错信息的模板。
    //{0},代表变体内的第一个参数,即io::Error
    //如"系统io错误,"系统io故障: No such file or directory"
    #[error("系统io故障:{0}")]
    //Io()是一个元组,里面包裹了一个数据
    //#[from] io::Error,当遇到std::io::Error.且需要返回ImportError时,会自动把这个io错误装入ImportError::Io中.这样可以自由的使用?,控制错误
    //如果没有 #[from]，你写 File::open()?. 会报错，必须写File::open().map_err(ImportError::Io)?。有了它，Rust 自动帮你转换。
    //ImportError这个枚举,是唯一的类型
    //逻辑上ImportError::Io,ImportError::Database都是这个类型处于不同的状态
    Io(#[from] io::Error),

    #[error("数据库底层故障:{0}")]
    Database(#[from] sqlx::Error),

    #[error("Excel 引擎解析故障: {0}")]
    Excel(#[from] calamine::Error), // 自动捕获 Calamine 读取 zip 包等错误

    #[error("异步任务 Join 失败: {0}")]
    TaskJoin(#[from] tokio::task::JoinError), // 自动捕获线程 Panic
    //我们在 spawn_blocking 里跑 Excel 解析。如果那个线程突然崩溃（Panic）了，tokio 会返回这个错误。我们需要捕获它告诉用户“后台任务崩了”。

    
}