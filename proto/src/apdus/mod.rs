//! Ledger common APDU definitions

mod app_info;
pub use app_info::{AppFlags, AppInfoReq, AppInfoResp};

mod device_info;
pub use device_info::{DeviceInfoReq, DeviceInfoResp};
