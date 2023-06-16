//! Exit application APDU

use encdec::{DecodeOwned, Encode};

use crate::{ApduError, ApduStatic};

/// Exit application request APDU, used to exit a running application
///
/// Note this is not supported by _all_ applications
#[derive(Clone, Debug, PartialEq, Default, Encode, DecodeOwned)]
#[encdec(error = "ApduError")]
pub struct ExitAppReq {}

/// Set CLA and INS values for [ExitAppReq]
impl ApduStatic for ExitAppReq {
    const CLA: u8 = 0xb0;
    const INS: u8 = 0xa7;
}

impl ExitAppReq {
    /// Create a new exit application request
    pub fn new() -> Self {
        Self {}
    }
}
