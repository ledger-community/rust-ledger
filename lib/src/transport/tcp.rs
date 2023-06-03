use std::{
    fmt::Display,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    net::TcpStream,
};
use tracing::{debug, error};

use crate::{
    info::{LedgerInfo, Model},
    Error,
};

use super::{Exchange, Transport};

/// TCP transport implementation for interacting with Speculos via the TCP APDU socket
#[derive(Default)]
pub struct TcpTransport {}

/// TCP based device
pub struct TcpDevice {
    s: TcpStream,
    pub info: TcpInfo,
}

/// TCP device information
#[derive(Clone, PartialEq, Debug)]
pub struct TcpInfo {
    pub addr: SocketAddr,
}

impl Default for TcpInfo {
    fn default() -> Self {
        Self {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1237)),
        }
    }
}

impl Display for TcpInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

impl TcpTransport {
    /// Create a new [TcpTransport] instance
    pub fn new() -> Result<Self, Error> {
        Ok(Self {})
    }
}

#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Transport for TcpTransport {
    type Filters = ();
    type Info = TcpInfo;
    type Device = TcpDevice;

    /// List available devices using the [TcpTransport]
    ///
    /// (This looks for a speculos socket on the default port and returns a device if found,
    /// if you want to connect to a specific device use [TcpTransport::connect])
    async fn list(&mut self, _filters: Self::Filters) -> Result<Vec<LedgerInfo>, Error> {
        let mut devices = vec![];

        // Check whether speculos socket is open on the default port
        let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 1237);

        if let Ok(_s) = TcpStream::connect(&addr).await {
            // TODO: fill in model if we can request this?
            devices.push(LedgerInfo {
                conn: TcpInfo { addr }.into(),
                model: Model::Unknown(0),
            });
        }

        Ok(devices)
    }

    /// Connect to a TCP device using the provided [TcpInfo]
    async fn connect(&mut self, info: TcpInfo) -> Result<TcpDevice, Error> {
        debug!("Connecting to: {:?}", info);

        // Connect to provided TCP socket
        let s = match TcpStream::connect(info.addr).await {
            Ok(v) => v,
            Err(e) => {
                error!("TCP connection failed: {:?}", e);
                return Err(e.into());
            }
        };

        // Return TCP device handle
        Ok(TcpDevice { s, info })
    }
}

impl TcpDevice {
    /// Internal helper to write command data
    async fn write_command(&mut self, req: &[u8]) -> Result<(), Error> {
        // Setup data buffer to send
        let mut buff = vec![0; 4 + req.len()];

        // Write APDU length
        buff[0..4].copy_from_slice(&(req.len() as u32).to_be_bytes());

        // Write APDU data
        buff[4..].copy_from_slice(req);

        debug!("TX: {:02x?}", buff);

        // Send APDU request
        if let Err(e) = self.s.write_all(&buff).await {
            error!("Failed to write request APDU: {:?}", e);
            return Err(e.into());
        }

        Ok(())
    }

    /// Internal helper to read response data
    async fn read_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut buff = vec![0u8; 4];

        // Read response length (u32 big endian + 2 bytes for status)
        let n = match self.s.read_exact(&mut buff[..4]).await {
            Ok(_) => u32::from_be_bytes(buff[..4].try_into().unwrap()) as usize + 2,
            Err(e) => {
                error!("Failed to read response APDU length: {:?}", e);
                return Err(e.into());
            }
        };

        // Read response data
        buff.resize(n + 4, 0);
        if let Err(e) = self.s.read_exact(&mut buff[4..][..n]).await {
            error!("Failed to read response APDU data: {:?}", e);
            return Err(e.into());
        }

        debug!("RX: {:02x?}", buff);

        // Return response data
        Ok(buff[4..].to_vec())
    }

    pub(crate) async fn is_connected(&self) -> Result<bool, Error> {
        let r = self.s.ready(Interest::WRITABLE).await?;
        Ok(!r.is_read_closed() || !r.is_write_closed())
    }
}

/// [Exchange] implementation for the TCP transport
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Exchange for TcpDevice {
    async fn exchange(&mut self, req: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        // Write APDU request
        self.write_command(req).await?;

        // Await APDU response with timeout
        let d = match tokio::time::timeout(timeout, self.read_data()).await {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(e.into()),
        };

        // Return response data
        Ok(d)
    }
}
