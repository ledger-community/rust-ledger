use std::collections::HashMap;

use tokio::{
    runtime::Builder,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::LocalSet,
};
use tracing::{debug, error, warn};

use crate::{
    error::Error,
    provider::{LedgerReq, LedgerResp, ReqChannel},
    transport::{GenericDevice, GenericTransport, Transport},
    Exchange,
};

/// Context for provider task
struct ProviderImpl {
    /// Transport for communicating with devices
    t: GenericTransport,
    /// Channel for receiving requests
    req_rx: UnboundedReceiver<(LedgerReq, UnboundedSender<LedgerResp>)>,
    /// Storage for connected devices
    devices: HashMap<usize, GenericDevice>,
    /// Index for device connections
    device_index: usize,
}

/// Static provider context, provides a global singleton for ledger device comms
pub struct ProviderContext {
    /// Channel for sending requests to the provider task
    req_tx: ReqChannel,
}

impl ProviderContext {
    /// Create a new provider context with a thread-pinned task for managing ledger operations
    pub async fn new() -> Self {
        // Setup channel for interacting with the pinned provider task
        let (req_tx, req_rx) = unbounded_channel::<(LedgerReq, UnboundedSender<LedgerResp>)>();

        // Setup runtime with local set just for this task
        // Required for 'ProviderCtx::new' to be callable from withing a `tokio::spawn` context,
        // see: https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html#use-inside-tokiospawn
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create runtime");

        // Spawn a new _real_ thread using this runtime
        std::thread::spawn(move || {
            // Setup local set for this thread
            let local = LocalSet::new();

            // Setup _pinned_ local task for interacting with devices
            // (HidApi and other libraries are not thread safe / okay with changing threads)
            local.spawn_local(async move {
                // Setup ledger provider task
                let mut p = match ProviderImpl::new(req_rx).await {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to initialise ledger task: {:?}", e);
                        return;
                    }
                };

                // Run provide task
                p.run().await;
            });

            rt.block_on(local);
        });

        Self { req_tx }
    }

    /// Fetch request channel for interacting with the provider task
    pub fn req_tx(&self) -> ReqChannel {
        self.req_tx.clone()
    }
}

impl ProviderImpl {
    /// Create provider instance
    pub async fn new(
        req_rx: UnboundedReceiver<(LedgerReq, UnboundedSender<LedgerResp>)>,
    ) -> Result<Self, Error> {
        // Setup transport
        let t = match GenericTransport::new().await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to create transport: {}", e);
                return Err(Error::Unknown);
            }
        };

        Ok(Self {
            t,
            req_rx,
            devices: HashMap::new(),
            device_index: 0,
        })
    }

    /// Run provider task
    pub async fn run(&mut self) {
        debug!("Starting ledger provider task");

        // Poll on incoming requests
        while let Some((req, tx)) = self.req_rx.recv().await {
            debug!("LedgerProvider request: {:02x?}", req);

            if let Some(resp) = self.handle_req(&req).await {
                debug!("LedgerProvider response: {:02x?}", resp);

                if let Err(e) = tx.send(resp) {
                    error!("Failed to forward response: {}", e);
                }
            }
        }

        debug!("Exiting ledger provider task");
    }

    /// Handle incoming requests and generate responses
    async fn handle_req(&mut self, req: &LedgerReq) -> Option<LedgerResp> {
        let resp = match req {
            // List devices using the provided filters
            LedgerReq::List(filters) => match self.t.list(*filters).await {
                Ok(i) => LedgerResp::Devices(i),
                Err(e) => LedgerResp::Error(e),
            },
            // Connect to a specific device
            LedgerReq::Connect(info) => {
                // Check whether we already have a handle for this device
                if let Some((k, d)) = self.devices.iter().find(|(_k, v)| v.info() == info.conn) {
                    let k = *k;
                    debug!("Found existing handle {}: {:?}", k, info);

                    let c = d.is_connected().await;

                    // Check whether handle is still active / available
                    match c {
                        // If the handle is available and in-use, return an error
                        Ok(true) => {
                            warn!("Device {k} already in use");
                            return Some(LedgerResp::Error(Error::DeviceInUse));
                        }
                        // Otherwise, drop the handle and continue connection
                        Ok(false) => {
                            debug!("Handle {k} disconnected");
                            self.devices.remove(&k);
                        }
                        Err(e) => {
                            error!("Failed to fetch disconnected state: {e:?}");
                            self.devices.remove(&k);
                        }
                    }
                }

                // Connect to the device
                let d = match self.t.connect(info.clone()).await {
                    Ok(d) => d,
                    Err(e) => {
                        error!("Failed to connect to device: {}", e);
                        return Some(LedgerResp::Error(e));
                    }
                };

                // Add connected device to internal tracking
                let index = self.device_index;
                self.device_index = self.device_index.wrapping_add(1);

                debug!("Connected device {index}: {}", d.info());

                self.devices.insert(index, d);

                // Return device handle
                LedgerResp::Handle(index)
            }
            LedgerReq::Req(index, apdu, timeout) => {
                // Fetch device handle
                let d = match self.devices.get_mut(index) {
                    Some(d) => d,
                    None => {
                        error!("Attempted to send APDU to unknown device handle: {}", index);
                        return Some(LedgerResp::Error(Error::Unknown));
                    }
                };

                // Issue APDU request to device and return response
                match Exchange::exchange(d, apdu, *timeout).await {
                    Ok(r) => LedgerResp::Resp(r),
                    Err(e) => LedgerResp::Error(e),
                }
            }
            LedgerReq::Close(index) => {
                // Drop device handle
                if let Some(d) = self.devices.remove(index) {
                    debug!("Closed device {index}: {:?}", d.info());
                }

                // no response for close message (channel no longer exists)
                return None;
            }
        };

        Some(resp)
    }
}
