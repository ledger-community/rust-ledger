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
    /// App list start request APDU is class `0xe0`
    const CLA: u8 = 0xe0;

    /// App list start request APDU is instruction `0x01`
    const INS: u8 = 0xde;
}

impl ApduStatic for AppListNextReq {
    /// App list next request APDU is class `0xe0`
    const CLA: u8 = 0xe0;

    /// App list next request APDU is instruction `0x01`
    const INS: u8 = 0xdf;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppData {
    pub flags: u32,
    pub hash_code_data: [u8; 32],
    pub hash: [u8; 32],
    pub name: String,
}

/// Decodes an `AppData` structure from a binary slice, starting at the given offset.
///
/// # Binary format
/// The expected format of the data is as follows (all fields are in order):
///
/// - [0]      : 1 byte - Reserved or tag byte (skipped by `*offset += 1`)
/// - [1..5]   : 4 bytes - Flags (big-endian u32)
/// - [5..37]  : 32 bytes - Hash code data
/// - [37..69] : 32 bytes - Hash
/// - [69]     : 1 byte - Length of the name field (N)
/// - [70..70+N]: N bytes - Name (UTF-8 encoded)
///
/// The function updates the provided `offset` as it parses each field.
///
/// # Arguments
/// * `data` - The binary slice containing the encoded `AppData`.
/// * `offset` - A mutable reference to the current offset in the slice. This will be updated as fields are parsed.
///
/// # Returns
/// * `Ok(AppData)` if decoding is successful.
/// * `Err(ApduError)` if decoding fails.
pub fn decode_app_data(data: &[u8], offset: &mut usize) -> Result<AppData, ApduError> {
    *offset += 1;
    let mut app_info: AppData = Default::default();
    let bytes =
        <[u8; 4]>::try_from(&data[*offset..*offset + 4]).map_err(|_| ApduError::InvalidLength)?;
    app_info.flags = u32::from_be_bytes(bytes);
    *offset += 4;
    if data.len() < *offset + 32 {
        return Err(ApduError::InvalidLength);
    }
    app_info
        .hash_code_data
        .copy_from_slice(&data[*offset..*offset + 32]);
    *offset += 32;
    if data.len() < *offset + 32 {
        return Err(ApduError::InvalidLength);
    }
    app_info.hash.copy_from_slice(&data[*offset..*offset + 32]);
    *offset += 32;
    if data.len() <= *offset {
        return Err(ApduError::InvalidLength);
    }
    let name_len: usize = data[*offset] as usize;
    *offset += 1;
    if data.len() < *offset + name_len {
        return Err(ApduError::InvalidLength);
    }
    app_info.name = String::from_utf8(Vec::from(&data[*offset..*offset + name_len])).unwrap();
    *offset += name_len;

    Ok(app_info)
}
