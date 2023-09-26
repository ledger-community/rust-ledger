//! USB HID transport implementation
//!
//! # SAFETY
//!
//! This is _not_ `Send` or thread safe, see [transport][crate::transport] docs for
//! more details.
//!

use std::{ffi::CString, fmt::Display, io::ErrorKind, time::Duration};

use hidapi::{HidApi, HidDevice, HidError};
use tracing::{debug, error, trace, warn};

use crate::{
    info::{LedgerInfo, Model},
    Error,
};

use super::{Exchange, Transport};

/// Basic USB device information
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct UsbInfo {
    #[cfg_attr(feature = "clap", clap(long, value_parser=u16_parse_hex))]
    /// USB Device Vendor ID (VID) in hex
    pub vid: u16,

    #[cfg_attr(feature = "clap", clap(long, value_parser=u16_parse_hex))]
    /// USB Device Product ID (PID) in hex
    pub pid: u16,

    #[cfg_attr(feature = "clap", clap(long))]
    /// Device path
    pub path: Option<String>,
}

impl Display for UsbInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vid, self.pid)
    }
}

/// Helper to pass VID/PID pairs from hex values
#[cfg(feature = "clap")]
fn u16_parse_hex(s: &str) -> Result<u16, std::num::ParseIntError> {
    u16::from_str_radix(s, 16)
}

/// USB HID based transport
///
/// # Safety
/// Due to `hidapi` this is not thread safe an only one instance must exist in an application.
/// If you don't need low-level control see [crate::LedgerProvider] for a tokio based wrapper.
pub struct UsbTransport {
    hid_api: HidApi,
}

/// USB HID based device
pub struct UsbDevice {
    pub info: UsbInfo,
    device: HidDevice,
}

/// Ledger USB VID
pub const LEDGER_VID: u16 = 0x2c97;

impl UsbTransport {
    /// Create a new [UsbTransport]
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            hid_api: HidApi::new()?,
        })
    }
}

// With the unstable_async_trait feature we can (correctly) mark this as non-send
// however [async_trait] can't easily differentiate between send and non-send so we're
// exposing this as Send for the moment

#[cfg(feature = "unstable_async_trait")]
impl !Send for UsbDevice {}
#[cfg(feature = "unstable_async_trait")]
impl !Sync for UsbDevice {}

#[cfg(feature = "unstable_async_trait")]
impl !Send for UsbTransport {}
#[cfg(feature = "unstable_async_trait")]
impl !Sync for UsbTransport {}

/// WARNING: THIS IS A LIE TO APPEASE `async_trait`
#[cfg(not(feature = "unstable_async_trait"))]
unsafe impl Send for UsbTransport {}

#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Transport for UsbTransport {
    type Filters = ();
    type Info = UsbInfo;
    type Device = UsbDevice;

    /// List available devices using the [UsbTransport]
    async fn list(&mut self, _filters: Self::Filters) -> Result<Vec<LedgerInfo>, Error> {
        debug!("Listing USB devices");

        // Refresh available devices
        // TODO: determine whether the refresh call is critical (or, useful?)
        if let Err(e) = self.hid_api.refresh_devices() {
            warn!("Failed to refresh devices: {e:?}");
        }

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Fetch list of devices, filtering for ledgers
        let devices: Vec<_> = self
            .hid_api
            .device_list()
            .filter(|d| d.vendor_id() == LEDGER_VID)
            .map(|d| LedgerInfo {
                model: Model::from_pid(d.product_id()),
                conn: UsbInfo {
                    vid: d.vendor_id(),
                    pid: d.product_id(),
                    path: Some(d.path().to_string_lossy().to_string()),
                }
                .into(),
            })
            .collect();

        debug!("devices: {:?}", devices);

        Ok(devices)
    }

    /// Connect to a device using the usb transport
    async fn connect(&mut self, info: UsbInfo) -> Result<UsbDevice, Error> {
        debug!("Connecting to USB device: {:?}", info);

        // If we have a path, use this to connect
        let d = if let Some(p) = &info.path {
            let p = CString::new(p.clone()).unwrap();
            self.hid_api.open_path(&p)

        // Otherwise, fallback to (non unique!) vid:pid
        } else {
            self.hid_api.open(info.vid, info.pid)
        };

        match d {
            Ok(d) => {
                debug!("Connected to USB device: {:?}", info);
                Ok(UsbDevice { device: d, info })
            }
            Err(e) => {
                debug!("Failed to connect to USB device: {:?}", e);
                Err(e.into())
            }
        }
    }
}

// HID packet length (header + data)
const HID_PACKET_LEN: usize = 64;

// Five bytes: channnel (0x101), tag (0x05), sequence index
const HID_HEADER_LEN: usize = 5;

impl UsbDevice {
    /// Write an APDU to the device
    pub fn write(&mut self, apdu: &[u8]) -> Result<(), Error> {
        debug!("Write APDU");

        // Setup outgoing data buffer with length prefix
        let mut data = Vec::with_capacity(apdu.len() + 2);
        data.extend_from_slice(&(apdu.len() as u16).to_be_bytes());
        data.extend_from_slice(apdu);

        debug!("TX: {:02x?}", data);

        // Write data in 64 byte chunks
        for (i, c) in data.chunks(HID_PACKET_LEN - HID_HEADER_LEN).enumerate() {
            trace!("Writing chunk {} of {} bytes", i, c.len());

            // Setup HID packet with header and data
            let mut packet = Vec::with_capacity(HID_PACKET_LEN + 1);

            // Zero prefix for unknown reasons
            packet.push(0x00);

            // Header channnel (0x101), tag (0x05), sequence index
            packet.extend_from_slice(&[0x01, 0x01, 0x05]);
            packet.extend_from_slice(&(i as u16).to_be_bytes());
            // Remaining data
            packet.extend_from_slice(c);

            trace!("Write: 0x{:02x?}", packet);

            // Write HID packet
            self.device.write(&packet)?;
        }

        Ok(())
    }

    /// Read an APDU from the device
    pub fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
        debug!("Read APDU");

        let mut buff = [0u8; HID_PACKET_LEN + 1];

        // Read first chunk of response
        // Timeout argument applied here as once the reply has started timeout bounds should be more consistent
        let n = match self
            .device
            .read_timeout(&mut buff, timeout.as_millis() as i32)
        {
            Ok(n) => n,
            Err(HidError::IoError { error }) if error.kind() == ErrorKind::TimedOut => {
                return Err(Error::Timeout)
            }
            Err(e) => return Err(e.into()),
        };

        // Check read length is valid for following operations
        if n == 0 {
            error!("Empty response");
            return Err(Error::EmptyResponse);
        } else if n < 7 {
            error!("Unexpected read length {n}");
            return Err(Error::UnexpectedResponse);
        }

        // Check header matches expectations
        if buff[..5] != [0x01, 0x01, 0x05, 0x00, 0x00] {
            error!("Unexpected response header: {:02x?}", &buff[..5]);
            return Err(Error::UnexpectedResponse);
        }

        trace!("initial read: {buff:02x?}");

        // Parse response length
        let len = u16::from_be_bytes([buff[5], buff[6]]) as usize;

        trace!("Read len: {len}");

        // Setup response buffer and add any remaining data
        let mut resp = Vec::with_capacity(len);

        let data_len = len.min(n - 7);
        resp.extend_from_slice(&buff[7..][..data_len]);

        // Read following chunks if required
        let mut seq_idx = 1;
        while resp.len() < len {
            let rem = len - resp.len();

            trace!("Read chunk {seq_idx} ({rem} bytes remaining)");

            // Read next chunk, constant timeout as chunks should be sent end-to-end
            let n = match self.device.read_timeout(&mut buff, 500) {
                Ok(n) => n,
                Err(e) => return Err(e.into()),
            };

            if n < 5 {
                error!("Invalid chunk length {n}");
                return Err(Error::UnexpectedResponse);
            }

            // Check header and sequence index
            if buff[..3] != [0x01, 0x01, 0x05] {
                error!("Unexpected response header: {:02x?}", &buff[..5]);
                return Err(Error::UnexpectedResponse);
            }
            if u16::from_be_bytes([buff[3], buff[4]]) != seq_idx {
                error!("Unexpected sequence index: {:02x?}", &buff[5..7]);
                return Err(Error::UnexpectedResponse);
            }

            // Add to response buffer
            let data_len = rem.min(n - 5);
            resp.extend_from_slice(&buff[5..][..data_len]);
            seq_idx += 1;
        }

        debug!("RX: {:02x?}", resp);

        Ok(resp)
    }

    pub(crate) async fn is_connected(&self) -> Result<bool, Error> {
        Ok(self.device.get_device_info().is_ok())
    }
}

/// [Exchange] impl for sending APDUs to a [UsbDevice]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Exchange for UsbDevice {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        // Write APDU command, chunked for HID transport
        self.write(command)?;
        // Read APDU response, chunked for HID transport
        self.read(timeout)
    }
}
