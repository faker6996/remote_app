import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconScreenShare, IconDeviceDesktop, IconUsers, IconEye } from "@tabler/icons-react";

interface ConnectionState {
  connected: boolean;
  serverAddr: string;
  agentId: string;
  status: string;
}

type ConnectionMode = "host" | "viewer";

function App() {
  const [connState, setConnState] = useState<ConnectionState>({
    connected: false,
    serverAddr: "ws://localhost:3030",
    agentId: "",
    status: "Ready",
  });

  const [mode, setMode] = useState<ConnectionMode>("viewer");
  const [myPeerId, setMyPeerId] = useState<string>("");
  const [remotePeerId, setRemotePeerId] = useState<string>("");

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationFrameRef = useRef<number>(0);
  const isRenderingRef = useRef(false);

  // Generate random Peer ID on mount (for host mode)
  useEffect(() => {
    const id = Math.random().toString(36).substring(2, 8).toUpperCase();
    setMyPeerId(id);
  }, []);

  // Frame rendering loop
  const renderFrame = useCallback(async () => {
    if (!connState.connected || !canvasRef.current) {
      return;
    }

    try {
      // Get frame from Tauri backend with dimensions
      const frame = await invoke<{ width: number; height: number; data: number[] } | null>("get_frame");

      if (frame && canvasRef.current) {
        const canvas = canvasRef.current;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const { width, height, data } = frame;
        const pixelCount = width * height;

        // Set canvas size if changed
        if (canvas.width !== width || canvas.height !== height) {
          canvas.width = width;
          canvas.height = height;
        }

        // Convert BGRA to RGBA ImageData
        const imageData = ctx.createImageData(width, height);
        for (let i = 0; i < pixelCount; i++) {
          const srcIdx = i * 4;
          const dstIdx = i * 4;
          // BGRA -> RGBA conversion
          imageData.data[dstIdx + 0] = data[srcIdx + 2]; // R <- B
          imageData.data[dstIdx + 1] = data[srcIdx + 1]; // G
          imageData.data[dstIdx + 2] = data[srcIdx + 0]; // B <- R
          imageData.data[dstIdx + 3] = 255; // A (fully opaque)
        }

        ctx.putImageData(imageData, 0, 0);
      }
    } catch (error) {
      console.error("Frame render error:", error);
    }

    // Continue the render loop
    if (connState.connected) {
      animationFrameRef.current = requestAnimationFrame(renderFrame);
    }
  }, [connState.connected]);

  // Start/stop render loop based on connection state
  useEffect(() => {
    if (connState.connected && !isRenderingRef.current) {
      isRenderingRef.current = true;
      animationFrameRef.current = requestAnimationFrame(renderFrame);
    }

    return () => {
      isRenderingRef.current = false;
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [connState.connected, renderFrame]);

  // P2P: Start hosting (share screen)
  const handleStartHost = async () => {
    try {
      setConnState({ ...connState, status: "Starting host..." });
      const peerId = await invoke<string>("start_host", {
        signalingUrl: connState.serverAddr,
      });
      setMyPeerId(peerId);
      setConnState({
        ...connState,
        connected: true,
        status: `Sharing as ${peerId}`,
      });
    } catch (error) {
      setConnState({
        ...connState,
        status: `Failed: ${error}`,
      });
    }
  };

  // P2P: Connect to remote peer
  const handleConnectPeer = async () => {
    if (!remotePeerId || remotePeerId.length < 4) {
      setConnState({ ...connState, status: "Enter valid Peer ID" });
      return;
    }
    try {
      setConnState({ ...connState, status: "Connecting..." });
      const result = await invoke<string>("connect_peer", {
        signalingUrl: connState.serverAddr,
        remotePeerId: remotePeerId,
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

  // Stop connection (both host and viewer)
  const handleStopConnection = async () => {
    try {
      await invoke("stop_connection");
      setConnState({
        ...connState,
        connected: false,
        status: "Disconnected",
      });
    } catch (error) {
      console.error("Disconnect failed:", error);
    }
  };

  // Mouse event handlers for remote control
  const getCanvasCoordinates = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };

    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;

    return {
      x: Math.round((e.clientX - rect.left) * scaleX),
      y: Math.round((e.clientY - rect.top) * scaleY),
    };
  };

  const handleMouseMove = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected) return;
    const { x, y } = getCanvasCoordinates(e);
    try {
      await invoke("send_input", { eventType: "mouse_move", x, y });
    } catch (error) {
      console.error("Send input error:", error);
    }
  };

  const handleMouseDown = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected) return;
    const { x, y } = getCanvasCoordinates(e);
    try {
      await invoke("send_input", { eventType: "mouse_down", x, y });
    } catch (error) {
      console.error("Send input error:", error);
    }
  };

  const handleMouseUp = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected) return;
    const { x, y } = getCanvasCoordinates(e);
    try {
      await invoke("send_input", { eventType: "mouse_up", x, y });
    } catch (error) {
      console.error("Send input error:", error);
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
          {/* Mode Toggle Tabs */}
          <div className="flex rounded-xl bg-muted/50 p-1 gap-1">
            <button
              onClick={() => setMode("host")}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
                mode === "host" ? "bg-primary text-primary-foreground shadow-md" : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <IconUsers className="size-4" />
              Share Screen
            </button>
            <button
              onClick={() => setMode("viewer")}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
                mode === "viewer" ? "bg-primary text-primary-foreground shadow-md" : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <IconEye className="size-4" />
              View Screen
            </button>
          </div>

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

          {/* Mode-specific UI */}
          {mode === "host" ? (
            <div className="space-y-4">
              <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Your Peer ID</h3>
              <div className="p-6 rounded-xl bg-gradient-to-br from-primary/20 to-primary/5 border border-primary/20 text-center">
                <div className="text-4xl font-bold font-mono tracking-widest text-primary mb-2">{myPeerId}</div>
                <p className="text-xs text-muted-foreground">Share this ID with the person viewing your screen</p>
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground ml-1">Signaling Server</label>
                <input
                  placeholder="ws://localhost:3030"
                  value={connState.serverAddr}
                  onChange={(e) => setConnState({ ...connState, serverAddr: e.target.value })}
                  disabled={connState.connected}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus:border-ring focus:ring-1 focus:ring-ring disabled:opacity-50 transition-all font-mono"
                />
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Connect to Remote</h3>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground ml-1">Peer ID</label>
                <input
                  placeholder="Enter 6-digit code..."
                  value={remotePeerId}
                  onChange={(e) => setRemotePeerId(e.target.value.toUpperCase())}
                  disabled={connState.connected}
                  maxLength={6}
                  className="flex h-14 w-full rounded-xl border border-input bg-background px-4 py-2 text-2xl text-center font-mono tracking-widest placeholder:text-muted-foreground/50 placeholder:text-lg focus-visible:outline-none focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:opacity-50 transition-all uppercase"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground ml-1">Signaling Server</label>
                <input
                  placeholder="ws://localhost:3030"
                  value={connState.serverAddr}
                  onChange={(e) => setConnState({ ...connState, serverAddr: e.target.value })}
                  disabled={connState.connected}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus:border-ring focus:ring-1 focus:ring-ring disabled:opacity-50 transition-all font-mono"
                />
              </div>
            </div>
          )}
        </div>

        <div className="p-6 border-t border-border bg-muted/20">
          {!connState.connected ? (
            <button
              onClick={mode === "host" ? handleStartHost : handleConnectPeer}
              className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 w-full bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg shadow-primary/20 h-11 hover:scale-[1.02] active:scale-[0.98] cursor-pointer"
            >
              {mode === "host" ? "Start Sharing" : "Connect"}
            </button>
          ) : (
            <button
              onClick={handleStopConnection}
              className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 w-full bg-destructive hover:bg-destructive/90 text-destructive-foreground h-11 cursor-pointer"
            >
              Stop
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
            <canvas
              ref={canvasRef}
              className="max-w-full max-h-full block cursor-crosshair"
              tabIndex={0}
              onMouseMove={handleMouseMove}
              onMouseDown={handleMouseDown}
              onMouseUp={handleMouseUp}
            />
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
