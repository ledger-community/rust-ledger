//! Ledger CLI, a basic command line interface for interacting with Ledger hardware wallets.
//!
//! See [ledger_lib] for APIs used in this application.

use std::str::FromStr;

use clap::Parser;
use hex::ToHex;
use ledger_proto::{ApduHeader, GenericApdu, StatusCode};
use tracing::{debug, error};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use ledger_lib::{Device, Error, Filters, LedgerHandle, LedgerInfo, LedgerProvider, Transport};

/// Ledger Hardware Wallet Command Line Interface
#[derive(Clone, Debug, PartialEq, Parser)]
pub struct Args {
    #[clap(subcommand)]
    cmd: Command,

    /// Device index where multiple devices are available
    #[clap(long, default_value = "0")]
    index: usize,

    /// Filters for use when connecting to devices
    #[clap(long, default_value = "any")]
    filters: Filters,

    /// Timeout for device requests
    #[clap(long, default_value = "3s")]
    timeout: humantime::Duration,

    /// Enable verbose logging
    #[clap(long, default_value = "debug")]
    log_level: LevelFilter,
}

/// CLI subcommands
#[derive(Clone, Debug, PartialEq, Parser)]
pub enum Command {
    /// List available ledger devices
    List,
    /// Fetch application info
    AppInfo,
    /// Fetch device info
    DeviceInfo,
    /// Exchange a raw APDU with the device
    Apdu {
        /// APDU class
        #[clap(long, value_parser=u8_parse_maybe_hex)]
        cla: u8,

        /// APDU instruction
        #[clap(long, value_parser=u8_parse_maybe_hex)]
        ins: u8,

        /// P1 value
        #[clap(long, value_parser=u8_parse_maybe_hex, default_value_t=0)]
        p1: u8,

        /// P2 value
        #[clap(long, value_parser=u8_parse_maybe_hex, default_value_t=0)]
        p2: u8,

        /// Hex encoded APDU data
        #[clap(default_value = "")]
        data: ApduData,
    },
    /// Exchange raw data with the device
    File {
        #[clap(help = "file to read APDU data from (header + data)")]
        filename: Option<String>,
    },
    /// List applications installed on device
    ListApp,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApduData(Vec<u8>);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppInfo {
    flags: u32,
    hash_code_data: [u8; 32],
    hash: [u8; 32],
    name: String,
}

impl FromStr for ApduData {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = hex::decode(s)?;
        Ok(Self(v))
    }
}

fn u8_parse_maybe_hex(s: &str) -> Result<u8, std::num::ParseIntError> {
    if let Some(s) = s.strip_prefix("0x") {
        u8::from_str_radix(s, 16)
    } else {
        s.parse::<u8>()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load command line arguments
    let args = Args::parse();

    // Setup logging
    let filter = EnvFilter::from_default_env()
        .add_directive("hyper=warn".parse()?)
        .add_directive("rocket=warn".parse()?)
        .add_directive("btleplug=warn".parse()?)
        .add_directive(args.log_level.into());

    let _ = FmtSubscriber::builder()
        .compact()
        .without_time()
        .with_max_level(args.log_level)
        .with_env_filter(filter)
        .try_init();

    debug!("args: {:?}", args);

    // Initialise provider
    let mut p = LedgerProvider::init().await;

    // Fetch list of available devices
    let devices = p.list(args.filters).await?;

    // Handle commands
    match args.cmd {
        Command::List => {
            println!("devices:");
            for (i, d) in devices.iter().enumerate() {
                println!("  {i} {} ({})", d.model, d.conn);
            }
        }
        Command::AppInfo => {
            let mut d = connect(&mut p, &devices, args.index).await?;
            let i = d.app_info(args.timeout.into()).await?;

            println!("app info: {:?}", i);
        }
        Command::DeviceInfo => {
            let mut d = connect(&mut p, &devices, args.index).await?;
            let i = d.device_info(args.timeout.into()).await?;

            println!("device info: {:?}", i);
        }
        Command::Apdu {
            cla,
            ins,
            p1,
            p2,
            data,
        } => {
            let req = GenericApdu {
                header: ApduHeader { cla, ins, p1, p2 },
                data: data.0,
            };

            let mut d = connect(&mut p, &devices, args.index).await?;

            let mut buff = [0u8; 256];
            let resp = d
                .request::<GenericApdu>(req, &mut buff, args.timeout.into())
                .await?;

            println!("Response: {}", resp.data.encode_hex::<String>());
        }
        Command::File { filename } => match filename {
            Some(path) => {
                let data = std::fs::read_to_string(path)?;
                let mut d = connect(&mut p, &devices, args.index).await?;
                let mut buff = [0u8; 256];

                let apdu_seq: Vec<GenericApdu> = serde_json::from_str(data.as_str()).unwrap();

                for apdu_input in apdu_seq {
                    let resp = d
                        .request::<GenericApdu>(apdu_input, &mut buff, args.timeout.into())
                        .await;

                    match resp {
                        Ok(apdu_output) => {
                            println!("Response: {}", apdu_output.data.encode_hex::<String>())
                        }
                        Err(Error::Status(StatusCode::Ok)) => println!("App OK"),
                        Err(e) => println!("Command failed: {e:?}"),
                    }
                }
            }
            None => {
                error!("please provide an input file");
            }
        },
        Command::ListApp => {
            let mut d = connect(&mut p, &devices, args.index).await?;
            let mut app_list: Vec<AppInfo> = vec![];

            let mut flag: bool = true;
            let mut start: bool = true;

            while flag {
                let req = GenericApdu {
                    header: ApduHeader {
                        cla: 0xe0,
                        ins: {
                            match start {
                                true => 0xde,
                                false => 0xdf,
                            }
                        },
                        p1: 0x00,
                        p2: 0x00,
                    },
                    data: vec![],
                };

                start = false;

                let mut buff = [0u8; 256];
                let resp = d
                    .request::<GenericApdu>(req, &mut buff, args.timeout.into())
                    .await;

                match resp {
                    Ok(apdu_output) => {
                        //println!("Response: {}", apdu_output.data.encode_hex::<String>());

                        let mut offset: usize = 1;
                        while offset < apdu_output.data.len() - 2 {
                            offset += 1;
                            let mut app_info: AppInfo = Default::default();
                            let bytes =
                                <[u8; 4]>::try_from(&apdu_output.data[offset..offset + 4]).unwrap();
                            app_info.flags = u32::from_be_bytes(bytes);
                            offset += 4;
                            app_info
                                .hash_code_data
                                .copy_from_slice(&apdu_output.data[offset..offset + 32]);
                            offset += 32;
                            app_info
                                .hash
                                .copy_from_slice(&apdu_output.data[offset..offset + 32]);
                            offset += 32;
                            let name_len: usize = apdu_output.data[offset] as usize;
                            offset += 1;
                            app_info.name = String::from_utf8(Vec::from(
                                &apdu_output.data[offset..offset + name_len],
                            ))
                            .unwrap();
                            offset += name_len;

                            app_list.push(app_info);
                        }
                    }
                    Err(Error::Status(StatusCode::Ok)) => {
                        println!("flags, name, hash, hash_code:");
                        for info in &app_list {
                            println!(
                                "{:08x}, {}, {}, {}",
                                info.flags,
                                info.name,
                                info.hash.encode_hex::<String>(),
                                info.hash_code_data.encode_hex::<String>()
                            );
                        }
                        flag = false;
                    }
                    Err(e) => {
                        println!("Command failed: {e:?}");
                        flag = false;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Connect to a device with the provided index
async fn connect(
    p: &mut LedgerProvider,
    devices: &[LedgerInfo],
    index: usize,
) -> Result<LedgerHandle, Error> {
    // Check we have at least one device
    if devices.is_empty() {
        return Err(Error::NoDevices);
    }

    // Check we have a device matching the index specified
    if index > devices.len() {
        return Err(Error::InvalidDeviceIndex(index));
    }

    let d = &devices[index];
    debug!("Connecting to device: {:?}", d);

    // Connect to the device using the index offset
    match p.connect(d.clone()).await {
        Ok(v) => Ok(v),
        Err(e) => {
            error!("Failed to connect to device {:?}: {:?}", d, e);
            Err(e)
        }
    }
}
