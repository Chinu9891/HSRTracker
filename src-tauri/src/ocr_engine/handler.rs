use windows::{Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap}, Security::Cryptography::CryptographicBuffer};


pub struct RawFrame {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
}

fn create_softwarebitmap(frame: RawFrame) -> windows::core::Result<SoftwareBitmap> {
    let buffer = CryptographicBuffer::CreateFromByteArray(&frame.pixels)?;
    
    SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, frame.width, frame.height)
}

pub async fn ocr_handler() -> Result<(), ()> {

    Ok(())
}