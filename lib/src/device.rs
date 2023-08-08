//! High-level Ledger [Device] abstraction for application development

use std::time::Duration;

use encdec::{EncDec, Encode};
use tracing::error;

use ledger_proto::{
    apdus::{AppInfoReq, AppInfoResp, DeviceInfoReq, DeviceInfoResp},
    ApduError, ApduReq, StatusCode,
};

use crate::{
    info::{AppInfo, DeviceInfo},
    Error, Exchange,
};

const APDU_BUFF_LEN: usize = 256;

/// [Device] provides a high-level interface exchanging APDU objects with implementers of [Exchange]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
pub trait Device {
    /// Issue a request APDU, returning a reponse APDU
    async fn request<'a, 'b, RESP: EncDec<'b, ApduError>>(
        &mut self,
        request: impl ApduReq<'a> + Send,
        buff: &'b mut [u8],
        timeout: Duration,
    ) -> Result<RESP, Error>;

    /// Fetch application information
    async fn app_info(&mut self, timeout: Duration) -> Result<AppInfo, Error> {
        let mut buff = [0u8; APDU_BUFF_LEN];

        let r = self
            .request::<AppInfoResp>(AppInfoReq {}, &mut buff[..], timeout)
            .await?;

        Ok(AppInfo {
            name: r.name.to_string(),
            version: r.version.to_string(),
            flags: r.flags,
        })
    }

    /// Fetch device information
    async fn device_info(&mut self, timeout: Duration) -> Result<DeviceInfo, Error> {
        let mut buff = [0u8; APDU_BUFF_LEN];

        let r = self
            .request::<DeviceInfoResp>(DeviceInfoReq {}, &mut buff[..], timeout)
            .await?;

        Ok(DeviceInfo {
            target_id: r.target_id,
            se_version: r.se_version.to_string(),
            mcu_version: r.mcu_version.to_string(),
            flags: r.flags.to_vec(),
        })
    }
}

/// Generic [Device] implementation for types supporting [Exchange]
#[cfg_attr(not(feature = "unstable_async_trait"), async_trait::async_trait)]
impl<T: Exchange + Send> Device for T {
    /// Issue a request APDU to a device, encoding and decoding internally then returning a response APDU
    async fn request<'a, 'b, RESP: EncDec<'b, ApduError>>(
        &mut self,
        req: impl ApduReq<'a> + Send,
        buff: &'b mut [u8],
        timeout: Duration,
    ) -> Result<RESP, Error> {
        // Encode request
        let n = encode_request(req, buff)?;

        // Send request to device
        let resp_bytes = self.exchange(&buff[..n], timeout).await?;

        // Copy response back to buffer prior to decode
        // (these hijinks are required to allow devices to avoid ownership of APDU data)
        let n = resp_bytes.len();
        if n > buff.len() {
            error!(
                "Response length exceeds buffer length ({} > {})",
                n,
                buff.len()
            );
            return Err(ApduError::InvalidLength.into());
        }
        buff[..n].copy_from_slice(&resp_bytes[..]);

        // Handle error responses (2 bytes long, only a status)
        if n == 2 {
            // Return status code if matched, unknown otherwise
            let v = u16::from_be_bytes([resp_bytes[0], resp_bytes[1]]);
            match StatusCode::try_from(v) {
                Ok(c) => return Err(Error::Status(c)),
                Err(_) => return Err(Error::UnknownStatus(resp_bytes[0], resp_bytes[1])),
            }
        }

        // Decode response
        // TODO: is it useful to also return the status bytes?
        let (resp, _) = RESP::decode(&buff[..n])?;

        // Return decode response
        Ok(resp)
    }
}

/// Helper to perform APDU request encoding including the header, length, and body
fn encode_request<'a, REQ: ApduReq<'a>>(req: REQ, buff: &mut [u8]) -> Result<usize, Error> {
    let mut index = 0;

    let data_len = req.encode_len()?;

    // Check buffer length is reasonable
    if buff.len() < 5 + data_len {
        return Err(ApduError::InvalidLength.into());
    }

    // Encode request object

    // First the header
    let h = req.header();
    index += h.encode(&mut buff[index..])?;

    // Then the data length
    if data_len > u8::MAX as usize {
        return Err(ApduError::InvalidLength.into());
    }
    buff[index] = data_len as u8;
    index += 1;

    // Then finally the data
    index += req.encode(&mut buff[index..])?;

    Ok(index)
}

#[cfg(test)]
mod tests {
    use ledger_proto::{apdus::AppInfoReq, ApduStatic};

    use super::encode_request;

    #[test]
    fn test_encode_requests() {
        let mut buff = [0u8; 256];

        let req = AppInfoReq {};
        let n = encode_request(req, &mut buff).unwrap();
        assert_eq!(n, 5);
        assert_eq!(
            &buff[..n],
            &[AppInfoReq::CLA, AppInfoReq::INS, 0x00, 0x00, 0x00]
        );
    }
}
