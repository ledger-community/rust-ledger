//! Apdu error information for encoding / decoding etc.

/// APDU error type
#[derive(Debug, displaydoc::Display)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum ApduError {
    /// Invalid buffer length
    InvalidLength,

    /// Invalid Utf8 string encoding
    InvalidUtf8,

    /// Invalid APDU encoding version {0}
    InvalidVersion(u8),

    /// Invalid APDU encoding
    InvalidEncoding,
}

impl From<encdec::Error> for ApduError {
    fn from(value: encdec::Error) -> Self {
        match value {
            encdec::Error::Length => Self::InvalidLength,
            encdec::Error::Utf8 => Self::InvalidUtf8,
        }
    }
}
