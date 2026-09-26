import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

interface CaptureResult {
  uptime: number
}
function App() {
  const [isCapturing, setCapturing] = useState(false);
  const [err, setErr] = useState<null | string>(null);
  const [uptime, setUptime] = useState<number | null>(null);

  async function startCapture() {
    setUptime(null);
    setErr(null);
    setCapturing(true);
    await invoke("start_capture");
  }

  async function stopCapture() {
    await invoke("end_capture");
  }

  useEffect(() => {
    let unlisten: Promise<UnlistenFn>;
    (async () => {
      unlisten = listen<CaptureResult>('session_res', (event) => {
        console.log(`Got payload: ${event.payload}`);
        setUptime(event.payload.uptime);
        setCapturing(false);
      });
    })();
    
    return () => { 
      unlisten.then((f) => f());
    };
  }, [])

  useEffect(() => {
    let unlisten: Promise<UnlistenFn>;
    (async () => {
      unlisten = listen<string>('session_err', (event) => {
        setCapturing(false);
        console.log(`Got error, payload: ${event.payload}`);
        setErr(event.payload);
      });
    })();
    
    return () => { 
      unlisten.then((f) => f());
    };
  }, [])

  return (
    <main className="container">
      <h1>Welcome to frame session</h1>

      <button onClick={isCapturing ? stopCapture : startCapture}>{isCapturing ? `Stop capture` : `Start capture`}</button>

      {uptime && (
        <h1>{`Uptime: ${uptime}`}</h1>
      )}

      {err && (
        <h1>{`Error: ${err}`}</h1>
      )}
    </main>
  );
}

export default App;
