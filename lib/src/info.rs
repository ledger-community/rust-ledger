//! Device information types and connection filters

use strum::{Display, EnumString};

use super::transport;

/// Ledger device information
#[derive(Clone, PartialEq, Debug)]
pub struct LedgerInfo {
    /// Device Model
    pub model: Model,

    /// Device connection information
    pub conn: ConnInfo,
}

impl std::fmt::Display for LedgerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.model, self.conn)
    }
}

/// Ledger device models
#[derive(Clone, PartialEq, Debug, Display, EnumString)]
pub enum Model {
    /// Nano S
    NanoS,
    /// Nano S Plus
    NanoSPlus,
    /// Nano X
    NanoX,
    /// Stax
    Stax,
    /// Unknown model
    Unknown(u16),
}

impl Model {
    /// Convert a USB PID to a [Model] kind
    ///
    /// Note that ledger PIDs vary depending on the device state so only the top byte is used
    /// for matching.
    pub fn from_pid(pid: u16) -> Model {
        match pid & 0xFF00 {
            // TODO: support all the models
            //0x0001 => Ok(Model::NanoS),
            0x4000 => Model::NanoX,
            0x5000 => Model::NanoSPlus,
            //0x0006 => Ok(Model::Stax),
            _ => Model::Unknown(pid),
        }
    }
}

/// Ledger connection information
#[derive(Clone, PartialEq, Debug)]
pub enum ConnInfo {
    #[cfg(feature = "transport_usb")]
    Usb(transport::UsbInfo),
    #[cfg(feature = "transport_tcp")]
    Tcp(transport::TcpInfo),
    #[cfg(feature = "transport_ble")]
    Ble(transport::BleInfo),
}

impl std::fmt::Display for ConnInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "transport_usb")]
            Self::Usb(i) => write!(f, "HID {}", i),
            #[cfg(feature = "transport_tcp")]
            Self::Tcp(i) => write!(f, "TCP {}", i),
            #[cfg(feature = "transport_ble")]
            Self::Ble(i) => write!(f, "BLE {}", i),
        }
    }
}

#[cfg(feature = "transport_usb")]
impl From<transport::UsbInfo> for ConnInfo {
    fn from(value: transport::UsbInfo) -> Self {
        Self::Usb(value)
    }
}

#[cfg(feature = "transport_tcp")]
impl From<transport::TcpInfo> for ConnInfo {
    fn from(value: transport::TcpInfo) -> Self {
        Self::Tcp(value)
    }
}

#[cfg(feature = "transport_ble")]
impl From<transport::BleInfo> for ConnInfo {
    fn from(value: transport::BleInfo) -> Self {
        Self::Ble(value)
    }
}

/// Application info object
#[derive(Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub flags: ledger_proto::apdus::AppFlags,
}

/// Device info object
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub target_id: [u8; 4],
    pub se_version: String,
    pub mcu_version: String,
    pub flags: Vec<u8>,
}
