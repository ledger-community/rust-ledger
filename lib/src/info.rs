//! Device information types and connection filters

use std::collections::BTreeMap;

use strum::{Display, EnumString};
use uuid::{uuid, Uuid};

use crate::Filters;

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

impl LedgerInfo {
    /// Fetch connection kind enumeration
    pub fn kind(&self) -> ConnType {
        match &self.conn {
            #[cfg(feature = "transport_usb")]
            ConnInfo::Usb(_) => ConnType::Usb,
            #[cfg(feature = "transport_tcp")]
            ConnInfo::Tcp(_) => ConnType::Tcp,
            #[cfg(feature = "transport_ble")]
            ConnInfo::Ble(_) => ConnType::Ble,
        }
    }
}

/// Ledger device models
#[derive(Copy, Clone, PartialEq, Debug, Display, EnumString)]
pub enum Model {
    /// Nano S
    NanoS,
    /// Nano S Plus
    NanoSPlus,
    /// Nano X
    NanoX,
    /// Stax
    Stax,
    /// Flex
    Flex,
    /// Nano Gen5
    NanoGen5,
    /// Unknown model
    Unknown { usb_pid: Option<u16> },
}

impl Model {
    /// Convert a USB PID to a [Model] kind
    pub fn from_usb_pid(usb_pid: u16) -> Model {
        INTERNAL_DEVICE_INFOS
            .iter()
            .find_map(|device_info| {
                device_info
                    .matches_usb_pid(usb_pid)
                    .then_some(device_info.model)
            })
            .unwrap_or_else(|| Model::Unknown {
                usb_pid: Some(usb_pid),
            })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BleSpec {
    pub service_uuid: Uuid,
    pub notify_uuid: Uuid,
    pub write_uuid: Uuid,
    pub write_cmd_uuid: Uuid,
}

struct InternalDeviceInfo {
    model: Model,
    legacy_usb_product_id: u16,
    product_id_mm: u16,
    ble_specs: Vec<BleSpec>,
}

impl InternalDeviceInfo {
    fn matches_usb_pid(&self, usb_pid: u16) -> bool {
        // First compare the passed pid with the legacy product id, if that doesn't match, use product_id_mm.
        // The logic was taken from here:
        // https://github.com/LedgerHQ/ledger-live/blob/b870b8018319b7489c39c92e743adcfd4e33e948/libs/ledgerjs/packages/devices/src/index.ts#L190
        // (the legacy product id match will probably only work for some early variants of NanoS, but it's
        // still better to be consistent with Ledger Live, just in case).
        usb_pid == self.legacy_usb_product_id || usb_pid >> 8 == self.product_id_mm
    }
}

// The table was taken from here:
// https://github.com/LedgerHQ/ledger-live/blob/b870b8018319b7489c39c92e743adcfd4e33e948/libs/ledgerjs/packages/devices/src/index.ts#L41
lazy_static::lazy_static! {
    static ref INTERNAL_DEVICE_INFOS: Vec<InternalDeviceInfo> = {
        vec![
            InternalDeviceInfo {
                model: Model::NanoS,
                legacy_usb_product_id: 0x0001,
                product_id_mm: 0x10,
                ble_specs: vec![],
            },
            InternalDeviceInfo {
                model: Model::NanoX,
                legacy_usb_product_id: 0x0004,
                product_id_mm: 0x40,
                ble_specs: vec![
                    BleSpec {
                        service_uuid: uuid!("13d63400-2c97-0004-0000-4c6564676572"),
                        notify_uuid: uuid!("13d63400-2c97-0004-0001-4c6564676572"),
                        write_uuid: uuid!("13d63400-2c97-0004-0002-4c6564676572"),
                        write_cmd_uuid: uuid!("13d63400-2c97-0004-0003-4c6564676572"),
                    },
                ],
            },
            InternalDeviceInfo {
                model: Model::NanoSPlus,
                legacy_usb_product_id: 0x0005,
                product_id_mm: 0x50,
                ble_specs: vec![],
            },
            InternalDeviceInfo {
                model: Model::Stax,
                legacy_usb_product_id: 0x0006,
                product_id_mm: 0x60,
                ble_specs: vec![
                    BleSpec {
                        service_uuid: uuid!("13d63400-2c97-6004-0000-4c6564676572"),
                        notify_uuid: uuid!("13d63400-2c97-6004-0001-4c6564676572"),
                        write_uuid: uuid!("13d63400-2c97-6004-0002-4c6564676572"),
                        write_cmd_uuid: uuid!("13d63400-2c97-6004-0003-4c6564676572"),
                    },
                ],
            },
            InternalDeviceInfo {
                model: Model::Flex,
                legacy_usb_product_id: 0x0007,
                product_id_mm: 0x70,
                ble_specs: vec![
                    BleSpec {
                        service_uuid: uuid!("13d63400-2c97-3004-0000-4c6564676572"),
                        notify_uuid: uuid!("13d63400-2c97-3004-0001-4c6564676572"),
                        write_uuid: uuid!("13d63400-2c97-3004-0002-4c6564676572"),
                        write_cmd_uuid: uuid!("13d63400-2c97-3004-0003-4c6564676572"),
                    },
                ],
            },
            InternalDeviceInfo {
                model: Model::NanoGen5,
                legacy_usb_product_id: 0x0008,
                product_id_mm: 0x80,
                ble_specs: vec![
                    BleSpec {
                        service_uuid: uuid!("13d63400-2c97-8004-0000-4c6564676572"),
                        notify_uuid: uuid!("13d63400-2c97-8004-0001-4c6564676572"),
                        write_uuid: uuid!("13d63400-2c97-8004-0002-4c6564676572"),
                        write_cmd_uuid: uuid!("13d63400-2c97-8004-0003-4c6564676572"),
                    },
                ],
            },
        ]
    };

    static ref BLE_SPECS_BY_SERVICE_UUID: BTreeMap<Uuid, &'static BleSpec> = {
        INTERNAL_DEVICE_INFOS
            .iter()
            .flat_map(|dev_info| dev_info.ble_specs.iter())
            .map(|ble_spec| (ble_spec.service_uuid, ble_spec))
            .collect()
    };

    static ref INTERNAL_DEVICE_INFOS_BY_BLE_SERVICE_UUID: BTreeMap<Uuid, &'static InternalDeviceInfo> = {
        INTERNAL_DEVICE_INFOS
            .iter()
            .flat_map(|dev_info| {
                dev_info.ble_specs.iter().map(move |ble_spec| (ble_spec.service_uuid, dev_info))
            })
            .collect()
    };
}

pub fn model_by_ble_service_uuid(service_uuid: &Uuid) -> Option<Model> {
    INTERNAL_DEVICE_INFOS_BY_BLE_SERVICE_UUID
        .get(service_uuid)
        .map(|dev_info| dev_info.model)
}

pub fn ble_spec_by_service_uuid(service_uuid: &Uuid) -> Option<&'static BleSpec> {
    BLE_SPECS_BY_SERVICE_UUID.get(service_uuid).copied()
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

/// Ledger connection types
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ConnType {
    Usb,
    Tcp,
    Ble,
}

impl From<ConnType> for Filters {
    /// Convert a connection type to a discovery filter
    fn from(value: ConnType) -> Self {
        match value {
            ConnType::Usb => Filters::Hid,
            ConnType::Tcp => Filters::Tcp,
            ConnType::Ble => Filters::Ble,
        }
    }
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
