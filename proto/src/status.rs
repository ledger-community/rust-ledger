/// Device status codes (two bytes, trailing response data)
///
/// Replicated from: https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/errors/src/index.ts#L212
#[derive(Copy, Clone, Debug, displaydoc::Display, num_enum::TryFromPrimitive)]
#[repr(u16)]
pub enum StatusCode {
    /// Access condition not fulfilled
    AccessConditionNotFulfilled = 0x9804,
    /// Algorithm not supported
    AlgorithmNotSupported = 0x9484,
    /// APDU class not supported
    ClaNotSupported = 0x6e00,
    /// Code blocked
    CodeBlocked = 0x9840,
    /// Code not initialized
    CodeNotInitialized = 0x9802,
    /// Command incompatible file structure
    CommandIncompatibleFileStructure = 0x6981,
    /// Conditions of use not satisfied
    ConditionsOfUseNotSatisfied = 0x6985,
    /// Contradiction invalidation
    ContradictionInvalidation = 0x9810,
    /// Contradiction secret code status
    ContradictionSecretCodeStatus = 0x9808,
    /// Custom image bootloader
    CustomImageBootloader = 0x662f,
    /// Custom image empty
    CustomImageEmpty = 0x662e,
    /// File already exists
    FileAlreadyExists = 0x6a89,
    /// File not found
    FileNotFound = 0x9404,
    /// GP auth failed
    GpAuthFailed = 0x6300,
    /// Device halted
    Halted = 0x6faa,
    /// Inconsistent file
    InconsistentFile = 0x9408,
    /// Incorrect data
    IncorrectData = 0x6a80,
    /// Incorrect length
    IncorrectLength = 0x6700,
    /// Incorrect P1 or P2 values
    IncorrectP1P2 = 0x6b00,
    /// Instruction not supported
    InsNotSupported = 0x6d00,
    /// Device not onboarded
    DeviceNotOnboarded = 0x6d07,
    /// Device also not onboarded
    DeviceNotOnboarded2 = 0x6611,
    /// Invalid KCV
    InvalidKcv = 0x9485,
    /// Invalid offset
    InvalidOffset = 0x9402,
    /// Licensing error
    Licensing = 0x6f42,
    /// Device locked
    LockedDevice = 0x5515,
    /// Max value reached
    MaxValueReached = 0x9850,
    /// Memory problem
    MemoryProblem = 0x9240,
    /// Missing critical parameter
    MissingCriticalParameter = 0x6800,
    /// No EF selected
    NoEfSelected = 0x9400,
    /// Not enough memory space
    NotEnoughMemorySpace = 0x6a84,
    /// OK
    Ok = 0x9000,
    /// Remaining PIN attempts
    PinRemainingAttempts = 0x63c0,
    /// Referenced data not found
    ReferencedDataNotFound = 0x6a88,
    /// Security status not satisfied
    SecurityStatusNotSatisfied = 0x6982,
    /// Technical problem
    TechnicalProblem = 0x6f00,
    /// Unknown APDU
    UnknownApdu = 0x6d02,
    /// User refused on device
    UserRefusedOnDevice = 0x5501,
    /// Not enough space
    NotEnoughSpace = 0x5102,
}
