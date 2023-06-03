//! [LedgerProvider] provides a tokio-based thread-safe interface for
//! interacting with ledger devices.

use std::time::Duration;

use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedSender},
    OnceCell,
};

mod context;
use context::ProviderContext;

use crate::{error::Error, info::LedgerInfo, transport::Transport, Exchange, Filters};

/// Ledger provider manages device discovery and connection
pub struct LedgerProvider {
    req_tx: ReqChannel,
}

/// Ledger device handle for interacting with [LedgerProvider] backed devices
#[derive(Debug)]
pub struct LedgerHandle {
    pub info: LedgerInfo,

    /// Device index in provider map
    index: usize,

    /// Channel for issuing requests to the provider task
    req_tx: ReqChannel,
}

/// Request object for communication to the provider task
#[derive(Clone, Debug, PartialEq)]
pub enum LedgerReq {
    /// List available devices
    List(Filters),

    /// Connect to a specific device
    Connect(LedgerInfo),

    /// APDU request issued to a device handle
    Req(usize, Vec<u8>, Duration),

    /// Close the device handle
    Close(usize),
}

/// Request object for communication from the provider task
#[derive(Debug)]
pub enum LedgerResp {
    /// List of available ledger devices
    Devices(Vec<LedgerInfo>),

    /// Device handle following connection
    Handle(usize),

    /// APDU response from a device handle
    Resp(Vec<u8>),

    /// Error / operation failure
    Error(Error),
}

/// Helper type alias for [LedgerProvider] requests
pub type ReqChannel = UnboundedSender<(LedgerReq, UnboundedSender<LedgerResp>)>;

/// Global provider context, handle for pinned thread used for device communication
static PROVIDER_CTX: OnceCell<ProviderContext> = OnceCell::const_new();

impl LedgerProvider {
    /// Create or connect to the ledger provider instance
    pub async fn init() -> Self {
        // Fetch or create the provider context
        let ctx = PROVIDER_CTX
            .get_or_init(|| async { ProviderContext::new().await })
            .await;

        // Return handle to request channel
        Self {
            req_tx: ctx.req_tx(),
        }
    }
}

/// [Transport] implementation for high-level [LedgerProvider]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Transport for LedgerProvider {
    type Device = LedgerHandle;
    type Info = LedgerInfo;
    type Filters = Filters;

    /// List available devices using the specified filter
    async fn list(&mut self, filters: Filters) -> Result<Vec<LedgerInfo>, Error> {
        let (tx, mut rx) = unbounded_channel::<LedgerResp>();

        // Send control request
        self.req_tx
            .send((LedgerReq::List(filters), tx))
            .map_err(|_| Error::Unknown)?;

        // Await resposne
        match rx.recv().await {
            Some(LedgerResp::Devices(i)) => Ok(i),
            Some(LedgerResp::Error(e)) => Err(e),
            _ => Err(Error::Unknown),
        }
    }

    /// Connect to an available device
    async fn connect(&mut self, info: LedgerInfo) -> Result<LedgerHandle, Error> {
        let (tx, mut rx) = unbounded_channel::<LedgerResp>();

        // Send control request
        self.req_tx
            .send((LedgerReq::Connect(info.clone()), tx))
            .map_err(|_| Error::Unknown)?;

        // Await resposne
        match rx.recv().await {
            Some(LedgerResp::Handle(index)) => Ok(LedgerHandle {
                info,
                index,
                req_tx: self.req_tx.clone(),
            }),
            Some(LedgerResp::Error(e)) => Err(e),
            _ => Err(Error::Unknown),
        }
    }
}

/// [Exchange] implementation for [LedgerProvider] backed [LedgerHandle]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Exchange for LedgerHandle {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        let (tx, mut rx) = unbounded_channel::<LedgerResp>();

        // Send APDU request
        self.req_tx
            .send((LedgerReq::Req(self.index, command.to_vec(), timeout), tx))
            .map_err(|_| Error::Unknown)?;

        // Await APDU response
        match rx.recv().await {
            Some(LedgerResp::Resp(data)) => Ok(data),
            Some(LedgerResp::Error(e)) => Err(e),
            _ => Err(Error::Unknown),
        }
    }
}

/// [Drop] impl sends close message to provider when [LedgerHandle] is dropped
impl Drop for LedgerHandle {
    fn drop(&mut self) {
        let (tx, _rx) = unbounded_channel::<LedgerResp>();
        let _ = self.req_tx.send((LedgerReq::Close(self.index), tx));
    }
}
