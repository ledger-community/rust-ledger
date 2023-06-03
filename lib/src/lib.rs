//! A Ledger hardware wallet communication library
//!
//! [Device] provides a high-level API for exchanging APDUs with Ledger devices using the [ledger_proto] traits.
//! This is suitable for extension with application-specific interface traits, and automatically
//! implemented over [Exchange] for low-level byte exchange with devices.
//!
//! [LedgerProvider] and [LedgerHandle] provide a high-level tokio-compatible [Transport]
//! for application integration, supporting connecting to and interacting with ledger devices.
//! This uses a pinned thread to avoid thread safety issues with `hidapi` and async executors.
//!
//! Low-level [Transport] implementations are provided for [USB/HID](transport::UsbTransport),
//! [BLE](transport::BleTransport) and [TCP](transport::TcpTransport), with a [Generic](transport::GenericTransport)
//! implementation providing a common interface over all enabled transports.
//!
//! ## Safety
//!
//! Transports are currently marked as `Send` due to limitations of [async_trait] and are NOT all
//! thread safe. If you're calling this from an async context, please use [LedgerProvider].
//!
//! This will be corrected when the unstable async trait feature is stabilised,
//! which until then can be opted-into using the `unstable_async_trait` feature
//!
//! ## Examples
//!
//! ```no_run
//! use ledger_lib::{LedgerProvider, Filters, Transport, Device, DEFAULT_TIMEOUT};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Fetch provider handle
//!     let mut provider = LedgerProvider::init().await;
//!
//!     // List available devices
//!     let devices = provider.list(Filters::Any).await?;
//!
//!     // Check we have -a- device to connect to
//!     if devices.is_empty() {
//!         return Err(anyhow::anyhow!("No devices found"));
//!     }
//!
//!     // Connect to the first device
//!     let mut ledger = provider.connect(devices[0].clone()).await?;
//!
//!     // Request device information
//!     let info = ledger.app_info(DEFAULT_TIMEOUT).await?;
//!     println!("info: {info:?}");
//!
//!     Ok(())
//! }
//! ```

#![cfg_attr(feature = "unstable_async_trait", feature(async_fn_in_trait))]
#![feature(negative_impls)]

use std::time::Duration;

pub mod info;
pub use info::LedgerInfo;

mod error;
pub use error::Error;

pub mod transport;
pub use transport::Transport;

mod provider;
pub use provider::{LedgerHandle, LedgerProvider};

mod device;
pub use device::Device;

/// Default timeout helper for use with [Device] and [Exchange]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Device discovery filter
#[derive(Copy, Clone, Debug, PartialEq, strum::Display)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[non_exhaustive]
pub enum Filters {
    /// List all devices available using supported transport
    Any,
    /// List only HID devices
    Hid,
    /// List only TCP devices
    Tcp,
    /// List only BLE device
    Ble,
}

impl Default for Filters {
    fn default() -> Self {
        Self::Any
    }
}

/// [Exchange] trait provides a low-level interface for byte-wise exchange of APDU commands with a ledger devices
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
pub trait Exchange {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error>;
}

/// Blanket [Exchange] impl for mutable references
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl<T: Exchange + Send> Exchange for &mut T {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        <T as Exchange>::exchange(self, command, timeout).await
    }
}
