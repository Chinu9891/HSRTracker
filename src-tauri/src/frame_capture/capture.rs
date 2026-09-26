use image::{ImageBuffer, RgbaImage};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use std::sync::Mutex;
use std::sync::atomic::AtomicI32;
use tokio::sync::mpsc::{Receiver};
use windows::core::{Interface, Ref};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::{Direct3D::*, Direct3D11::*};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DispatcherQueueOptions, RoInitialize, DQTAT_COM_NONE,
    DQTYPE_THREAD_CURRENT, RO_INIT_MULTITHREADED,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, FindWindowW, GetMessageW, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage, TranslateMessage,
};

use crate::frame_capture::error::FrameSessionError;

const ROI_X: u32 = 2560;
const ROI_Y: u32 = 0;
const ROI_HEIGHT: u32 = 520;
const ROI_WIDTH: u32 = 520;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionResult {
    pub uptime: u64,
}

pub enum FrameHandlerResult {
    WindowClosed,
    ProcessingFinished
}

pub async fn begin_session(
    session_rx: &mut Receiver<bool>,
) -> Result<SessionResult, FrameSessionError> {
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|e| {
        error!("Error while initializing frame session: {e}");
        FrameSessionError::InitializationError
    })?;
    unsafe {
        CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        })
        .map_err(|e| {
            error!("Error while initializing frame session: {e}");
            FrameSessionError::InitializationError
        })?;
    };

    let (device, device_context) = unsafe {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| {
            error!("Error while initializing frame session: {e}");
            FrameSessionError::InitializationError
        })?;
        (
            device.ok_or(FrameSessionError::InitializationError)?,
            context.ok_or(FrameSessionError::InitializationError)?,
        )
    };

    let hwnd = unsafe {
        FindWindowW(
            windows::core::w!("UnityWndClass"),
            windows::core::w!("Honkai: Star Rail"),
        )
        .map_err(|e| {
            error!("Error while trying to find Honkai star rail's game window: {e}");
            FrameSessionError::HsrNotFound
        })?
    };

    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;

    let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd)? };

    let direct3d_device = unsafe {
        CreateDirect3D11DeviceFromDXGIDevice(&device.cast::<IDXGIDevice>()?)?
            .cast::<IDirect3DDevice>()?
    };

    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &direct3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        item.Size()?,
    )?;

    let session = pool.CreateCaptureSession(&item)?;
    let roi_texture = {
        let mut texture = None;

        let roi_desc = D3D11_TEXTURE2D_DESC {
            Width: ROI_WIDTH,
            Height: ROI_HEIGHT,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: 0,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };

        unsafe {
            device
                .CreateTexture2D(&roi_desc, None, Some(&mut texture))?;
        }

        texture.ok_or(FrameSessionError::InitializationError)?
    };

    let staging_texture = {
        let mut texture = None;

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: ROI_WIDTH,
            Height: ROI_HEIGHT,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        unsafe {
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut texture))?;
        }

        texture.ok_or(FrameSessionError::InitializationError)?
    };

    session.SetIsBorderRequired(false)?;
    let (closed_tx, mut closed_rx) = tokio::sync::mpsc::unbounded_channel::<FrameHandlerResult>();
    let closed_tx_clone = closed_tx.clone();

    item.Closed(&TypedEventHandler::new(
        move |_event: Ref<'_, GraphicsCaptureItem>, _| {
            let _ = closed_tx_clone.send(FrameHandlerResult::WindowClosed);
            Ok(())
        },
    ))?;
    
    let i = AtomicI32::new(1);
    pool.FrameArrived(&TypedEventHandler::new(
        move |sender: Ref<'_, Direct3D11CaptureFramePool>, _| {
            let sender = sender.unwrap();
            let frame = sender.TryGetNextFrame()?;
            let surface = frame.Surface()?;

            let interface = surface.cast::<IDirect3DDxgiInterfaceAccess>()?;
            let texture = unsafe { interface.GetInterface::<ID3D11Texture2D>()? };
            let mut desc: D3D11_TEXTURE2D_DESC = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut desc) };

            let src_box = D3D11_BOX {
                left: ROI_X,
                top: ROI_Y,
                front: 0,
                right: ROI_X + ROI_WIDTH,
                bottom: ROI_Y + ROI_HEIGHT,
                back: 1,
            };

            unsafe {
                device_context.CopySubresourceRegion(
                    &roi_texture,
                    0,
                    0,
                    0,
                    0,
                    &texture,
                    0,
                    Some(&src_box),
                );
            }

            unsafe {
                device_context.CopyResource(&staging_texture, &roi_texture);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                device_context.Map(
                    &staging_texture,
                    0,
                    D3D11_MAP_READ,
                    0,
                    Some(&mut mapped),
                )?;
            }

            let mut pixels: Vec<u8> =
                Vec::with_capacity(ROI_WIDTH as usize * ROI_HEIGHT as usize * 4);
            let row_bytes = ROI_WIDTH as usize * 4;
            for row in 0..ROI_HEIGHT as usize {
                unsafe {
                    let src = (mapped.pData as *const u8).add(row * mapped.RowPitch as usize);

                    pixels.extend_from_slice(std::slice::from_raw_parts(src, row_bytes));
                };
            }

            unsafe {
                device_context.Unmap(&staging_texture, 0);
            }
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }

            if i.load(std::sync::atomic::Ordering::Relaxed) % 10 == 0 {
                let image: RgbaImage = ImageBuffer::from_raw(ROI_WIDTH, ROI_HEIGHT, pixels)
                    .expect("invalid pixel buffer");
                image
                    .save(format!(
                        "C:\\Users\\Chinu\\Downloads\\testImg\\test{}.png",
                        i.load(std::sync::atomic::Ordering::Relaxed)
                    ))
                    .unwrap();
            }
            i.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        },
    ))?;
    let now = Instant::now();
    session.StartCapture()?;

    let mut pump_tick =
        tokio::time::interval(std::time::Duration::from_millis(10));

    loop {
        tokio::select! {
            _ = pump_tick.tick() => {
                let mut msg = MSG::default();
                while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }
                    .as_bool()
                {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
            _ = session_rx.recv() => break,
            _ = closed_rx.recv() => break,
        }
    }

    session.Close()?;
    let uptime = Instant::now() - now;

    Ok(SessionResult {
        uptime: uptime.as_secs(),
    })
}
