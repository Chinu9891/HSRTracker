use log::error;
use tauri::State;
use tokio::sync::mpsc::Sender;
use crate::frame_capture::handler::{OrchestratorMessage, session_orchestrator};

mod frame_capture;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_capture(orchestrator_tx: State<'_, Sender<OrchestratorMessage>>) -> Result<(), ()> {
    let _ = orchestrator_tx.send(OrchestratorMessage::BeginCapture).await;
    Ok(())
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

                let k = runtime.block_on(local.run_until(
                    session_orchestrator(app_handle, &mut receiver)
                ));

                if let Err(e) = k {
                    error!("Orchestrator error: {e:?}");
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, start_capture, end_capture])
        .manage(sender)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
