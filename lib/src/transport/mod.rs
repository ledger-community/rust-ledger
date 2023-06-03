//! Low-level transport implementations for communication with ledger devices and nano apps
//!
//! Transports are gated by `transport_X` features, while [GenericTransport] and
//! [GenericDevice] provide an abstraction over enabled transports.
//!
//! # Safety
//! [UsbTransport] (and thus [GenericTransport] when `transport_usb` feature is enabled)
//! is _not_ `Send` or `Sync`, however this is marked as such to appease `async_trait`...
//!
//! Once `async_trait` has stabilised transports can be marked correctly.
//! (This is also implemented under the `unstable_async_trait` feature)
//! Until then, use [LedgerProvider](crate::LedgerProvider) for a `Sync + Send` interface or
//!  be _super sure_ you're not going to call transports from a multi-threaded context.

use std::{fmt::Debug, time::Duration};

use tracing::{debug, warn};

#[cfg(feature = "transport_usb")]
mod usb;
#[cfg(feature = "transport_usb")]
pub use usb::{UsbDevice, UsbInfo, UsbTransport};

#[cfg(feature = "transport_ble")]
mod ble;
#[cfg(feature = "transport_ble")]
pub use ble::{BleDevice, BleInfo, BleTransport};

#[cfg(feature = "transport_tcp")]
mod tcp;
#[cfg(feature = "transport_tcp")]
pub use tcp::{TcpDevice, TcpInfo, TcpTransport};

use crate::{
    info::{ConnInfo, LedgerInfo},
    Error, Exchange, Filters,
};

/// [Transport] trait provides an abstract interface for transport implementations
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
pub trait Transport {
    /// Connection filters
    type Filters: Debug;
    /// Device information, used for listing and connecting
    type Info: Debug;
    /// Device handle for interacting with the device
    type Device: Exchange;

    /// List available devices
    async fn list(&mut self, filters: Self::Filters) -> Result<Vec<LedgerInfo>, Error>;

    /// Connect to a device using info from a previous list operation
    async fn connect(&mut self, info: Self::Info) -> Result<Self::Device, Error>;
}

/// [GenericTransport] for device communication, abstracts underlying transport types
///
pub struct GenericTransport {
    #[cfg(feature = "transport_usb")]
    usb: UsbTransport,

    #[cfg(feature = "transport_ble")]
    ble: BleTransport,

    #[cfg(feature = "transport_tcp")]
    tcp: TcpTransport,
}

/// [GenericDevice] for communication with ledger devices, abstracts underlying transport types
///
pub enum GenericDevice {
    #[cfg(feature = "transport_usb")]
    Usb(UsbDevice),

    #[cfg(feature = "transport_ble")]
    Ble(BleDevice),

    #[cfg(feature = "transport_tcp")]
    Tcp(TcpDevice),
}

impl GenericTransport {
    /// Create a new [GenericTransport] with all endabled transports
    pub async fn new() -> Result<Self, Error> {
        debug!("Initialising GenericTransport");

        Ok(Self {
            #[cfg(feature = "transport_usb")]
            usb: UsbTransport::new()?,

            #[cfg(feature = "transport_ble")]
            ble: BleTransport::new().await?,

            #[cfg(feature = "transport_tcp")]
            tcp: TcpTransport::new()?,
        })
    }
}

#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Transport for GenericTransport {
    type Filters = Filters;
    type Info = LedgerInfo;
    type Device = GenericDevice;

    /// List available ledger devices using all enabled transports
    async fn list(&mut self, filters: Filters) -> Result<Vec<LedgerInfo>, Error> {
        let mut devices = vec![];

        #[cfg(feature = "transport_usb")]
        if filters == Filters::Any || filters == Filters::Hid {
            let mut d = self.usb.list(()).await?;
            devices.append(&mut d);
        }

        #[cfg(feature = "transport_ble")]
        if filters == Filters::Any || filters == Filters::Ble {
            // BLE discovery is allowed to fail if not explictly selected
            // as dbus does not always provide the relevant service (eg. under WSL)
            // TODO: work out whether we can detect this to separate no BLE from discovery failure

            match self.ble.list(()).await {
                Ok(mut d) => devices.append(&mut d),
                Err(e) if filters == Filters::Any => {
                    warn!("BLE discovery failed: {e:?}");
                }
                Err(e) => return Err(e),
            }
        }

        #[cfg(feature = "transport_tcp")]
        if filters == Filters::Any || filters == Filters::Tcp {
            let mut d = self.tcp.list(()).await?;
            devices.append(&mut d);
        }

        Ok(devices)
    }

    /// Connect to a ledger device using available transports
    ///
    async fn connect(&mut self, info: LedgerInfo) -> Result<GenericDevice, Error> {
        debug!("Connecting to device: {:?}", info);

        let d = match info.conn {
            #[cfg(feature = "transport_usb")]
            ConnInfo::Usb(i) => self.usb.connect(i).await.map(GenericDevice::Usb)?,
            #[cfg(feature = "transport_tcp")]
            ConnInfo::Tcp(i) => self.tcp.connect(i).await.map(GenericDevice::Tcp)?,
            #[cfg(feature = "transport_ble")]
            ConnInfo::Ble(i) => self.ble.connect(i).await.map(GenericDevice::Ble)?,
        };

        Ok(d)
    }
}

impl GenericDevice {
    /// Fetch connection info for a device
    pub fn info(&self) -> ConnInfo {
        match self {
            GenericDevice::Usb(d) => d.info.clone().into(),
            GenericDevice::Ble(d) => d.info.clone().into(),
            GenericDevice::Tcp(d) => d.info.clone().into(),
        }
    }

    pub(crate) async fn is_connected(&self) -> Result<bool, Error> {
        match self {
            GenericDevice::Usb(d) => d.is_connected().await,
            GenericDevice::Ble(d) => d.is_connected().await,
            GenericDevice::Tcp(d) => d.is_connected().await,
        }
    }
}

#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Exchange for GenericDevice {
    /// Exchange an APDU with the [GenericDevice]
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        match self {
            #[cfg(feature = "transport_usb")]
            Self::Usb(d) => d.exchange(command, timeout).await,
            Self::Ble(d) => d.exchange(command, timeout).await,
            Self::Tcp(d) => d.exchange(command, timeout).await,
        }
    }
}

#[cfg(feature = "transport_usb")]
impl From<UsbDevice> for GenericDevice {
    fn from(value: UsbDevice) -> Self {
        Self::Usb(value)
    }
}

#[cfg(feature = "transport_tcp")]
impl From<TcpDevice> for GenericDevice {
    fn from(value: TcpDevice) -> Self {
        Self::Tcp(value)
    }
}

#[cfg(feature = "transport_ble")]
impl From<BleDevice> for GenericDevice {
    fn from(value: BleDevice) -> Self {
        Self::Ble(value)
    }
}
