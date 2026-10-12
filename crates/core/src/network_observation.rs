use serde::Serialize;
#[cfg(windows)]
use std::{mem::size_of, net::Ipv4Addr};

#[derive(Clone, Debug, Serialize)]
pub struct NetworkObservation {
    pub source: &'static str,
    pub interfaces: Vec<ObservedInterface>,
    pub default_gateways: Vec<ObservedGateway>,
    pub wan_visible_ip: ObservedValue,
    pub mappings: Vec<MappingObservation>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ObservedInterface {
    pub interface_index: u32,
    pub name: String,
    pub ipv4: Vec<String>,
    pub gateways: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ObservedGateway {
    pub interface_index: u32,
    pub address: String,
    pub source: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservedValue {
    pub state: ObservationState,
    pub source: &'static str,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationState {
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingMethod {
    UpnpIgd,
    Pcp,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingTransport {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingState {
    NotAttempted,
}

#[derive(Clone, Debug, Serialize)]
pub struct MappingObservation {
    pub method: MappingMethod,
    pub transport: MappingTransport,
    pub state: MappingState,
}

impl Default for ObservedValue {
    fn default() -> Self {
        Self {
            state: ObservationState::Unknown,
            source: "No external visible-IP observation is performed in M1",
            value: None,
        }
    }
}

impl Default for NetworkObservation {
    fn default() -> Self {
        Self {
            source: "UNKNOWN (Windows IP Helper observation unavailable)",
            interfaces: Vec::new(),
            default_gateways: Vec::new(),
            wan_visible_ip: ObservedValue::default(),
            mappings: [MappingMethod::UpnpIgd, MappingMethod::Pcp]
                .into_iter()
                .flat_map(|method| {
                    [MappingTransport::Tcp, MappingTransport::Udp].map(move |transport| {
                        MappingObservation {
                            method,
                            transport,
                            state: MappingState::NotAttempted,
                        }
                    })
                })
                .collect(),
        }
    }
}

/// Reads local IP Helper tables only. It does not contact a router or firewall.
pub fn observe_network() -> NetworkObservation {
    let mut result = NetworkObservation {
        source: "Windows IP Helper (GetAdaptersAddresses, GetIpForwardTable2)",
        ..Default::default()
    };

    #[cfg(not(windows))]
    {
        result.source = "UNKNOWN (Windows IP Helper is unavailable on this platform)";
    }

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::ERROR_BUFFER_OVERFLOW,
            NetworkManagement::IpHelper::{
                FreeMibTable, GetAdaptersAddresses, GetIpForwardTable2, GAA_FLAG_INCLUDE_GATEWAYS,
                IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_GATEWAY_ADDRESS_LH,
                IP_ADAPTER_UNICAST_ADDRESS_LH, MIB_IPFORWARD_TABLE2,
            },
            Networking::WinSock::AF_INET,
        };

        let mut byte_count = 15 * 1024u32;
        let mut storage = vec![0usize; (byte_count as usize).div_ceil(size_of::<usize>())];
        let mut status = GetAdaptersAddresses(
            AF_INET as u32,
            GAA_FLAG_INCLUDE_GATEWAYS,
            std::ptr::null(),
            storage.as_mut_ptr().cast(),
            &mut byte_count,
        );
        if status == ERROR_BUFFER_OVERFLOW && byte_count <= 1024 * 1024 {
            storage.resize((byte_count as usize).div_ceil(size_of::<usize>()), 0);
            status = GetAdaptersAddresses(
                AF_INET as u32,
                GAA_FLAG_INCLUDE_GATEWAYS,
                std::ptr::null(),
                storage.as_mut_ptr().cast(),
                &mut byte_count,
            );
        }
        if status == 0 {
            let mut adapter = storage.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            while !adapter.is_null() {
                let entry = &*adapter;
                let index = entry.Anonymous1.Anonymous.IfIndex;
                let name = if entry.FriendlyName.is_null() {
                    String::new()
                } else {
                    let mut len = 0;
                    while *entry.FriendlyName.add(len) != 0 {
                        len += 1;
                    }
                    String::from_utf16_lossy(std::slice::from_raw_parts(entry.FriendlyName, len))
                };
                let mut interface = ObservedInterface {
                    interface_index: index,
                    name,
                    ..Default::default()
                };
                read_sockaddr_list(
                    entry
                        .FirstUnicastAddress
                        .cast::<IP_ADAPTER_UNICAST_ADDRESS_LH>(),
                    |addr| interface.ipv4.push(addr),
                );
                read_sockaddr_list(
                    entry
                        .FirstGatewayAddress
                        .cast::<IP_ADAPTER_GATEWAY_ADDRESS_LH>(),
                    |addr| interface.gateways.push(addr),
                );
                result.interfaces.push(interface);
                adapter = entry.Next;
            }
        } else {
            result.source = "Windows IP Helper (interface query unavailable)";
        }

        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        if GetIpForwardTable2(AF_INET, &mut table) == 0 && !table.is_null() {
            let count = (*table).NumEntries as usize;
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
            for row in rows {
                if row.DestinationPrefix.PrefixLength == 0 {
                    let next_hop = row.NextHop.Ipv4;
                    if next_hop.sin_family == AF_INET && next_hop.sin_addr.S_un.S_addr != 0 {
                        let ip =
                            Ipv4Addr::from(u32::from_be(next_hop.sin_addr.S_un.S_addr)).to_string();
                        result.default_gateways.push(ObservedGateway {
                            interface_index: row.InterfaceIndex,
                            address: ip,
                            source: "GetIpForwardTable2 default route",
                        });
                    }
                }
            }
            FreeMibTable(table.cast());
        }
    }
    result
}

#[cfg(windows)]
unsafe fn read_sockaddr_list<T>(mut node: *mut T, mut visit: impl FnMut(String))
where
    T: SockaddrNode,
{
    while !node.is_null() {
        let item = &*node;
        if let Some(ip) = item.ipv4() {
            visit(ip);
        }
        node = item.next();
    }
}

#[cfg(windows)]
trait SockaddrNode {
    unsafe fn next(&self) -> *mut Self;
    unsafe fn ipv4(&self) -> Option<String>;
}

#[cfg(windows)]
unsafe fn socket_address_ipv4(
    address: *mut windows_sys::Win32::Networking::WinSock::SOCKADDR,
    length: i32,
) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    if address.is_null()
        || length < size_of::<SOCKADDR_IN>() as i32
        || (*address).sa_family != AF_INET
    {
        return None;
    }
    let address = &*address.cast::<SOCKADDR_IN>();
    Some(Ipv4Addr::from(u32::from_be(address.sin_addr.S_un.S_addr)).to_string())
}

#[cfg(windows)]
impl SockaddrNode
    for windows_sys::Win32::NetworkManagement::IpHelper::IP_ADAPTER_UNICAST_ADDRESS_LH
{
    unsafe fn next(&self) -> *mut Self {
        self.Next
    }
    unsafe fn ipv4(&self) -> Option<String> {
        socket_address_ipv4(self.Address.lpSockaddr, self.Address.iSockaddrLength)
    }
}

#[cfg(windows)]
impl SockaddrNode
    for windows_sys::Win32::NetworkManagement::IpHelper::IP_ADAPTER_GATEWAY_ADDRESS_LH
{
    unsafe fn next(&self) -> *mut Self {
        self.Next
    }
    unsafe fn ipv4(&self) -> Option<String> {
        socket_address_ipv4(self.Address.lpSockaddr, self.Address.iSockaddrLength)
    }
}
