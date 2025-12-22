//红队工具首选windows-sys,只有在特定情况下才使用windows库
// 在 windows-sys 中，所有的操作都是最原始的 C 风格 FFI 调用。对于 WLAN（无线局域网）的操作，你将直接面对 Windows 的Native Wifi API。
mod base_operation;
use std::ptr;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::NetworkManagement::WiFi::*;

fn main() {
    unsafe {
        let mut negotiated_version = 0u32;
        let mut client_handle = 0usize;

        // 1. 打开 WLAN 句柄（版本 2：Vista 及以上）
        let status = WlanOpenHandle(2, ptr::null(), &mut negotiated_version, &mut client_handle);
        if status != ERROR_SUCCESS {
            eprintln!("无法初始化 WLAN API，错误码: {}", status);
            return;
        }

        // 2. 枚举无线网卡接口
        let mut p_interface_list: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        let status = WlanEnumInterfaces(client_handle, ptr::null(), &mut p_interface_list);
        if status != ERROR_SUCCESS || p_interface_list.is_null() {
            eprintln!("枚举无线接口失败，错误码: {}", status);
            WlanCloseHandle(client_handle, ptr::null());
            return;
        }

        let list = &*p_interface_list;
        println!("找到 {} 个无线接口", list.dwNumberOfItems);

        // 遍历每个接口
        for i in 0..list.dwNumberOfItems {
            let info_ptr = list.InterfaceInfo.as_ptr().add(i as usize);
            let info = &*info_ptr;

            let description = String::from_utf16_lossy(&info.strInterfaceDescription);
            println!("网卡: {}", description.trim_end_matches('\0'));

            // 3. 获取该接口可见的 Wi-Fi 网络列表
            let mut p_network_list: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
            let status = WlanGetAvailableNetworkList(
                client_handle,
                &info.InterfaceGuid,
                0,
                ptr::null(),
                &mut p_network_list,
            );

            if status == ERROR_SUCCESS && !p_network_list.is_null() {
                let networks = &*p_network_list;
                for j in 0..networks.dwNumberOfItems {
                    let net_ptr = networks.Network.as_ptr().add(j as usize);
                    let net = &*net_ptr;

                    let ssid_len = net.dot11Ssid.uSSIDLength as usize;
                    let ssid_bytes = &net.dot11Ssid.ucSSID[..ssid_len];
                    let ssid_name = String::from_utf8_lossy(ssid_bytes);

                    println!(
                        "  - [{}] SSID: {}, 信号强度: {}%",
                        j + 1,
                        ssid_name,
                        net.wlanSignalQuality
                    );
                }

                // 4. 释放网络列表内存
                WlanFreeMemory(p_network_list as *mut _);
            }
        }

        // 5. 释放接口列表内存
        WlanFreeMemory(p_interface_list as *mut _);

        // 6. 关闭 WLAN 客户端句柄
        WlanCloseHandle(client_handle, ptr::null());
    }
}
