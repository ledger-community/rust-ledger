//! Application information request and response APDUs

use encdec::{Decode, Encode};

use crate::{ApduError, ApduStatic};

/// Application information request APDU
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
#[encdec(error = "ApduError")]
pub struct AppInfoReq {}

/// Set CLA and INS values for [AppInfoReq]
impl ApduStatic for AppInfoReq {
    /// Application Info GET APDU is class `0xb0`
    const CLA: u8 = 0xb0;

    /// Application Info GET APDU is instruction `0x00`
    const INS: u8 = 0x01;
}

/// Application information response APDU
#[derive(Debug, PartialEq)]
pub struct AppInfoResp<'a> {
    /// Application name
    pub name: &'a str,
    /// Application version
    pub version: &'a str,
    /// Application flags
    pub flags: AppFlags,
}

bitflags::bitflags! {
    /// Application info flags
    #[derive(Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct AppFlags: u8 {
        /// Recovery mode
        const RECOVERY = 1 << 0;
        /// Signed application
        const SIGNED = 1 << 1;
        /// User onboarded
        const ONBOARDED = 1 << 2;
        /// ??
        const TRUST_ISSUER = 1 << 3;
        /// ??
        const TRUST_CUSTOM_CA = 1 << 4;
        /// HSM initialised
        const HSM_INITIALISED = 1 << 5;
        /// PIN validated
        const PIN_VALIDATED = 1 << 7;
    }
}

impl<'a> AppInfoResp<'a> {
    /// Create a new application version APDU
    pub fn new(name: &'a str, version: &'a str, flags: AppFlags) -> Self {
        Self {
            name,
            version,
            flags,
        }
    }
}

const APP_VERSION_FMT: u8 = 1;

impl Encode for AppInfoResp<'_> {
    type Error = ApduError;

    fn encode_len(&self) -> Result<usize, Self::Error> {
        let mut len = 0;

        len += 1;
        len += 1 + self.name.len();
        len += 1 + self.version.len();
        len += 2;

        Ok(len)
    }

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        if buff.len() < self.encode_len()? {
            return Err(ApduError::InvalidLength);
        }

        let mut index = 0;
        buff[0] = APP_VERSION_FMT;
        index += 1;

        buff[index] = self.name.len() as u8;
        buff[index + 1..][..self.name.len()].copy_from_slice(self.name.as_bytes());
        index += 1 + self.name.len();

        buff[index] = self.version.len() as u8;
        buff[index + 1..][..self.version.len()].copy_from_slice(self.version.as_bytes());
        index += 1 + self.version.len();

        buff[index] = 1;
        buff[index + 1] = self.flags.bits();
        index += 2;

        Ok(index)
    }
}

impl<'a> Decode<'a> for AppInfoResp<'a> {
    type Output = Self;

    type Error = ApduError;

    fn decode(buff: &'a [u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;

        // Check app version format
        if buff[index] != APP_VERSION_FMT {
            return Err(ApduError::InvalidVersion(buff[index]));
        }
        index += 1;

        // Fetch name string
        let name_len = buff[index] as usize;
        let name = core::str::from_utf8(&buff[index + 1..][..name_len])
            .map_err(|_| ApduError::InvalidUtf8)?;
        index += 1 + name_len;

        // Fetch version string
        let version_len = buff[index] as usize;
        let version = core::str::from_utf8(&buff[index + 1..][..version_len])
            .map_err(|_| ApduError::InvalidUtf8)?;
        index += 1 + version_len;

        // Fetch flags (if available)
        let flags = if buff.len() > index {
            let flags_len = buff[index];
            let flags = AppFlags::from_bits_truncate(buff[index + 1]);
            index += 1 + flags_len as usize;
            flags
        } else {
            AppFlags::empty()
        };

        Ok((
            Self {
                name,
                version,
                flags,
            },
            index,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_resp() {
        let r = AppInfoResp::new("test name", "test version", AppFlags::ONBOARDED);

        let mut buff = [0u8; 256];
        crate::tests::encode_decode(&mut buff, r);
    }
}
