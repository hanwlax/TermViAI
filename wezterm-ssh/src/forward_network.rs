//! Local routing evidence for reverse tunnels. A route never proves SSH reachability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Available,
    Offline,
    Unknown,
}

#[cfg(not(windows))]
pub fn path_to(_: &str) -> Path {
    Path::Unknown
}
#[cfg(not(windows))]
pub fn revision() -> u64 {
    0
}

#[cfg(windows)]
pub use platform::{path_to, revision};

#[cfg(windows)]
mod platform {
    use super::Path;
    use std::ffi::c_void;
    use std::net::{SocketAddr, ToSocketAddrs};
    use std::ptr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Once;
    use windows_sys::Win32::Foundation::{
        ERROR_HOST_UNREACHABLE, ERROR_NETWORK_UNREACHABLE, ERROR_NOT_FOUND,
    };
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_INET};
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC,
    };

    static REVISION: AtomicU64 = AtomicU64::new(0);
    unsafe extern "system" fn interface(
        _: *const c_void,
        _: *const MIB_IPINTERFACE_ROW,
        _: MIB_NOTIFICATION_TYPE,
    ) {
        REVISION.fetch_add(1, Ordering::Relaxed);
    }
    unsafe extern "system" fn address(
        _: *const c_void,
        _: *const MIB_UNICASTIPADDRESS_ROW,
        _: MIB_NOTIFICATION_TYPE,
    ) {
        REVISION.fetch_add(1, Ordering::Relaxed);
    }
    unsafe extern "system" fn route(
        _: *const c_void,
        _: *const MIB_IPFORWARD_ROW2,
        _: MIB_NOTIFICATION_TYPE,
    ) {
        REVISION.fetch_add(1, Ordering::Relaxed);
    }
    unsafe extern "system" fn power(_: *const c_void, event: u32, _: *const c_void) -> u32 {
        if event == PBT_APMRESUMEAUTOMATIC {
            REVISION.fetch_add(1, Ordering::Relaxed);
        }
        0
    }
    pub fn revision() -> u64 {
        static REGISTER: Once = Once::new();
        REGISTER.call_once(|| unsafe {
            // Process-lifetime registrations: callbacks only touch a static atomic.
            let mut handle = ptr::null_mut();
            let power_params = Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS { Callback: Some(power), Context: ptr::null_mut() }));
            for status in [
                NotifyIpInterfaceChange(AF_UNSPEC, Some(interface), ptr::null(), false, &mut handle),
                NotifyUnicastIpAddressChange(AF_UNSPEC, Some(address), ptr::null(), false, &mut handle),
                NotifyRouteChange2(AF_UNSPEC, Some(route), ptr::null(), false, &mut handle),
                PowerRegisterSuspendResumeNotification(DEVICE_NOTIFY_CALLBACK, (power_params as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS).cast(), &mut handle),
            ] {
                if status != 0 { log::warn!("Network change subscription failed ({status}); using periodic route checks"); }
            }
        });
        REVISION.load(Ordering::Relaxed)
    }

    fn external_interfaces_down() -> bool {
        unsafe {
            let mut table = ptr::null_mut();
            if GetIfTable2(&mut table) != 0 {
                return false;
            }
            let rows =
                std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
            let down = rows
                .iter()
                .all(|r| r.Type == IF_TYPE_SOFTWARE_LOOPBACK || r.OperStatus != IfOperStatusUp);
            FreeMibTable(table.cast());
            down
        }
    }

    fn route_to(address: SocketAddr) -> Path {
        if address.ip().is_loopback() {
            return Path::Available;
        }
        let dest = socket2::SockAddr::from(address);
        unsafe {
            let mut route = MIB_IPFORWARD_ROW2::default();
            let mut source = SOCKADDR_INET::default();
            match GetBestRoute2(
                ptr::null(),
                0,
                ptr::null(),
                dest.as_ptr().cast(),
                0,
                &mut route,
                &mut source,
            ) {
                0 => {
                    let mut interface = MIB_IF_ROW2 {
                        InterfaceLuid: route.InterfaceLuid,
                        ..Default::default()
                    };
                    if GetIfEntry2(&mut interface) != 0 {
                        return Path::Unknown;
                    }
                    if interface.OperStatus != IfOperStatusUp {
                        return Path::Offline;
                    }
                    match source.si_family {
                        AF_INET if source.Ipv4.sin_addr.S_un.S_addr != 0 => Path::Available,
                        AF_INET6 if source.Ipv6.sin6_addr.u.Byte != [0; 16] => Path::Available,
                        _ => Path::Unknown,
                    }
                }
                ERROR_NOT_FOUND | ERROR_NETWORK_UNREACHABLE | ERROR_HOST_UNREACHABLE => {
                    Path::Offline
                }
                _ => Path::Unknown,
            }
        }
    }
    pub fn path_to(host: &str) -> Path {
        let host = host.trim_matches(['[', ']']);
        let down = external_interfaces_down();
        let mut result = None;
        if let Ok(addresses) = (host, 0).to_socket_addrs() {
            for addr in addresses {
                let path = if down && !addr.ip().is_loopback() {
                    Path::Offline
                } else {
                    route_to(addr)
                };
                match path {
                    Path::Available => return path,
                    Path::Unknown => result = Some(path),
                    Path::Offline => {
                        result.get_or_insert(path);
                    }
                }
            }
        }
        result.unwrap_or(if down { Path::Offline } else { Path::Unknown })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn loopback_routes_do_not_require_external_connectivity() {
            revision();
            assert_eq!(path_to("127.0.0.1"), Path::Available);
            assert_eq!(path_to("[::1]"), Path::Available);
        }
    }
}
