//! Ledger Hardware Wallet APDU traits and shared types.
//!
//! This provides abstractions for encoding and decoding APDUs for to
//! support interaction with Ledger devices.
//!
//! APDUs must implement [ApduBase] as well as [encdec::Encode] and [encdec::Decode]
//! (or [encdec::DecodeOwned]) for binary serialisation, with commands providing
//! header information via [ApduReq].
//! [encdec::Encode] and [encdec::Decode] can be automatically derived using `encdec` macros,
//! or manually implemented over existing objects / encodings.
//!
//! An [ApduStatic] helper is provided to automatically implement [ApduReq] for APDU requests
//! with static headers and a common [ApduError] type is provided to unify serialisation and
//! deserialisation errors across APDU objects.
//!
//!
//! ## Examples
//!
//! Command APDU (no body) using [ApduStatic]:
//!
//! ```
//! use ledger_proto::{ApduStatic, ApduError, Encode, DecodeOwned};
//!
//! /// Application information request APDU
//! #[derive(Clone, Debug, PartialEq, Encode, DecodeOwned)]
//! #[encdec(error = "ApduError")]
//! pub struct AppInfoReq {}
//!
//! /// Set CLA and INS values for [AppInfoReq]
//! impl ApduStatic for AppInfoReq {
//!     /// Application Info GET APDU is class `0xb0`
//!     const CLA: u8 = 0xb0;
//!     /// Application Info GET APDU is instruction `0x00`
//!     const INS: u8 = 0x01;
//! }
//! ```
//!
//! Manual response APDU implementation
//!
//! ```
//! use ledger_proto::{ApduStatic, ApduError, Encode, Decode};
//!
//! /// Example response APDU
//! #[derive(Clone, Debug, PartialEq)]
//! pub struct StringResp<'a> {
//!     pub value: &'a str,
//! }
//!
//! /// [Encode] implementation for [StringResp]
//! impl <'a> Encode for StringResp<'a> {
//!   type Error = ApduError;
//!
//!   /// Fetch encoded length
//!   fn encode_len(&self) -> Result<usize, Self::Error> {
//!       Ok(1 + self.value.as_bytes().len())
//!   }
//!
//!   /// Encode to bytes
//!   fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
//!     let b = self.value.as_bytes();
//!
//!     // Check buffer length is valid
//!     if buff.len() < self.encode_len()?
//!         || b.len() > u8::MAX as usize {
//!       return Err(ApduError::InvalidLength);
//!     }
//!
//!     // Write value length
//!     buff[0] = b.len() as u8;
//!
//!     // Write value
//!     buff[1..][..b.len()]
//!         .copy_from_slice(b);
//!
//!     Ok(1 + b.len())
//!   }
//! }
//!
//! impl <'a> Decode<'a> for StringResp<'a> {
//!    type Output = Self;
//!    type Error = ApduError;
//!
//!     fn decode(buff: &'a [u8]) -> Result<(Self::Output, usize), Self::Error> {
//!         // Check buffer length
//!         if buff.len() < 1 {
//!             return Err(ApduError::InvalidLength);
//!         }
//!         let n = buff[0]as usize;
//!         if n + 1 > buff.len() {
//!             return Err(ApduError::InvalidLength);
//!         }
//!
//!         // Parse string value
//!         let s = match core::str::from_utf8(&buff[1..][..n]) {
//!             Ok(v) => v,
//!             Err(_) => return Err(ApduError::InvalidUtf8),
//!         };
//!
//!         // Return object and parsed length
//!         Ok((Self{ value: s}, n + 1))
//!    }
//! }
//! ```
//!
//! For more examples, see the shared APDUs provided in the [apdus] module.
//!

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub use encdec::{Decode, DecodeOwned, EncDec, Encode};

mod error;
pub use error::ApduError;

pub mod apdus;

mod status;
pub use status::StatusCode;

/// APDU command header
#[derive(Copy, Clone, PartialEq, Debug, Default, Encode, DecodeOwned)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[encdec(error = "ApduError")]
pub struct ApduHeader {
    /// Class ID
    pub cla: u8,
    /// Instruction ID
    pub ins: u8,
    /// Parameter 1
    pub p1: u8,
    /// Parameter 2
    pub p2: u8,
}

/// Helper trait for defining static APDU commands, automatically
/// implements [ApduReq].
///
/// ```
/// use ledger_proto::{ApduStatic, ApduError, Encode, Decode};
///
/// // App information request APDU (no body)
/// #[derive(Clone, Debug, PartialEq, Encode, Decode)]
/// #[encdec(error = "ApduError")]
/// pub struct AppInfoReq {}
///
/// /// Set CLA and INS values for [AppInfoReq]
/// impl ApduStatic for AppInfoReq {
///     /// Application Info GET APDU is class `0xb0`
///     const CLA: u8 = 0xb0;
///
///     /// Application Info GET APDU is instruction `0x00`
///     const INS: u8 = 0x01;
/// }
/// ```
pub trait ApduStatic {
    /// Class ID for APDU commands
    const CLA: u8;

    /// Instruction ID for APDU commands
    const INS: u8;

    /// Fetch P1 value (defaults to `0` if not extended)
    fn p1(&self) -> u8 {
        0
    }

    /// Fetch P2 value (defaults to `0` if not extended)
    fn p2(&self) -> u8 {
        0
    }
}

/// Generic APDU request trait
pub trait ApduReq<'a>: EncDec<'a, ApduError> {
    /// Fetch the [ApduHeader] for a given APDU request
    fn header(&self) -> ApduHeader;
}

/// Blanket [ApduReq] impl for [ApduStatic] types
impl<'a, T: EncDec<'a, ApduError> + ApduStatic> ApduReq<'a> for T {
    fn header(&self) -> ApduHeader {
        ApduHeader {
            cla: T::CLA,
            ins: T::INS,
            p1: self.p1(),
            p2: self.p2(),
        }
    }
}

/// Generic APDU base trait, auto-implemented where `T: EncDec<'a, ApduError>`
pub trait ApduBase<'a>: EncDec<'a, ApduError> {}

/// Blanket [ApduBase] implementation
impl<'a, T: EncDec<'a, ApduError>> ApduBase<'a> for T {}

/// Generic APDU object (enabled with `alloc` feature), prefer use of strict APDU types where possible
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg(feature = "alloc")]
pub struct GenericApdu {
    /// Request APDU Header (uses [Default] for incoming / response APDUs)
    pub header: ApduHeader,
    /// APDU data
    #[cfg_attr(feature = "serde", serde(with = "hex::serde"))]
    pub data: Vec<u8>,
}

/// [ApduReq] implementation for [GenericApdu], exposes internal header
#[cfg(feature = "alloc")]
impl ApduReq<'_> for GenericApdu {
    fn header(&self) -> ApduHeader {
        self.header
    }
}

/// [Encode] implementation for [GenericApdu]
#[cfg(feature = "alloc")]
impl Encode for GenericApdu {
    type Error = ApduError;

    fn encode_len(&self) -> Result<usize, Self::Error> {
        Ok(self.data.len())
    }

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        // Check buffer length
        if buff.len() < self.data.len() {
            return Err(ApduError::InvalidLength);
        }
        // Copy data
        buff[..self.data.len()].copy_from_slice(&self.data);
        // Return write length
        Ok(self.data.len())
    }
}

/// [DecodeOwned] implementation for [GenericApdu]
#[cfg(feature = "alloc")]
impl DecodeOwned for GenericApdu {
    type Output = Self;

    type Error = ApduError;

    fn decode_owned(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let data = buff.to_vec();
        Ok((
            Self {
                header: Default::default(),
                data,
            },
            buff.len(),
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use encdec::EncDec;

    /// Helper to test round-trip encode / decode for APDUS
    pub fn encode_decode<'a, A: EncDec<'a, ApduError> + PartialEq>(buff: &'a mut [u8], a: A) {
        // Test encoding
        let n = a.encode(buff).unwrap();

        // Test decoding
        let (a1, n1) = A::decode(&buff[..n]).unwrap();

        // Compare results
        assert_eq!(n1, n);
        assert_eq!(a1, a);
    }

    #[test]
    fn header_encode_decode() {
        let h = ApduHeader {
            cla: 1,
            ins: 2,
            p1: 3,
            p2: 4,
        };

        let mut b = [0u8; 4];

        encode_decode(&mut b, h);

        assert_eq!(&b, &[1, 2, 3, 4]);
    }
}
