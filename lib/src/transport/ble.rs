//! Bluetooth Low Energy (BLE) transport

use std::{fmt::Display, pin::Pin, time::Duration};

use btleplug::{
    api::{
        BDAddr, Central as _, Characteristic, Manager as _, Peripheral, ScanFilter,
        ValueNotification, WriteType,
    },
    platform::Manager,
};
use futures::{stream::StreamExt, Stream};
use tracing::{debug, error, trace, warn};
use uuid::{uuid, Uuid};

use super::{Exchange, Transport};
use crate::{
    info::{ConnInfo, LedgerInfo, Model},
    Error,
};

/// Transport for listing and connecting to BLE connected Ledger devices
pub struct BleTransport {
    manager: Manager,
    peripherals: Vec<(LedgerInfo, btleplug::platform::Peripheral)>,
}

/// BLE specific device information
#[derive(Clone, Debug, PartialEq)]
pub struct BleInfo {
    name: String,
    addr: BDAddr,
}

impl Display for BleInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// BLE connected ledger device
pub struct BleDevice {
    pub info: BleInfo,
    mtu: u8,
    p: btleplug::platform::Peripheral,
    c_write: Characteristic,
    c_read: Characteristic,
}

/// Bluetooth spec for ledger devices
/// see: https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/devices/src/index.ts#L32
#[derive(Clone, PartialEq, Debug)]
struct BleSpec {
    pub model: Model,
    pub service_uuid: Uuid,
    pub notify_uuid: Uuid,
    pub write_uuid: Uuid,
    pub write_cmd_uuid: Uuid,
}

/// Spec for types of bluetooth device
const BLE_SPECS: &[BleSpec] = &[
    BleSpec {
        model: Model::NanoX,
        service_uuid: uuid!("13d63400-2c97-0004-0000-4c6564676572"),
        notify_uuid: uuid!("13d63400-2c97-0004-0001-4c6564676572"),
        write_uuid: uuid!("13d63400-2c97-0004-0002-4c6564676572"),
        write_cmd_uuid: uuid!("13d63400-2c97-0004-0003-4c6564676572"),
    },
    BleSpec {
        model: Model::Stax,
        service_uuid: uuid!("13d63400-2c97-6004-0000-4c6564676572"),
        notify_uuid: uuid!("13d63400-2c97-6004-0001-4c6564676572"),
        write_uuid: uuid!("13d63400-2c97-6004-0002-4c6564676572"),
        write_cmd_uuid: uuid!("13d63400-2c97-6004-0003-4c6564676572"),
    },
];

impl BleTransport {
    pub async fn new() -> Result<Self, Error> {
        // Setup connection manager
        let manager = Manager::new().await?;

        Ok(Self {
            manager,
            peripherals: vec![],
        })
    }

    /// Helper to perform scan for available BLE devices, used in [list] and [connect].
    async fn scan_internal(
        &self,
        duration: Duration,
    ) -> Result<Vec<(LedgerInfo, btleplug::platform::Peripheral)>, Error> {
        let mut matched = vec![];

        // Grab adapter list
        let adapters = self.manager.adapters().await?;

        // TODO: load filters?
        let f = ScanFilter { services: vec![] };

        // Search using adapters
        for adapter in adapters.iter() {
            let info = adapter.adapter_info().await?;
            debug!("Scan with adapter {info}");

            // Start scan with adaptor
            adapter.start_scan(f.clone()).await?;

            tokio::time::sleep(duration).await;

            // Fetch peripheral list
            let mut peripherals = adapter.peripherals().await?;
            if peripherals.is_empty() {
                debug!("No peripherals found on adaptor {info}");
                continue;
            }

            // Load peripheral information
            for p in peripherals.drain(..) {
                // Fetch peripheral properties
                let (properties, _connected) = (p.properties().await?, p.is_connected().await?);

                // Skip peripherals where we couldn't fetch properties
                let properties = match properties {
                    Some(v) => v,
                    None => {
                        debug!("Failed to fetch properties for peripheral: {p:?}");
                        continue;
                    }
                };

                // Skip peripherals without a local name (NanoX should report this)
                let name = match &properties.local_name {
                    Some(v) => v,
                    None => continue,
                };

                debug!("Peripheral: {p:?} props: {properties:?}");

                // Match on peripheral names
                let model = if name.contains("Nano X") {
                    Model::NanoX
                } else if name.contains("Stax") {
                    Model::Stax
                } else {
                    continue;
                };

                // Add to device list
                matched.push((
                    LedgerInfo {
                        model: model.clone(),
                        conn: BleInfo {
                            name: name.clone(),
                            addr: properties.address,
                        }
                        .into(),
                    },
                    p,
                ));
            }
        }

        Ok(matched)
    }
}

/// [Transport] implementation for [BleTransport]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Transport for BleTransport {
    type Filters = ();
    type Info = BleInfo;
    type Device = BleDevice;

    /// List BLE connected ledger devices
    async fn list(&mut self, _filters: Self::Filters) -> Result<Vec<LedgerInfo>, Error> {
        // Scan for available devices
        let devices = self.scan_internal(Duration::from_millis(1000)).await?;

        // Filter to return info list
        let info: Vec<_> = devices.iter().map(|d| d.0.clone()).collect();

        // Save listed devices for next connect
        self.peripherals = devices;

        Ok(info)
    }

    /// Connect to a specific ledger device
    ///
    /// Note: this _must_ follow a [Self::list] operation to match `info` with known peripherals
    async fn connect(&mut self, info: Self::Info) -> Result<Self::Device, Error> {
        // Match known peripherals using provided device info
        let (d, p) = match self
            .peripherals
            .iter()
            .find(|(d, _p)| d.conn == info.clone().into())
        {
            Some(v) => v,
            None => {
                warn!("No device found matching: {info:?}");
                return Err(Error::NoDevices);
            }
        };
        let i = match &d.conn {
            ConnInfo::Ble(i) => i,
            _ => unreachable!(),
        };

        let name = &i.name;

        // Fetch properties
        let properties = p.properties().await?;

        // Connect to device and subscribe to characteristics
        // Fetch specs for matched model (contains characteristic identifiers)
        let specs = match BLE_SPECS.iter().find(|s| s.model == d.model) {
            Some(v) => v,
            None => {
                warn!("No specs for model: {:?}", d.model);
                return Err(Error::Unknown);
            }
        };

        // If we're not connected, attempt to connect
        if !p.is_connected().await? {
            if let Err(e) = p.connect().await {
                warn!("Failed to connect to {name}: {e:?}");
                return Err(Error::Unknown);
            }

            if !p.is_connected().await? {
                warn!("Not connected to {name}");
                return Err(Error::Unknown);
            }
        }

        debug!("peripheral {name}: {p:?} properties: {properties:?}");

        // Then, grab available services and locate characteristics
        p.discover_services().await?;

        let characteristics = p.characteristics();

        trace!("Characteristics: {characteristics:?}");

        let c_write = characteristics.iter().find(|c| c.uuid == specs.write_uuid);
        let c_read = characteristics.iter().find(|c| c.uuid == specs.notify_uuid);

        let (c_write, c_read) = match (c_write, c_read) {
            (Some(w), Some(r)) => (w, r),
            _ => {
                error!("Failed to match read and write characteristics for {name}");
                return Err(Error::Unknown);
            }
        };

        // Create device instance
        let mut d = BleDevice {
            info: info.clone(),
            mtu: 23,
            p: p.clone(),
            c_write: c_write.clone(),
            c_read: c_read.clone(),
        };

        // Request MTU (cmd 0x08, seq: 0x0000, len: 0x0000)
        match d.fetch_mtu().await {
            Ok(mtu) => d.mtu = mtu,
            Err(e) => {
                warn!("Failed to fetch MTU: {:?}", e);
            }
        }

        debug!("using MTU: {}", d.mtu);

        Ok(d)
    }
}

const BLE_HEADER_LEN: usize = 3;

impl BleDevice {
    /// Helper to write commands as chunks based on device MTU
    async fn write_command(&mut self, cmd: u8, payload: &[u8]) -> Result<(), Error> {
        // Setup outgoing data (adds 2-byte big endian length prefix)
        let mut data = Vec::with_capacity(payload.len() + 2);
        data.extend_from_slice(&(payload.len() as u16).to_be_bytes()); // Data length
        data.extend_from_slice(payload); // Data

        debug!("TX cmd: 0x{cmd:02x} payload: {data:02x?}");

        // Write APDU in chunks
        for (i, c) in data.chunks(self.mtu as usize - BLE_HEADER_LEN).enumerate() {
            // Setup chunk buffer
            let mut buff = Vec::with_capacity(self.mtu as usize);
            let cmd = match i == 0 {
                true => cmd,
                false => 0x03,
            };

            buff.push(cmd); // Command
            buff.extend_from_slice(&(i as u16).to_be_bytes()); // Sequence ID
            buff.extend_from_slice(c);

            debug!("Write chunk {i}: {:02x?}", buff);

            self.p
                .write(&self.c_write, &buff, WriteType::WithResponse)
                .await?;
        }

        Ok(())
    }

    /// Helper to read response packet from notification channel
    async fn read_data(
        &mut self,
        mut notifications: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    ) -> Result<Vec<u8>, Error> {
        // Await first response
        let v = match notifications.next().await {
            Some(v) => v.value,
            None => {
                return Err(Error::Unknown);
            }
        };

        debug!("RX: {:02x?}", v);

        // Check response length is reasonable
        if v.len() < 5 {
            error!("response too short");
            return Err(Error::Unknown);
        } else if v[0] != 0x05 {
            error!("unexpected response type: {:?}", v[0]);
            return Err(Error::Unknown);
        }

        // Read out full response length
        let len = v[4] as usize;

        trace!("Expecting response length: {}", len);

        // Setup response buffer
        let mut buff = Vec::with_capacity(len);
        buff.extend_from_slice(&v[5..]);

        // Read further responses
        // TODO: check this is correct with larger packets
        while buff.len() < len {
            // Await response notification
            let v = match notifications.next().await {
                Some(v) => v.value,
                None => {
                    error!("Failed to fetch next chunk from peripheral");
                    self.p.unsubscribe(&self.c_read).await?;
                    return Err(Error::Unknown);
                }
            };

            debug!("RX: {v:02x?}");

            // TODO: check sequence index?

            // add received data to buffer
            buff.extend_from_slice(&v[5..]);
        }

        Ok(buff)
    }

    /// Helper to fetch the available MTU from a bluetooth device
    async fn fetch_mtu(&mut self) -> Result<u8, Error> {
        // Setup read characteristic subscription
        self.p.subscribe(&self.c_read).await?;
        let mut n = self.p.notifications().await?;

        // Write get mtu command
        self.write_command(0x08, &[]).await?;

        // Await MTU response
        let mtu = match n.next().await {
            Some(r) if r.value[0] == 0x08 && r.value.len() == 6 => {
                debug!("RX: {:02x?}", r);
                r.value[5]
            }
            Some(r) => {
                warn!("Unexpected MTU response: {r:02x?}");
                return Err(Error::Unknown);
            }
            None => {
                warn!("Failed to request MTU");
                return Err(Error::Unknown);
            }
        };

        // Unsubscribe from characteristic
        self.p.unsubscribe(&self.c_read).await?;

        Ok(mtu)
    }

    pub(crate) async fn is_connected(&self) -> Result<bool, Error> {
        let c = self.p.is_connected().await?;
        Ok(c)
    }
}

/// [Exchange] impl for BLE backed devices
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl Exchange for BleDevice {
    async fn exchange(&mut self, command: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        // Fetch notification channel for responses
        self.p.subscribe(&self.c_read).await?;
        let notifications = self.p.notifications().await?;

        // Write command data
        if let Err(e) = self.write_command(0x05, command).await {
            self.p.unsubscribe(&self.c_read).await?;
            return Err(e);
        }

        debug!("Await response");

        // Wait for response
        let buff = match tokio::time::timeout(timeout, self.read_data(notifications)).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                self.p.unsubscribe(&self.c_read).await?;
                return Err(e);
            }
            Err(e) => {
                self.p.unsubscribe(&self.c_read).await?;
                return Err(e.into());
            }
        };

        Ok(buff)
    }
}
