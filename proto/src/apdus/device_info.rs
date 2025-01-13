//! Device information request and response APDUs

use encdec::{Decode, Encode};

use crate::{ApduError, ApduStatic};

/// Device info APDU command
#[derive(Copy, Clone, PartialEq, Debug, Default, Encode, Decode)]
#[encdec(error = "ApduError")]
pub struct DeviceInfoReq {}

impl ApduStatic for DeviceInfoReq {
    /// Device info request APDU is class `0xe0`
    const CLA: u8 = 0xe0;

    /// Device info request APDU is instruction `0x01`
    const INS: u8 = 0x01;
}

/// Device info APDU response
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct DeviceInfoResp<'a> {
    /// Target ID
    pub target_id: [u8; 4],

    /// Secure Element Version
    pub se_version: &'a str,

    /// Device Flag(s)
    pub flags: &'a [u8],

    /// MCU Version
    pub mcu_version: &'a str,
}

impl<'a> DeviceInfoResp<'a> {
    /// Create a new device info APDU
    pub fn new(
        target_id: [u8; 4],
        se_version: &'a str,
        mcu_version: &'a str,
        flags: &'a [u8],
    ) -> Self {
        Self {
            target_id,
            se_version,
            mcu_version,
            flags,
        }
    }
}

impl Encode for DeviceInfoResp<'_> {
    type Error = ApduError;

    /// Encode an device info APDU into the provided buffer
    fn encode(&self, buff: &mut [u8]) -> Result<usize, ApduError> {
        // Check buffer length is viable
        if buff.len() < self.encode_len()? {
            return Err(ApduError::InvalidLength);
        }

        let mut index = 0;

        // Write target ID
        buff[index..][..4].copy_from_slice(&self.target_id);
        index += 4;

        // Write SE version
        buff[index] = self.se_version.len() as u8;
        buff[index + 1..][..self.se_version.len()].copy_from_slice(self.se_version.as_bytes());
        index += 1 + self.se_version.len();

        // Write flags
        buff[index] = self.flags.len() as u8;
        buff[index + 1..][..self.flags.len()].copy_from_slice(self.flags);
        index += 1 + self.flags.len();

        // Write MCU version
        buff[index] = self.mcu_version.len() as u8;
        buff[index + 1..][..self.mcu_version.len()].copy_from_slice(self.mcu_version.as_bytes());
        index += 1 + self.mcu_version.len();

        Ok(index)
    }

    /// Compute APDU encoded length
    fn encode_len(&self) -> Result<usize, ApduError> {
        let mut len = 4;

        len += 1 + self.se_version.len();
        len += 1 + self.flags.len();
        len += 1 + self.mcu_version.len();

        Ok(len)
    }
}

impl<'a> Decode<'a> for DeviceInfoResp<'a> {
    type Output = Self;
    type Error = ApduError;

    /// Decode an device info APDU from the provided buffer
    fn decode(buff: &'a [u8]) -> Result<(Self, usize), ApduError> {
        let mut index = 0;

        // Fetch target id
        let mut target_id = [0u8; 4];
        target_id.copy_from_slice(&buff[..4]);
        index += 4;

        // Fetch secure element version
        let se_version_len = buff[index] as usize;
        let se_version = core::str::from_utf8(&buff[index + 1..][..se_version_len])
            .map_err(|_| ApduError::InvalidUtf8)?;
        index += 1 + se_version_len;

        // Fetch flags
        let flags_len = buff[index] as usize;
        let flags = &buff[index + 1..][..flags_len];
        index += 1 + flags_len;

        // Fetch mcu version
        let mcu_version_len = buff[index] as usize;
        let mcu_version = core::str::from_utf8(&buff[index + 1..][..mcu_version_len])
            .map_err(|_| ApduError::InvalidUtf8)?;
        index += 1 + mcu_version_len;

        Ok((
            Self {
                target_id,
                se_version,
                flags,
                mcu_version,
            },
            index,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_info_resp() {
        let r = DeviceInfoResp::new([0x01, 0x02, 0x03, 0x04], "SOME SE", "SOME MCU", &[0xaa]);

        let mut buff = [0u8; 256];
        crate::tests::encode_decode(&mut buff, r);
    }
}
