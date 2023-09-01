use encdec::{Decode, Encode};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{ApduError, ApduStatic};

/// App List Start APDU command
#[derive(Copy, Clone, PartialEq, Debug, Default, Encode, Decode)]
#[encdec(error = "ApduError")]
pub struct AppListStartReq {}

/// App List Next APDU command
#[derive(Copy, Clone, PartialEq, Debug, Default, Encode, Decode)]
#[encdec(error = "ApduError")]
pub struct AppListNextReq {}

impl ApduStatic for AppListStartReq {
    /// Device info request APDU is class `0xe0`
    const CLA: u8 = 0xe0;

    /// Device info request APDU is instruction `0x01`
    const INS: u8 = 0xde;
}

impl ApduStatic for AppListNextReq {
    /// Device info request APDU is class `0xe0`
    const CLA: u8 = 0xe0;

    /// Device info request APDU is instruction `0x01`
    const INS: u8 = 0xdf;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppData {
    pub flags: u32,
    pub hash_code_data: [u8; 32],
    pub hash: [u8; 32],
    pub name: String,
}

pub fn decode_app_data(data: &[u8], offset: &mut usize) -> Result<AppData, ApduError> {
    *offset += 1;
    let mut app_info: AppData = Default::default();
    let bytes = <[u8; 4]>::try_from(&data[*offset..*offset + 4]).unwrap();
    app_info.flags = u32::from_be_bytes(bytes);
    *offset += 4;
    app_info
        .hash_code_data
        .copy_from_slice(&data[*offset..*offset + 32]);
    *offset += 32;
    app_info.hash.copy_from_slice(&data[*offset..*offset + 32]);
    *offset += 32;
    let name_len: usize = data[*offset] as usize;
    *offset += 1;
    app_info.name = String::from_utf8(Vec::from(&data[*offset..*offset + name_len])).unwrap();
    *offset += name_len;

    Ok(app_info)
}
