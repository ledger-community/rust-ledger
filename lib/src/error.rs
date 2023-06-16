//! Ledger interface [Error] type and conversions

use ledger_proto::{ApduError, StatusCode};

/// Ledger interface error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "transport_usb")]
    #[error(transparent)]
    Hid(#[from] hidapi::HidError),

    #[cfg(feature = "transport_tcp")]
    #[error(transparent)]
    Tcp(#[from] tokio::io::Error),

    #[cfg(feature = "transport_ble")]
    #[error(transparent)]
    Ble(#[from] btleplug::Error),

    #[error("Unknown ledger model: {0}")]
    UnknownModel(u16),

    #[error("Unknown error")]
    Unknown,

    #[error("No devices found")]
    NoDevices,

    #[error("Invalid device index: {0}")]
    InvalidDeviceIndex(usize),

    #[error("Apdu encode/decode error: {0}")]
    Apdu(#[from] ApduError),

    /// Recognised status codes (see [StatusCode])
    #[error("Status: {0}")]
    Status(StatusCode),

    /// Unrecognised status codes
    #[error("Status: 0x{0:02x}{1:02x} (unrecognised)")]
    UnknownStatus(u8, u8),

    #[error("Request timeout")]
    Timeout,

    #[error("Device or transport closed")]
    Closed,

    #[error("Empty response payload")]
    EmptyResponse,

    #[error("Unexpected response payload")]
    UnexpectedResponse,

    #[error("Device in use")]
    DeviceInUse,

    #[error("Already running application ({0})")]
    ApplicationLoaded(String),
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_e: tokio::time::error::Elapsed) -> Self {
        Self::Timeout
    }
}
