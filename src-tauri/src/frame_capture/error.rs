#[derive(Debug)]
pub enum FrameSessionError {
    InitializationError,
}

impl From<windows::core::Error> for FrameSessionError {
    fn from(_value: windows::core::Error) -> Self {
        FrameSessionError::InitializationError
    }
}

#[derive(Debug)]
pub enum InitializationError {
    HsrNotFound,
    DeviceInitialization,
    TextureInitialization,
    DuplicateRequest,
    Other(windows::core::Error)
}

impl From<windows::core::Error> for InitializationError {
    fn from(value: windows::core::Error) -> Self {
        InitializationError::Other(value)
    }
}