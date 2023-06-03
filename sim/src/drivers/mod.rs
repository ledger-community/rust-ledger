//! Drivers for speculos runtime execution

use core::fmt::Debug;

use async_trait::async_trait;
use strum::{Display, EnumString, EnumVariantNames};

use crate::Options;

mod local;
pub use local::{LocalDriver, LocalHandle};

mod docker;
pub use docker::{DockerDriver, DockerHandle};

/// Mode selector for generic drivers
#[derive(Copy, Clone, PartialEq, Debug, clap::ValueEnum, EnumString, EnumVariantNames, Display)]
#[strum(serialize_all = "lowercase")]
pub enum DriverMode {
    /// Run Speculos as child process (requires that `speculos.py` is available on the PATH)
    Local,
    /// Run Speculos via docker container
    Docker,
}

/// [`Driver`] trait for speculos providers
#[async_trait]
pub trait Driver {
    type Handle: Debug;

    /// Run speculos with the specified app and options
    async fn run(&self, app: &str, opts: Options) -> anyhow::Result<Self::Handle>;

    /// Wait for task exit / completion
    async fn wait(&self, handle: &mut Self::Handle) -> anyhow::Result<()>;

    /// Exit task
    async fn exit(&self, mut handle: Self::Handle) -> anyhow::Result<()>;
}

/// Generic driver helper, allows implementations to be abstract over
/// concrete driver types
pub enum GenericDriver {
    Local(LocalDriver),
    Docker(DockerDriver),
}

impl GenericDriver {
    /// Create a new [GenericDriver] with the specified [DriverMode]
    pub fn new(mode: DriverMode) -> Result<Self, anyhow::Error> {
        let d = match mode {
            DriverMode::Local => Self::Local(LocalDriver::new()),
            DriverMode::Docker => Self::Docker(DockerDriver::new()?),
        };
        Ok(d)
    }
}

/// Generic Handle helper for use with [GenericDriver]
#[derive(Debug)]
pub enum GenericHandle {
    Local(LocalHandle),
    Docker(DockerHandle),
}

/// [Driver] implementation for [GenericDriver], calls out to [LocalDriver] or
/// [DockerDriver] depending on configuration.
#[async_trait]
impl Driver for GenericDriver {
    type Handle = GenericHandle;

    async fn run(&self, app: &str, opts: Options) -> anyhow::Result<Self::Handle> {
        let h = match self {
            GenericDriver::Local(d) => d.run(app, opts).await.map(GenericHandle::Local)?,
            GenericDriver::Docker(d) => d.run(app, opts).await.map(GenericHandle::Docker)?,
        };

        Ok(h)
    }

    async fn wait(&self, handle: &mut Self::Handle) -> anyhow::Result<()> {
        match (self, handle) {
            (GenericDriver::Local(d), GenericHandle::Local(h)) => d.wait(h).await?,
            (GenericDriver::Docker(d), GenericHandle::Docker(h)) => d.wait(h).await?,
            _ => panic!("driver/handler mismatch"),
        };
        Ok(())
    }

    async fn exit(&self, handle: Self::Handle) -> anyhow::Result<()> {
        match (self, handle) {
            (GenericDriver::Local(d), GenericHandle::Local(h)) => d.exit(h).await?,
            (GenericDriver::Docker(d), GenericHandle::Docker(h)) => d.exit(h).await?,
            _ => panic!("driver/handler mismatch"),
        };
        Ok(())
    }
}
