//! Run application APDU

use encdec::{Decode, Encode};

use crate::{ApduError, ApduStatic};

/// Run application request APDU, request to BOLOS to launch an application on the Ledger Device
#[derive(Clone, Debug, PartialEq, Encode)]
#[encdec(error = "ApduError")]
pub struct RunAppReq<'a> {
    /// Application name to launch (note this is case sensitive)
    pub app_name: &'a str,
}

/// Set CLA and INS values for [RunAppReq]
impl ApduStatic for RunAppReq<'_> {
    const CLA: u8 = 0xe0;
    const INS: u8 = 0xd8;
}

impl<'a> RunAppReq<'a> {
    /// Create a new run application request APDU
    pub fn new(app_name: &'a str) -> Self {
        Self { app_name }
    }
}

impl<'a> Decode<'a> for RunAppReq<'a> {
    type Output = Self;

    type Error = ApduError;

    fn decode(buff: &'a [u8]) -> Result<(Self::Output, usize), Self::Error> {
        let app_name = match core::str::from_utf8(buff) {
            Ok(v) => v,
            Err(_e) => return Err(ApduError::InvalidUtf8),
        };

        Ok((Self { app_name }, buff.len()))
    }
}

#[cfg(test)]
mod test {
    use super::RunAppReq;

    #[test]
    fn encode_decode_run_app_req() {
        let r = RunAppReq::new("test app");

        let mut buff = [0u8; 256];
        crate::tests::encode_decode(&mut buff, r);
    }
}
