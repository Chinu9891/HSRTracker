use crate::frame_capture::{
    capture::{begin_capture, initialize_capture},
    error::{FrameSessionError, InitializationError},
};
use log::{error, info, warn};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc::Receiver, oneshot::Sender};

#[derive(Debug)]
pub enum OrchestratorMessage {
    BeginCapture(Sender<Result<(), InitializationError>>),
    StopCapture,
}

pub async fn session_orchestrator(
    app: AppHandle,
    orchestrator_recv: &mut Receiver<OrchestratorMessage>,
) -> Result<(), ()> {
    loop {
        let session = tokio::select! {
            msg = orchestrator_recv.recv() => {
                let Some(message) = msg else {
                    warn!("All channels closed; quitting handler");
                    break;
                };

                match message {
                    OrchestratorMessage::BeginCapture(sender) => {
                        match initialize_capture() {
                            Ok(init) => {
                                // early return fine here since `begin_capture` will emit an error event if it fails to start
                                if sender.send(Ok(())).is_err() {
                                    error!("All channels closed. Exiting");
                                    break;
                                }
                                init
                            },
                            Err(e) => {
                                if sender.send(Err(e)).is_err() {
                                    error!("All channels closed. Exiting");
                                    break;
                                }
                                continue
                            }
                        }
                    },
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
        let session = begin_capture(&mut stop_rx, session);
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
                                FrameSessionError::InitializationError => "initialization error",
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
                            continue;
                        }
                        Some(OrchestratorMessage::BeginCapture(sender)) => {
                            info!("Received begin capture request while one is already active; no-op");
                            if sender.send(Err(InitializationError::DuplicateRequest)).is_err() {
                                error!("All channels closed. Exiting");
                                return Ok(());
                            }
                            continue;
                        },
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
