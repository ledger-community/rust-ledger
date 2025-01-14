//! Rust wrapper for executing Speculos via local install or docker image,
//! provided to simplify CI/CD with ledger applications.
//!
//! Drivers are provided for [Docker](DockerDriver) and [Local](LocalDriver)
//! execution, with a [Generic](GenericDriver) abstraction to support
//! runtime driver selection.
//!
//! ### Examples:
//!
//! ``` no_run
//! # use tracing::{debug};
//! use ledger_sim::{GenericDriver, DriverMode, Driver, Model, Options};
//! use ledger_lib::{Device, transport::{Transport, TcpTransport, TcpInfo}, DEFAULT_TIMEOUT};
//! use ledger_proto::apdus::{AppInfoReq, AppInfoResp};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Setup driver for speculos connection
//!     let driver = GenericDriver::new(DriverMode::Docker)?;
//!
//!     // Launch speculos with the provided app
//!     let opts = Options {
//!         model: Model::NanoX,
//!         apdu_port: Some(1237),
//!         ..Default::default()
//!     };
//!     let mut handle = driver.run("ledger-app", opts).await?;
//!
//!     // Setup TCP APDU transport to speculos
//!     let mut transport = TcpTransport::new()?;
//!     let mut device = transport.connect(TcpInfo::default()).await?;
//!
//!     // Fetch app info via transport
//!     let mut buff = [0u8; 256];
//!     let info = device.request::<AppInfoResp>(AppInfoReq{}, &mut buff, DEFAULT_TIMEOUT).await?;
//!
//!     // Await simulator exit or exit signal
//!     tokio::select!(
//!         // Await simulator task completion
//!         _ = driver.wait(&mut handle) => {
//!             debug!("Complete!");
//!         }
//!         // Exit on ctrl + c
//!         _ = tokio::signal::ctrl_c() => {
//!             debug!("Exit!");
//!             driver.exit(handle).await?;
//!         },
//!     );
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;

use clap::Parser;

use strum::{Display, EnumString, VariantNames};

mod drivers;
pub use drivers::*;

mod handle;
pub use handle::*;

/// Device model
#[derive(Copy, Clone, PartialEq, Debug, VariantNames, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Model {
    /// Nano S
    NanoS,
    /// Nano S Plus
    #[strum(serialize = "nanosplus", to_string = "nanosp")]
    NanoSP,
    /// Nano X
    NanoX,
}

impl Model {
    /// Fetch target name for a given ledger model
    pub fn target(&self) -> &'static str {
        match self {
            Model::NanoS => "nanos",
            Model::NanoSP => "nanosplus",
            Model::NanoX => "nanox",
        }
    }
}

/// Simulator display mode
#[derive(Copy, Clone, PartialEq, Debug, VariantNames, Display, EnumString, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Display {
    /// Headless mode
    Headless,
    /// QT based rendering
    Qt,
    /// Text based (command line) rendering
    Text,
}

/// Simulator options
#[derive(Clone, PartialEq, Debug, Parser)]
pub struct Options {
    /// Model to simulate
    #[clap(long, default_value_t = Options::default().model)]
    pub model: Model,

    /// Display mode
    #[clap(long, value_enum, default_value_t = Options::default().display)]
    pub display: Display,

    /// SDK version override (defaults based on --model)
    #[clap(long)]
    pub sdk: Option<String>,

    /// API level override
    #[clap(long)]
    pub api_level: Option<String>,

    /// BIP39 seed for initialisation
    #[clap(long, env)]
    pub seed: Option<String>,

    /// Enable HTTP API port
    #[clap(long, default_value_t = Options::default().http_port)]
    pub http_port: u16,

    /// Enable APDU TCP port (usually 1237)
    #[clap(long, env)]
    pub apdu_port: Option<u16>,

    /// Enable debugging and wait for GDB connection (port 1234)
    #[clap(long)]
    pub debug: bool,

    /// Speculos root (used to configure python paths if set)
    #[clap(long, env = "SPECULOS_ROOT")]
    pub root: Option<String>,

    /// Trace syscalls
    #[clap(long)]
    pub trace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            model: Model::NanoSP,
            display: Display::Headless,
            sdk: None,
            api_level: None,
            seed: None,
            http_port: 5000,
            apdu_port: None,
            debug: false,
            root: None,
            trace: false,
        }
    }
}

impl Options {
    /// Build an argument list from [Options]
    pub fn args(&self) -> Vec<String> {
        // Basic args
        let mut args = vec![
            format!("--model={}", self.model),
            format!("--display={}", self.display),
            format!("--api-port={}", self.http_port),
        ];

        if let Some(seed) = &self.seed {
            args.push(format!("--seed={seed}"));
        }

        if let Some(apdu_port) = &self.apdu_port {
            args.push(format!("--apdu-port={apdu_port}"));
        }

        if let Some(sdk) = &self.sdk {
            args.push(format!("--sdk={sdk}"));
        }

        if let Some(api_level) = &self.api_level {
            args.push(format!("--apiLevel={api_level}"));
        }

        if self.debug {
            args.push("--debug".to_string());
        }

        if self.trace {
            args.push("-t".to_string());
        }

        args
    }

    /// Build environmental variable list from [Options]
    pub fn env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(seed) = &self.seed {
            env.insert("SPECULOS_SEED".to_string(), seed.clone());
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::Model;

    #[test]
    fn model_name_encoding() {
        let t = &[
            (Model::NanoS, "nanos", "nanos"),
            (Model::NanoSP, "nanosp", "nanosp"),
            (Model::NanoSP, "nanosp", "nanosplus"),
            (Model::NanoX, "nanox", "nanox"),
        ];

        for (model, enc, dec) in t {
            assert_eq!(&model.to_string(), enc);
            assert_eq!(Ok(*model), Model::from_str(dec));
        }
    }
}
