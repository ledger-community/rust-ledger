//! A Ledger hardware wallet communication library
//!
//! [Device] provides a high-level API for exchanging APDUs with Ledger devices using the [ledger_proto] traits.
//! This is suitable for extension with application-specific interface traits, and automatically
//! implemented over [Exchange] for low-level byte exchange with devices.
//!
//! [LedgerProvider] and [LedgerHandle] provide a high-level tokio-compatible [Transport]
//! for application integration, supporting connecting to and interacting with ledger devices.
//!
//! Low-level [Transport] implementations are provided for [USB/HID](transport::UsbTransport),
//! [BLE](transport::BleTransport) and [TCP](transport::TcpTransport), with a [Generic](transport::GenericTransport)
//! implementation providing a common interface over all enabled transports.
//!
//! Note that futures produced by async methods of [Transport] and [Device] are not `Send`, i.e. they
//! can't be used with multi-threaded async executors. The reason is that [UsbTransport](transport::UsbTransport)
//! and [UsbDevice](transport::UsbDevice) are not `Send`. This is a (probably redundant) precaution
//! against potential quirks that might occur when the underlying `hidapi` objects change threads.\
//! In a multi-threaded async environment use [LedgerProvider] and [LedgerHandle] instead.
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

use std::{future::Future, time::Duration};

use tracing::debug;

use ledger_proto::{
    apdus::{ExitAppReq, RunAppReq},
    GenericApdu, StatusCode,
};

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

/// Default timeout helper for use with [Device] and [Exchange]/[NonSendExchange]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Device discovery filter
#[derive(Copy, Clone, Debug, Default, PartialEq, strum::Display)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[non_exhaustive]
pub enum Filters {
    /// List all devices available using supported transport
    #[default]
    Any,
    /// List only HID devices
    Hid,
    /// List only TCP devices
    Tcp,
    /// List only BLE device
    Ble,
}

/// [Exchange] trait provides a low-level interface for byte-wise exchange of APDU commands with a ledger devices.
pub trait Exchange {
    fn exchange(
        &mut self,
        command: &[u8],
        timeout: Duration,
    ) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;
}

/// Blanket [Exchange] impl for mutable references
impl<T: Exchange + Send> Exchange for &mut T {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        <T as Exchange>::exchange(self, command, timeout).await
    }
}

/// [NonSendExchange] trait provides a low-level interface for byte-wise exchange of APDU commands with a ledger devices.
///
/// It is the same as [Exchange], but it doesn't enforce the `Send` bound on the returned future.
#[allow(async_fn_in_trait)]
pub trait NonSendExchange {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error>;
}

/// Blanket [NonSendExchange] impl for types that implement [Exchange].
impl<T: Exchange> NonSendExchange for T {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        <T as Exchange>::exchange(self, command, timeout).await
    }
}

/// Launch an application by name and return a device handle.
///
/// This checks whether an application is running, exits this if it
/// is not the desired application, then launches the specified app
/// by name.
///
/// Note that this function is only usable with a `Transport` whose associated `Device` type
/// implements `Exchange` (e.g. `LedgerProvider`).
///
/// # WARNING
/// Due to the constant re-enumeration of devices when changing app
/// contexts, and the lack of reported serial numbers by ledger devices,
/// this is not incredibly reliable. Use at your own risk.
///
pub async fn launch_app<T>(
    mut t: T,
    info: <T as Transport>::Info,
    app_name: &str,
    opts: &LaunchAppOpts,
    timeout: Duration,
) -> Result<<T as Transport>::Device, Error>
where
    T: Transport<Info = LedgerInfo, Filters = Filters> + Send,
    <T as Transport>::Device: Exchange + Send,
{
    let mut buff = [0u8; 256];

    debug!("Connecting to {info:?}");

    // Connect to device and fetch the currently running application
    let mut d = t.connect(info.clone()).await?;
    let i = d.app_info(timeout).await?;

    // Early-return if we're already running the correct app
    if i.name == app_name {
        debug!("Already running app {app_name}");
        return Ok(d);
    }

    // Send an exit request to the running app
    if i.name != "BOLOS" {
        debug!("Exiting running app {}", i.name);

        match d
            .request::<GenericApdu>(ExitAppReq::new(), &mut buff, timeout)
            .await
        {
            Ok(_) | Err(Error::Status(StatusCode::Ok)) => (),
            Err(e) => return Err(e),
        }

        debug!("Exit complete, reconnecting to {info:?}");

        // Close and re-connect to the device
        drop(d);

        tokio::time::sleep(Duration::from_secs(opts.reconnect_delay_s as u64)).await;

        d = reconnect(&mut t, info.clone(), opts).await?;
    }

    // Send run request
    for i in 0..10 {
        debug!("Issuing run request ({i}/10)");

        let resp = d
            .request::<GenericApdu>(RunAppReq::new(app_name), &mut buff, timeout)
            .await;

        // Handle responses
        match resp {
            // Ok response or status, app opened
            Ok(_) | Err(Error::Status(StatusCode::Ok)) => {
                debug!("Run request complete, reconnecting to {info:?}");

                // Re-connect to the device following app loading
                drop(d);

                tokio::time::sleep(Duration::from_secs(opts.reconnect_delay_s as u64)).await;

                d = reconnect(&mut t, info.clone(), opts).await?;

                return Ok(d);
            }
            // Empty response, pending reply
            Err(Error::EmptyResponse) => tokio::time::sleep(Duration::from_secs(1)).await,
            // Error response, something failed
            Err(e) => return Err(e),
        }
    }

    Err(Error::Timeout)
}

pub struct LaunchAppOpts {
    /// Delay prior to attempting device re-connection in seconds.
    ///
    /// This delay is required to allow the OS to re-enumerate the HID
    /// device.
    pub reconnect_delay_s: usize,

    /// Timeout for reconnect operations in seconds.
    pub reconnect_timeout_s: usize,
}

impl Default for LaunchAppOpts {
    fn default() -> Self {
        Self {
            reconnect_delay_s: 3,
            reconnect_timeout_s: 10,
        }
    }
}

/// Helper to reconnect to devices
async fn reconnect<T: Transport<Info = LedgerInfo, Filters = Filters>>(
    mut t: T,
    info: LedgerInfo,
    opts: &LaunchAppOpts,
) -> Result<<T as Transport>::Device, Error> {
    let mut new_info = None;

    // Build filter based on device connection type
    let filters = Filters::from(info.kind());

    debug!("Starting reconnect");

    // Await device reconnection
    for i in 0..opts.reconnect_timeout_s {
        debug!("Listing devices ({i}/{})", opts.reconnect_timeout_s);

        // List available devices
        let devices = t.list(filters).await?;

        // Look for matching device listing
        // We can't use -paths- here because the VID changes on launch
        // nor device serials, because these are always set to 1 (?!)
        match devices
            .iter()
            .find(|i| i.model == info.model && i.kind() == info.kind())
        {
            Some(i) => {
                new_info = Some(i.clone());
                break;
            }
            None => tokio::time::sleep(Duration::from_secs(1)).await,
        };
    }

    let new_info = match new_info {
        Some(v) => v,
        None => return Err(Error::Closed),
    };

    debug!("Device found, reconnecting!");

    // Connect to device using new information object
    let d = t.connect(new_info).await?;

    // Return new device connection
    Ok(d)
}
