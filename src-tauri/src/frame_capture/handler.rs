use crate::frame_capture::{
    capture::{begin_session},
    error::FrameSessionError
};
use log::{error, info, warn};
use tauri::{AppHandle, Emitter};
use tokio::{
    sync::mpsc::{Receiver},
};

#[derive(Debug)]
pub enum OrchestratorError {
    Fatal,
    DeviceInitialization,
}

#[derive(PartialEq, Eq)]
pub enum OrchestratorMessage {
    BeginCapture,
    StopCapture,
}

pub async fn session_orchestrator(
    app: AppHandle,
    orchestrator_recv: &mut Receiver<OrchestratorMessage>,
) -> Result<(), OrchestratorError> {
    loop {
        tokio::select! {
            msg = orchestrator_recv.recv() => {
                let Some(message) = msg else {
                    warn!("All channels closed; quitting handler");
                    break;
                };

                match message {
                    OrchestratorMessage::BeginCapture => {},
                    OrchestratorMessage::StopCapture => {
                        warn!("Received Stop signal when no capture session is active; skipping");
                        continue;
                    }
                }
            },
            else => {
                break;
            }
        };

        let (stop_tx, mut stop_rx) = tokio::sync::mpsc::channel::<bool>(1);
        let session = begin_session(&mut stop_rx);
        tokio::pin!(session);

        loop {
            tokio::select! {
                result = &mut session => {
                    match result {
                        Ok(result) => {
                            if let Err(e) = app.emit("session_res", result) {
                                error!("Could not emit session result: {e}");
                            }
                        }
                        Err(e) => {
                            let msg = match e {
                                FrameSessionError::HsrNotFound => "adkjfh",
                                _ => "initialization error"
                            };
                            if let Err(e) = app.emit("session_err", msg) {
                                error!("Could not emit session result: {e}");
                            };
                            error!("Capture failed: {e:?}")
                        },
                    }
                    break;
                }

                message = orchestrator_recv.recv() => {
                    match message {
                        Some(OrchestratorMessage::StopCapture) => {
                            let _ = stop_tx.send(true).await;
                        }
                        Some(OrchestratorMessage::BeginCapture) => {
                            warn!("Capture is already running");
                        }
                        None => {
                            let _ = stop_tx.send(true).await;
                            let _ = session.await;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
