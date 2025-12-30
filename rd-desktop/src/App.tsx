import { useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface ConnectionState {
  connected: boolean;
  serverAddr: string;
  agentId: string;
  status: string;
}

function App() {
  const [connState, setConnState] = useState<ConnectionState>({
    connected: false,
    serverAddr: "127.0.0.1:4433",
    agentId: "",
    status: "Not connected",
  });

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frameRate, setFrameRate] = useState(0);

  const handleConnect = async () => {
    try {
      setConnState({ ...connState, status: "Connecting..." });
      const result = await invoke<string>("connect_agent", {
        serverAddr: connState.serverAddr,
        agentId: connState.agentId,
      });
      setConnState({
        ...connState,
        connected: true,
        status: result,
      });
    } catch (error) {
      setConnState({
        ...connState,
        status: `Failed: ${error}`,
      });
    }
  };

  const handleDisconnect = async () => {
    try {
      await invoke("disconnect");
      setConnState({
        ...connState,
        connected: false,
        status: "Disconnected",
      });
    } catch (error) {
      console.error("Disconnect failed:", error);
    }
  };

  return (
    <div className="app-container">
      <div className="control-panel">
        <h1>🖥️ Remote Desktop</h1>
        
        <div className="connection-form">
          <div className="input-group">
            <label>Server:</label>
            <input
              type="text"
              value={connState.serverAddr}
              onChange={(e) =>
                setConnState({ ...connState, serverAddr: e.target.value })
              }
              disabled={connState.connected}
              placeholder="127.0.0.1:4433"
            />
          </div>
          
          <div className="input-group">
            <label>Agent ID:</label>
            <input
              type="text"
              value={connState.agentId}
              onChange={(e) =>
                setConnState({ ...connState, agentId: e.target.value })
              }
              disabled={connState.connected}
              placeholder="bachtv"
            />
          </div>
          
          <div className="button-group">
            {!connState.connected ? (
              <button onClick={handleConnect} className="btn-connect">
                Connect
              </button>
            ) : (
              <button onClick={handleDisconnect} className="btn-disconnect">
                Disconnect
              </button>
            )}
          </div>
          
          <div className="status-bar">
            <span className={connState.connected ? "status-connected" : "status-disconnected"}>
              {connState.status}
            </span>
            {connState.connected && (
              <span className="fps-counter">FPS: {frameRate}</span>
            )}
          </div>
        </div>
      </div>

      <div className="viewer-container">
        <canvas
          ref={canvasRef}
          className="remote-canvas"
        />
        {!connState.connected && (
          <div className="placeholder">
            Connect to an agent to view remote desktop
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
