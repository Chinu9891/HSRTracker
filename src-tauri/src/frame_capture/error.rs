#[derive(Debug)]
pub enum FrameSessionError {
    InitializationError,
    HsrNotFound,
}

impl From<windows::core::Error> for FrameSessionError {
    fn from(_value: windows::core::Error) -> Self {
        FrameSessionError::InitializationError
    }
}
