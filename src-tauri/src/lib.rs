use crate::frame_capture::{
    capture::initialize_capture,
    error::InitializationError,
    handler::{session_orchestrator, OrchestratorMessage},
};
use log::error;
use tauri::State;
use tokio::sync::mpsc::Sender;

mod frame_capture;

#[tauri::command]
async fn start_capture(
    orchestrator_tx: State<'_, Sender<OrchestratorMessage>>,
) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel::<Result<(), InitializationError>>();

    let _ = orchestrator_tx
        .send(OrchestratorMessage::BeginCapture(sender))
        .await;

    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_millis(100000)) => {
            return Err("Unexpected error while capture initialization. Please try again.".to_string());
        },
        initialization_result = receiver => {
            let Ok(result) = initialization_result else {
                return Err("Unexpected error".to_string());
            };

            match result {
                Ok(_) => return Ok(()),
                Err(e) => match e {
                    InitializationError::HsrNotFound => { 
                        return Err("HSR is not open. Please start it before starting capture.".to_string())
                    },
                    _ => {
                        return Err("Unexpected initialization error. Please try again.".to_string())
                    }
                }
            }
        }
    };
}

#[tauri::command]
async fn end_capture(orchestrator_tx: State<'_, Sender<OrchestratorMessage>>) -> Result<(), ()> {
    let _ = orchestrator_tx.send(OrchestratorMessage::StopCapture).await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<OrchestratorMessage>(100);

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();

            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("capture runtime");

                let local = tokio::task::LocalSet::new();

                let k = runtime
                    .block_on(local.run_until(session_orchestrator(app_handle, &mut receiver)));

                if let Err(e) = k {
                    error!("Orchestrator error: {e:?}");
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![start_capture, end_capture])
        .manage(sender)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
