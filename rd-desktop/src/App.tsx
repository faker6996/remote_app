import { useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconScreenShare, IconDeviceDesktop } from "@tabler/icons-react";

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
    status: "Ready to connect",
  });

  const canvasRef = useRef<HTMLCanvasElement>(null);

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
    <div className="flex h-screen w-full bg-background text-foreground font-sans overflow-hidden selection:bg-primary/30">
      {/* Sidebar Controls */}
      <div className="w-96 flex flex-col border-r border-border glass relative z-20">
        <div className="p-6 border-b border-border">
          <div className="flex items-center gap-3 mb-1">
            <div className="size-10 rounded-xl bg-primary/20 flex items-center justify-center text-primary animate-pulse-subtle">
              <IconDeviceDesktop className="size-6" />
            </div>
            <h1 className="text-xl font-bold bg-linear-to-r from-primary to-accent-foreground bg-clip-text text-transparent">Remote Desktop</h1>
          </div>
          <p className="text-muted-foreground text-sm ml-1">High performance control</p>
        </div>

        <div className="p-6 space-y-6 flex-1 overflow-y-auto">
          {/* Status Card */}
          <div
            className={`p-4 rounded-xl border ${
              connState.connected ? "bg-success/10 border-success/20" : "bg-card border-border"
            } transition-all duration-300`}
          >
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">Status</span>
              {connState.connected && <span className="flex size-2 rounded-full bg-success animate-pulse" />}
            </div>
            <div className={`font-mono text-sm ${connState.connected ? "text-success" : "text-foreground"}`}>{connState.status}</div>
          </div>

          <div className="space-y-4">
            <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Connection Details</h3>

            <div className="space-y-1">
              <label className="text-xs text-muted-foreground ml-1">Server Address</label>
              <input
                placeholder="127.0.0.1:4433"
                value={connState.serverAddr}
                onChange={(e) => setConnState({ ...connState, serverAddr: e.target.value })}
                disabled={connState.connected}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus:border-ring focus:ring-1 focus:ring-ring focus:ring-opacity-20 disabled:cursor-not-allowed disabled:opacity-50 transition-all font-mono"
              />
            </div>

            <div className="space-y-1">
              <label className="text-xs text-muted-foreground ml-1">Agent ID</label>
              <input
                placeholder="Device Name..."
                value={connState.agentId}
                onChange={(e) => setConnState({ ...connState, agentId: e.target.value })}
                disabled={connState.connected}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus:border-ring focus:ring-1 focus:ring-ring focus:ring-opacity-20 disabled:cursor-not-allowed disabled:opacity-50 transition-all font-mono"
              />
            </div>
          </div>
        </div>

        <div className="p-6 border-t border-border bg-muted/20">
          {!connState.connected ? (
            <button
              onClick={handleConnect}
              className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 w-full bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg shadow-primary/20 h-11 hover:scale-[1.02] active:scale-[0.98] cursor-pointer"
            >
              Connect to Agent
            </button>
          ) : (
            <button
              onClick={handleDisconnect}
              className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 w-full bg-destructive hover:bg-destructive/90 text-destructive-foreground h-11 cursor-pointer"
            >
              Disconnect
            </button>
          )}
        </div>
      </div>

      {/* Main View Port */}
      <div className="flex-1 relative bg-background flex items-center justify-center p-8 overflow-hidden">
        {/* Background Pattern */}
        <div className="absolute inset-0 opacity-[0.03] bg-[linear-gradient(to_right,var(--border)_1px,transparent_1px),linear-gradient(to_bottom,var(--border)_1px,transparent_1px)] bg-size-[24px_24px]"></div>

        {/* Scan Line Animation */}
        {!connState.connected && (
          <div className="absolute inset-0 pointer-events-none bg-linear-to-b from-transparent via-primary/5 to-transparent h-[10%] w-full animate-scan opacity-50"></div>
        )}

        <div className={`relative transition-all duration-700 ${connState.connected ? "opacity-100 scale-100" : "opacity-80 scale-95 blur-sm"}`}>
          <div className="relative rounded-lg overflow-hidden shadow-2xl shadow-primary/10 border border-border bg-card aspect-video max-h-[80vh] w-300 flex items-center justify-center glass-card">
            <canvas ref={canvasRef} className="max-w-full max-h-full block" />
            {!connState.connected && (
              <div className="absolute inset-0 flex items-center justify-center flex-col gap-4 text-muted-foreground animate-float">
                <div className="size-20 rounded-full bg-secondary/50 border border-border flex items-center justify-center text-4xl shadow-inner">
                  <IconScreenShare className="size-10 opacity-50" />
                </div>
                <p className="font-medium tracking-wide">Waiting for connection...</p>
              </div>
            )}
          </div>
        </div>

        {/* Floating Overlay Controls when Connected */}
        {connState.connected && (
          <div className="absolute top-6 right-6 flex gap-2 animate-slide-in-right">
            <div className="px-3 py-1.5 rounded-full glass border border-border flex items-center gap-2 text-xs font-mono text-primary shadow-xl">
              <span className="size-2 bg-success rounded-full animate-pulse"></span>
              LIVE STREAM
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
