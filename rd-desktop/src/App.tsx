import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  IconScreenShare,
  IconDeviceDesktop,
  IconUsers,
  IconEye,
  IconCopy,
  IconCheck,
  IconMaximize,
  IconMinimize,
  IconShieldLock,
  IconAdjustments,
  IconLock,
  IconKey,
  IconVolume,
  IconVolumeOff,
  IconPaperclip,
  IconDownload,
  IconFileCheck,
  IconClipboardCheck,
  IconPhoneOff,
} from "@tabler/icons-react";

interface ConnectionState {
  connected: boolean;
  serverAddr: string;
  agentId: string;
  status: string;
}

type ConnectionMode = "host" | "viewer";
type StreamQuality = "speed" | "balanced" | "best";

// W3C KeyboardEvent.code to macOS CGKeyCode mapping
const getMacKeyCode = (code: string): number | null => {
  switch (code) {
    case "KeyA": return 0;
    case "KeyS": return 1;
    case "KeyD": return 2;
    case "KeyF": return 3;
    case "KeyH": return 4;
    case "KeyG": return 5;
    case "KeyZ": return 6;
    case "KeyX": return 7;
    case "KeyC": return 8;
    case "KeyV": return 9;
    case "KeyB": return 11;
    case "KeyQ": return 12;
    case "KeyW": return 13;
    case "KeyE": return 14;
    case "KeyR": return 15;
    case "KeyY": return 16;
    case "KeyT": return 17;
    case "Digit1": return 18;
    case "Digit2": return 19;
    case "Digit3": return 20;
    case "Digit4": return 21;
    case "Digit6": return 22;
    case "Digit5": return 23;
    case "Equal": return 24;
    case "Digit9": return 25;
    case "Digit7": return 26;
    case "Minus": return 27;
    case "Digit8": return 28;
    case "Digit0": return 29;
    case "BracketRight": return 30;
    case "KeyO": return 31;
    case "KeyU": return 32;
    case "BracketLeft": return 33;
    case "KeyI": return 34;
    case "KeyP": return 35;
    case "Enter": return 36;
    case "KeyL": return 37;
    case "KeyJ": return 38;
    case "Quote": return 39;
    case "KeyK": return 40;
    case "Semicolon": return 41;
    case "Backslash": return 42;
    case "Comma": return 43;
    case "Slash": return 44;
    case "KeyN": return 45;
    case "KeyM": return 46;
    case "Period": return 47;
    case "Tab": return 48;
    case "Space": return 49;
    case "Backquote": return 50;
    case "Backspace": return 51;
    case "Escape": return 53;
    case "MetaLeft": case "MetaRight": return 55;
    case "ShiftLeft": return 56;
    case "CapsLock": return 57;
    case "AltLeft": return 58;
    case "ControlLeft": return 59;
    case "ShiftRight": return 60;
    case "AltRight": return 61;
    case "ControlRight": return 62;
    case "ArrowLeft": return 123;
    case "ArrowRight": return 124;
    case "ArrowDown": return 125;
    case "ArrowUp": return 126;
    default: return null;
  }
};

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
  const [copied, setCopied] = useState<boolean>(false);
  const [fps, setFps] = useState<number>(0);
  const [isFullscreen, setIsFullscreen] = useState<boolean>(false);
  const [quality, setQuality] = useState<StreamQuality>("balanced");

  // Unattended Access states
  const [hostPassword, setHostPassword] = useState<string>("");
  const [passwordSaved, setPasswordSaved] = useState<boolean>(false);
  const [remotePassword, setRemotePassword] = useState<string>("");

  // Audio state (default muted to prevent local feedback screeching)
  const [isAudioMuted, setIsAudioMuted] = useState<boolean>(true);
  const audioCtxRef = useRef<AudioContext | null>(null);

  // Clipboard sync state
  const [clipboardToast, setClipboardToast] = useState<string | null>(null);

  // File Transfer state
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [transferProgress, setTransferProgress] = useState<{
    transfer_id: string;
    file_name: string;
    percent: number;
    is_sender: boolean;
  } | null>(null);
  const [transferCompleteMsg, setTransferCompleteMsg] = useState<{
    file_name: string;
    saved_path: string;
  } | null>(null);

  // Incoming connection request modal state (AnyDesk style authorization)
  const [incomingRequest, setIncomingRequest] = useState<{ remote_peer_id: string } | null>(null);
  const [allowRemoteInput, setAllowRemoteInput] = useState<boolean>(true);

  // Control mode: "same_pc" (Click-Only & Drag to prevent cursor feedback loop) vs "remote_full" (Full remote)
  const [controlMode, setControlMode] = useState<"same_pc" | "remote_full">("same_pc");
  const isMouseDownRef = useRef<boolean>(false);

  // System permissions state (macOS Screen Recording & Accessibility)
  const [sysPermissions, setSysPermissions] = useState<{ screen_recording: boolean; accessibility: boolean }>({
    screen_recording: true,
    accessibility: true,
  });

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const animationFrameRef = useRef<number>(0);
  const isRenderingRef = useRef(false);
  const frameCountRef = useRef<number>(0);
  const lastFpsTimeRef = useRef<number>(performance.now());

  // Listen to Tauri backend events
  useEffect(() => {
    let unlistenConnected: (() => void) | undefined;
    let unlistenDisconnected: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;
    let unlistenRequest: (() => void) | undefined;
    let unlistenAudio: (() => void) | undefined;
    let unlistenClipboard: (() => void) | undefined;
    let unlistenFileProgress: (() => void) | undefined;
    let unlistenFileComplete: (() => void) | undefined;

    const setupListeners = async () => {
      // Incoming request from viewer wanting to connect
      unlistenRequest = await listen<{ remote_peer_id: string }>("connection_request", (event) => {
        setIncomingRequest({ remote_peer_id: event.payload.remote_peer_id });
      });

      unlistenConnected = await listen<{ remote_peer_id: string }>("peer_connected", (event) => {
        setIncomingRequest(null);
        setConnState((prev) => ({
          ...prev,
          connected: true,
          status: `Connected with ${event.payload.remote_peer_id}`,
        }));
      });

      unlistenDisconnected = await listen("peer_disconnected", () => {
        setIncomingRequest(null);
        setConnState((prev) => ({
          ...prev,
          connected: false,
          status: "Disconnected",
        }));
      });

      unlistenStatus = await listen<string>("host_status", (event) => {
        setConnState((prev) => ({
          ...prev,
          status: event.payload,
        }));
      });

      // Audio stream from remote host
      unlistenAudio = await listen<{ sample_rate: number; channels: number; data: number[] }>(
        "remote_audio_frame",
        (event) => {
          if (isAudioMuted) return;
          try {
            if (!audioCtxRef.current) {
              const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
              audioCtxRef.current = new AudioCtx({ sampleRate: event.payload.sample_rate || 48000 });
            }
            const ctx = audioCtxRef.current;
            if (ctx.state === "suspended") {
              ctx.resume();
            }

            const { channels = 2, sample_rate = 48000, data } = event.payload;
            const u8 = new Uint8Array(data);
            const i16 = new Int16Array(u8.buffer, u8.byteOffset, u8.byteLength / 2);
            const numFrames = Math.floor(i16.length / channels);
            if (numFrames <= 0) return;

            const buffer = ctx.createBuffer(channels, numFrames, sample_rate);
            for (let ch = 0; ch < channels; ch++) {
              const chData = buffer.getChannelData(ch);
              for (let i = 0; i < numFrames; i++) {
                chData[i] = i16[i * channels + ch] / 32768.0;
              }
            }

            const source = ctx.createBufferSource();
            source.buffer = buffer;
            source.connect(ctx.destination);
            source.start();
          } catch (e) {
            console.error("Audio playback error:", e);
          }
        }
      );

      // Clipboard synchronized from remote peer
      unlistenClipboard = await listen<string>("clipboard_synced", (event) => {
        const preview = event.payload.length > 30 ? event.payload.slice(0, 30) + "..." : event.payload;
        setClipboardToast(preview);
        setTimeout(() => setClipboardToast(null), 3000);
      });

      // File transfer progress
      unlistenFileProgress = await listen<{
        transfer_id: string;
        file_name: string;
        percent: number;
        is_sender: boolean;
      }>("file_transfer_progress", (event) => {
        setTransferProgress(event.payload);
        if (event.payload.percent >= 100) {
          setTimeout(() => setTransferProgress(null), 2500);
        }
      });

      // File transfer complete
      unlistenFileComplete = await listen<{
        transfer_id: string;
        file_name: string;
        saved_path: string;
      }>("file_transfer_complete", (event) => {
        setTransferProgress(null);
        setTransferCompleteMsg({
          file_name: event.payload.file_name,
          saved_path: event.payload.saved_path,
        });
        setTimeout(() => setTransferCompleteMsg(null), 6000);
      });
    };

    setupListeners();

    return () => {
      if (unlistenRequest) unlistenRequest();
      if (unlistenConnected) unlistenConnected();
      if (unlistenDisconnected) unlistenDisconnected();
      if (unlistenStatus) unlistenStatus();
      if (unlistenAudio) unlistenAudio();
      if (unlistenClipboard) unlistenClipboard();
      if (unlistenFileProgress) unlistenFileProgress();
      if (unlistenFileComplete) unlistenFileComplete();
    };
  }, [isAudioMuted]);

  // Auto initialize host on startup to get Peer ID immediately
  useEffect(() => {
    let isMounted = true;
    const autoInitHost = async () => {
      try {
        const peerId = await invoke<string>("start_host", {
          signalingUrl: "ws://localhost:3030",
        });
        if (isMounted) {
          setMyPeerId(peerId);
          setConnState((prev) => ({
            ...prev,
            status: `Ready (ID: ${peerId})`,
          }));
        }
      } catch (err) {
        console.warn("Auto init host:", err);
      }
    };
    autoInitHost();
    return () => {
      isMounted = false;
    };
  }, []);

  // Poll system permissions on macOS
  useEffect(() => {
    let isMounted = true;
    const fetchPerms = async () => {
      try {
        const perms = await invoke<{ screen_recording: boolean; accessibility: boolean }>("check_permissions");
        if (isMounted) {
          setSysPermissions(perms);
        }
      } catch (e) {
        console.warn("Failed to check permissions:", e);
      }
    };
    fetchPerms();
    const interval = setInterval(fetchPerms, 3000);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async () => {
      const buffer = reader.result as ArrayBuffer;
      const fileData = Array.from(new Uint8Array(buffer));
      try {
        await invoke("send_file", {
          fileName: file.name,
          fileData,
        });
      } catch (err) {
        console.error("Failed to send file:", err);
      }
    };
    reader.readAsArrayBuffer(file);
    e.target.value = "";
  };

  // Frame rendering loop with hardware-accelerated JPEG base64 decoding
  const renderFrame = useCallback(async () => {
    if (!connState.connected || !canvasRef.current) {
      return;
    }

    try {
      const frame = await invoke<{ width: number; height: number; jpeg_base64: string } | null>("get_frame");

      if (frame && canvasRef.current && frame.jpeg_base64) {
        const canvas = canvasRef.current;
        const ctx = canvas.getContext("2d", { alpha: false });
        if (ctx) {
          const { width, height, jpeg_base64 } = frame;

          // Resize canvas if frame dimensions changed
          if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
          }

          const img = new Image();
          img.onload = () => {
            ctx.drawImage(img, 0, 0);
          };
          img.src = `data:image/jpeg;base64,${jpeg_base64}`;

          // Update FPS counter
          frameCountRef.current += 1;
          const now = performance.now();
          if (now - lastFpsTimeRef.current >= 1000) {
            setFps(Math.round((frameCountRef.current * 1000) / (now - lastFpsTimeRef.current)));
            frameCountRef.current = 0;
            lastFpsTimeRef.current = now;
          }
        }
      }
    } catch (error) {
      console.error("Frame render error:", error);
    }

    // Continue loop
    if (connState.connected) {
      animationFrameRef.current = requestAnimationFrame(renderFrame);
    }
  }, [connState.connected]);

  // Start / stop render loop
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

  // Start hosting (Share Screen)
  const handleStartHost = async () => {
    try {
      setConnState((prev) => ({ ...prev, status: "Starting host..." }));
      const peerId = await invoke<string>("start_host", {
        signalingUrl: connState.serverAddr,
      });
      setMyPeerId(peerId);
      setConnState((prev) => ({
        ...prev,
        status: `Ready (ID: ${peerId})`,
      }));
    } catch (error) {
      setConnState((prev) => ({
        ...prev,
        status: `Failed: ${error}`,
      }));
    }
  };

  // Connect to remote peer (View Screen)
  const handleConnectPeer = async () => {
    if (!remotePeerId || remotePeerId.trim().length < 4) {
      setConnState((prev) => ({ ...prev, status: "Please enter a valid Peer ID" }));
      return;
    }
    try {
      setConnState((prev) => ({ ...prev, status: "Connecting..." }));
      const result = await invoke<string>("connect_peer", {
        signalingUrl: connState.serverAddr,
        remotePeerId: remotePeerId.trim(),
        password: remotePassword.trim() ? remotePassword.trim() : null,
      });
      setConnState((prev) => ({
        ...prev,
        connected: true,
        status: result,
      }));
    } catch (error) {
      setConnState((prev) => ({
        ...prev,
        status: `Connection failed: ${error}`,
      }));
    }
  };

  // Handle incoming request response (Accept / Decline)
  const handleRespondRequest = async (accepted: boolean) => {
    try {
      await invoke("respond_connection_request", {
        accepted,
        allowInput: allowRemoteInput,
      });
      if (!accepted) {
        setIncomingRequest(null);
      }
    } catch (error) {
      console.error("Failed to respond to connection request:", error);
    }
  };

  // Save unattended password on Host
  const handleSaveUnattendedPassword = async () => {
    try {
      const isEnabled = await invoke<boolean>("set_unattended_password", {
        password: hostPassword,
      });
      setPasswordSaved(true);
      setTimeout(() => setPasswordSaved(false), 2500);
      if (!isEnabled) {
        setHostPassword("");
      }
    } catch (error) {
      console.error("Failed to set unattended password:", error);
    }
  };

  // Change stream quality
  const handleQualityChange = async (newQuality: StreamQuality) => {
    setQuality(newQuality);
    const qValue = newQuality === "speed" ? 50 : newQuality === "balanced" ? 70 : 85;
    try {
      await invoke("set_stream_quality", { quality: qValue });
    } catch (error) {
      console.error("Failed to set quality:", error);
    }
  };

  // Disconnect / stop session
  const handleStopConnection = async () => {
    // 1. Immediately reset connection state in UI (0ms delay)
    setConnState((prev) => ({
      ...prev,
      connected: false,
      status: "Disconnected",
    }));
    setFps(0);
    setIncomingRequest(null);

    // 2. Shut down backend WebRTC and notify remote peer
    try {
      await invoke("stop_connection");
    } catch (error) {
      console.error("Disconnect failed:", error);
    }
  };

  // Copy Peer ID to clipboard
  const handleCopyId = () => {
    if (!myPeerId) return;
    navigator.clipboard.writeText(myPeerId);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Calculate coordinates on the Host screen
  const getCanvasCoordinates = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 20, y: 30 };

    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;

    const rawX = Math.round((e.clientX - rect.left) * scaleX);
    const rawY = Math.round((e.clientY - rect.top) * scaleY);

    // Safeguard: Keep coordinates safely away from corners and top menu bar to avoid macOS Hot Corners at (0, 0)
    return {
      x: Math.max(15, Math.min(canvas.width - 15, rawX)),
      y: Math.max(25, Math.min(canvas.height - 15, rawY)),
    };
  };

  // Mouse event handlers (throttled to 40 events/sec to prevent WebRTC congestion)
  const lastMouseMoveTimeRef = useRef<number>(0);
  const handleMouseMove = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;

    // In "same_pc" test mode, do NOT forward hover moves to prevent cursor loops on 1 machine!
    // Only forward mouse moves when user is dragging (mouse button held down)
    if (controlMode === "same_pc" && !isMouseDownRef.current) {
      return;
    }

    const now = performance.now();
    if (now - lastMouseMoveTimeRef.current < 25) {
      return;
    }
    lastMouseMoveTimeRef.current = now;
    const { x, y } = getCanvasCoordinates(e);
    try {
      await invoke("send_input", { eventType: "mouse_move", x, y });
    } catch (error) {
      console.error("Send input error:", error);
    }
  };

  const handleMouseDown = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;
    isMouseDownRef.current = true;
    canvasRef.current?.focus();
    const { x, y } = getCanvasCoordinates(e);
    const eventType = e.button === 2 ? "right_mouse_down" : "mouse_down";
    try {
      await invoke("send_input", { eventType, x, y });
    } catch (error) {
      console.error("Send input error:", error);
    }
  };

  const handleMouseUp = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;
    isMouseDownRef.current = false;
    const { x, y } = getCanvasCoordinates(e);
    const eventType = e.button === 2 ? "right_mouse_up" : "mouse_up";
    try {
      await invoke("send_input", { eventType, x, y });
    } catch (error) {
      console.error("Send input error:", error);
    }
  };

  const handleWheel = async (e: React.WheelEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;
    try {
      await invoke("send_input", {
        eventType: "mouse_scroll",
        x: Math.round(e.deltaX),
        y: Math.round(e.deltaY),
      });
    } catch (error) {
      console.error("Send scroll error:", error);
    }
  };

  // Keyboard event forwarding
  const handleKeyDown = async (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;
    const keyCode = getMacKeyCode(e.code);
    if (keyCode !== null) {
      e.preventDefault();
      try {
        await invoke("send_key", { keyCode, pressed: true });
      } catch (error) {
        console.error("Send keydown error:", error);
      }
    }
  };

  const handleKeyUp = async (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (!connState.connected || mode !== "viewer") return;
    const keyCode = getMacKeyCode(e.code);
    if (keyCode !== null) {
      e.preventDefault();
      try {
        await invoke("send_key", { keyCode, pressed: false });
      } catch (error) {
        console.error("Send keyup error:", error);
      }
    }
  };

  // Fullscreen toggle
  const toggleFullscreen = () => {
    if (!containerRef.current) return;
    if (!document.fullscreenElement) {
      containerRef.current.requestFullscreen().catch((err) => console.error(err));
      setIsFullscreen(true);
    } else {
      document.exitFullscreen().catch((err) => console.error(err));
      setIsFullscreen(false);
    }
  };

  return (
    <div className="flex h-screen w-full bg-background text-foreground font-sans overflow-hidden selection:bg-primary/30 relative">
      {/* AnyDesk Style Incoming Connection Request Modal */}
      {incomingRequest && (
        <div className="absolute inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-card border border-border shadow-2xl rounded-2xl max-w-md w-full p-6 space-y-6 animate-scale-up">
            <div className="flex items-center gap-3">
              <div className="size-12 rounded-xl bg-amber-500/20 text-amber-500 flex items-center justify-center">
                <IconShieldLock className="size-7" />
              </div>
              <div>
                <h3 className="text-lg font-bold text-foreground">Connection Request</h3>
                <p className="text-xs text-muted-foreground">A remote device wants to access your desktop</p>
              </div>
            </div>

            <div className="p-4 rounded-xl bg-muted/40 border border-border text-center">
              <span className="text-xs text-muted-foreground block mb-1 uppercase tracking-wider font-mono">
                Remote Peer ID
              </span>
              <span className="text-3xl font-mono font-bold text-primary tracking-widest">
                {incomingRequest.remote_peer_id}
              </span>
            </div>

            <div className="space-y-3">
              <label className="flex items-center gap-3 p-3 rounded-lg bg-muted/20 border border-border cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={allowRemoteInput}
                  onChange={(e) => setAllowRemoteInput(e.target.checked)}
                  className="size-4 rounded text-primary focus:ring-primary"
                />
                <span className="text-sm font-medium">Allow remote keyboard and mouse control</span>
              </label>
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => handleRespondRequest(false)}
                className="flex-1 py-3 px-4 rounded-xl font-medium text-sm bg-destructive/10 hover:bg-destructive text-destructive hover:text-destructive-foreground transition-all cursor-pointer"
              >
                Decline
              </button>
              <button
                onClick={() => handleRespondRequest(true)}
                className="flex-1 py-3 px-4 rounded-xl font-medium text-sm bg-success hover:bg-success/90 text-white shadow-lg shadow-success/20 transition-all cursor-pointer"
              >
                Accept
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Sidebar Controls */}
      <div className="w-96 flex flex-col border-r border-border glass relative z-20">
        <div className="p-6 border-b border-border">
          <div className="flex items-center gap-3 mb-1">
            <div className="size-10 rounded-xl bg-primary/20 flex items-center justify-center text-primary animate-pulse-subtle">
              <IconDeviceDesktop className="size-6" />
            </div>
            <h1 className="text-xl font-bold bg-linear-to-r from-primary to-accent-foreground bg-clip-text text-transparent">
              Remote Desktop
            </h1>
          </div>
          <p className="text-muted-foreground text-sm ml-1">Ultra-low latency P2P remote control</p>
        </div>

        <div className="p-6 space-y-6 flex-1 overflow-y-auto">
          {/* Mode Toggle Tabs */}
          <div className="flex rounded-xl bg-muted/50 p-1 gap-1">
            <button
              onClick={() => {
                if (!connState.connected) setMode("viewer");
              }}
              disabled={connState.connected}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
                mode === "viewer"
                  ? "bg-primary text-primary-foreground shadow-md"
                  : "text-muted-foreground hover:text-foreground disabled:opacity-50"
              }`}
            >
              <IconEye className="size-4" />
              View Screen
            </button>
            <button
              onClick={() => {
                if (!connState.connected) setMode("host");
              }}
              disabled={connState.connected}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
                mode === "host"
                  ? "bg-primary text-primary-foreground shadow-md"
                  : "text-muted-foreground hover:text-foreground disabled:opacity-50"
              }`}
            >
              <IconUsers className="size-4" />
              Share Screen
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
            <div className={`font-mono text-sm ${connState.connected ? "text-success" : "text-foreground"}`}>
              {connState.status}
            </div>
            {connState.connected && fps > 0 && (
              <div className="mt-2 text-xs font-mono text-muted-foreground flex items-center gap-2">
                <span>FPS: <strong className="text-primary">{fps}</strong></span>
              </div>
            )}
          </div>

          {/* Stream Quality Selector */}
          <div className="space-y-2">
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground ml-1">
              <IconAdjustments className="size-3.5" />
              <span>Streaming Quality</span>
            </div>
            <div className="grid grid-cols-3 gap-1 bg-muted/40 p-1 rounded-xl border border-border text-xs">
              <button
                onClick={() => handleQualityChange("speed")}
                className={`py-1.5 rounded-lg font-medium transition-all ${
                  quality === "speed" ? "bg-background text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground"
                }`}
              >
                Speed
              </button>
              <button
                onClick={() => handleQualityChange("balanced")}
                className={`py-1.5 rounded-lg font-medium transition-all ${
                  quality === "balanced" ? "bg-background text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground"
                }`}
              >
                Balanced
              </button>
              <button
                onClick={() => handleQualityChange("best")}
                className={`py-1.5 rounded-lg font-medium transition-all ${
                  quality === "best" ? "bg-background text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground"
                }`}
              >
                Best
              </button>
            </div>
          </div>

          {/* Mode-specific UI */}
          {mode === "host" ? (
            <div className="space-y-4">
              {/* Accessibility Permission Alert for macOS */}
              {!sysPermissions.accessibility && (
                <div className="p-3.5 bg-amber-500/10 border border-amber-500/30 rounded-xl space-y-2 text-xs">
                  <div className="flex items-center gap-2 font-semibold text-amber-400">
                    <span className="text-base">⚠️</span>
                    <span>Chưa cấp quyền Trợ năng (Accessibility)</span>
                  </div>
                  <p className="text-[11px] text-amber-200/80 leading-relaxed">
                    macOS yêu cầu cấp quyền Trợ năng cho <strong>Antigravity</strong> để bên điều khiển có thể click chuột và gõ phím.
                  </p>
                  <button
                    onClick={() => invoke("open_accessibility_settings")}
                    className="w-full py-1.5 bg-amber-500 hover:bg-amber-400 text-black font-semibold rounded-lg text-xs transition-all cursor-pointer shadow-sm"
                  >
                    Bấm để mở Cài đặt Hệ Thống & Cấp quyền
                  </button>
                </div>
              )}

              <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">This Desk (Your ID)</h3>
              <div className="p-6 rounded-xl bg-gradient-to-br from-primary/20 to-primary/5 border border-primary/20 text-center relative">
                <div className="text-4xl font-bold font-mono tracking-widest text-primary mb-2">
                  {myPeerId || "------"}
                </div>
                <p className="text-xs text-muted-foreground">
                  Give this ID to the person viewing/controlling your screen
                </p>
                {myPeerId && (
                  <button
                    onClick={handleCopyId}
                    className="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-background/80 hover:bg-background border border-border text-foreground transition-all cursor-pointer"
                  >
                    {copied ? <IconCheck className="size-3.5 text-success" /> : <IconCopy className="size-3.5" />}
                    {copied ? "Copied!" : "Copy Address"}
                  </button>
                )}
              </div>

              {/* Unattended Access Configuration */}
              <div className="p-4 rounded-xl border border-border bg-card/60 space-y-3">
                <div className="flex items-center gap-2">
                  <IconLock className="size-4 text-primary" />
                  <span className="text-xs font-semibold">Unattended Access Password</span>
                </div>
                <p className="text-xs text-muted-foreground">
                  Allows connecting directly without needing to click Accept
                </p>
                <div className="flex gap-2">
                  <input
                    type="password"
                    placeholder="Set remote password..."
                    value={hostPassword}
                    onChange={(e) => setHostPassword(e.target.value)}
                    className="flex-1 h-9 rounded-lg border border-input bg-background px-3 text-xs focus:outline-none focus:border-primary"
                  />
                  <button
                    onClick={handleSaveUnattendedPassword}
                    className="px-3 h-9 rounded-lg text-xs font-medium bg-secondary hover:bg-secondary/80 text-secondary-foreground transition-all cursor-pointer"
                  >
                    {passwordSaved ? "Saved!" : "Save"}
                  </button>
                </div>
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
              {/* Always display Your ID so user knows it instantly */}
              <div className="p-4 rounded-xl bg-gradient-to-br from-primary/15 to-primary/5 border border-primary/20 flex items-center justify-between">
                <div>
                  <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider block">
                    This Desk (Your ID)
                  </span>
                  <span className="text-2xl font-bold font-mono text-primary tracking-widest">
                    {myPeerId || "Connecting..."}
                  </span>
                </div>
                {myPeerId && (
                  <button
                    onClick={handleCopyId}
                    className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-xs font-medium bg-background/80 hover:bg-background border border-border text-foreground transition-all cursor-pointer"
                    title="Copy Your ID"
                  >
                    {copied ? <IconCheck className="size-3.5 text-success" /> : <IconCopy className="size-3.5" />}
                    <span>{copied ? "Copied" : "Copy"}</span>
                  </button>
                )}
              </div>

              <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">Remote Desk</h3>

              <div className="space-y-1">
                <label className="text-xs text-muted-foreground ml-1">Enter Remote Address</label>
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
                <label className="flex items-center gap-1.5 text-xs text-muted-foreground ml-1">
                  <IconKey className="size-3" />
                  <span>Password (Optional for Unattended)</span>
                </label>
                <input
                  type="password"
                  placeholder="Remote password if set..."
                  value={remotePassword}
                  onChange={(e) => setRemotePassword(e.target.value)}
                  disabled={connState.connected}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus:border-ring focus:ring-1 focus:ring-ring disabled:opacity-50 transition-all"
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
              {mode === "host" ? "Share My Screen" : "Connect to Remote Desk"}
            </button>
          ) : (
            <button
              onClick={handleStopConnection}
              className="inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 w-full bg-destructive hover:bg-destructive/90 text-destructive-foreground h-11 cursor-pointer"
            >
              Disconnect Session
            </button>
          )}
        </div>
      </div>

      {/* Main Remote Viewport */}
      <div
        ref={containerRef}
        className="flex-1 relative bg-background flex items-center justify-center p-8 overflow-hidden select-none"
      >
        {/* Background Grid Pattern */}
        <div className="absolute inset-0 opacity-[0.03] bg-[linear-gradient(to_right,var(--border)_1px,transparent_1px),linear-gradient(to_bottom,var(--border)_1px,transparent_1px)] bg-size-[24px_24px]"></div>

        <div
          className={`relative transition-all duration-500 max-w-full max-h-full ${
            connState.connected ? "opacity-100 scale-100" : "opacity-80 scale-95 blur-xs"
          }`}
        >
          <div className="relative rounded-lg overflow-hidden shadow-2xl shadow-primary/10 border border-border bg-black aspect-video max-h-[85vh] w-300 flex items-center justify-center">
            <canvas
              ref={canvasRef}
              className="max-w-full max-h-full block cursor-crosshair focus:outline-none"
              tabIndex={0}
              onContextMenu={(e) => e.preventDefault()}
              onMouseMove={handleMouseMove}
              onMouseDown={handleMouseDown}
              onMouseUp={handleMouseUp}
              onMouseLeave={() => { isMouseDownRef.current = false; }}
              onWheel={handleWheel}
              onKeyDown={handleKeyDown}
              onKeyUp={handleKeyUp}
            />
            {!connState.connected && (
              <div className="absolute inset-0 flex items-center justify-center flex-col gap-4 text-muted-foreground animate-float">
                <div className="size-20 rounded-full bg-secondary/50 border border-border flex items-center justify-center text-4xl shadow-inner">
                  <IconScreenShare className="size-10 opacity-50" />
                </div>
                <p className="font-medium tracking-wide">
                  {mode === "host" ? "Click 'Share My Screen' to generate ID" : "Ready to connect"}
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Floating Session Controls */}
        {connState.connected && (
          <div className="absolute top-6 right-6 flex items-center gap-2 animate-slide-in-right z-30">
            {/* FPS Indicator */}
            <div className="px-3 py-1.5 rounded-full glass border border-border flex items-center gap-2 text-xs font-mono text-primary shadow-xl">
              <span className="size-2 bg-success rounded-full animate-pulse"></span>
              {fps > 0 ? `${fps} FPS` : "STREAM ACTIVE"}
            </div>

            {/* Control Mode Toggle (Test on same PC vs Full remote) */}
            <button
              onClick={() => setControlMode((prev) => (prev === "same_pc" ? "remote_full" : "same_pc"))}
              className={`px-3 py-1.5 rounded-full glass border text-xs flex items-center gap-1.5 transition-all cursor-pointer shadow-xl ${
                controlMode === "same_pc"
                  ? "text-primary border-primary/50 bg-primary/10 font-semibold"
                  : "text-muted-foreground border-border hover:text-foreground"
              }`}
              title={
                controlMode === "same_pc"
                  ? "Đang bật: Test cùng máy (Chỉ Click & Kéo - chống giật chuột sang Màn hình 0)"
                  : "Đang bật: Toàn quyền (Khác máy - gửi cả di chuột hover)"
              }
            >
              <span>{controlMode === "same_pc" ? "🖱️ Test cùng máy (Click-Only)" : "🌐 Khác máy (Full)"}</span>
            </button>

            {/* Audio Stream Mute / Unmute */}
            <button
              onClick={() => setIsAudioMuted(!isAudioMuted)}
              className={`p-2 rounded-full glass border border-border transition-all cursor-pointer shadow-xl ${
                !isAudioMuted ? "text-success hover:text-success/80" : "text-muted-foreground hover:text-foreground"
              }`}
              title={isAudioMuted ? "Unmute Audio" : "Mute Audio"}
            >
              {isAudioMuted ? <IconVolumeOff className="size-4" /> : <IconVolume className="size-4" />}
            </button>

            {/* Send File Button */}
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileSelect}
              className="hidden"
            />
            <button
              onClick={() => fileInputRef.current?.click()}
              className="p-2 rounded-full glass border border-border text-foreground hover:text-primary transition-all cursor-pointer shadow-xl"
              title="Send File"
            >
              <IconPaperclip className="size-4" />
            </button>

            {/* Fullscreen Toggle */}
            <button
              onClick={toggleFullscreen}
              className="p-2 rounded-full glass border border-border text-foreground hover:text-primary transition-all cursor-pointer shadow-xl"
              title="Toggle Fullscreen"
            >
              {isFullscreen ? <IconMinimize className="size-4" /> : <IconMaximize className="size-4" />}
            </button>

            {/* Disconnect Session Button */}
            <button
              onClick={handleStopConnection}
              className="p-2 rounded-full glass border border-destructive/40 text-destructive hover:bg-destructive hover:text-white transition-all cursor-pointer shadow-xl ml-1"
              title="Disconnect Session"
            >
              <IconPhoneOff className="size-4" />
            </button>
          </div>
        )}

        {/* Floating Notification Badges & File Transfer Progress */}
        <div className="absolute bottom-6 right-6 flex flex-col gap-2 z-40 max-w-sm pointer-events-none">
          {/* Clipboard Synced Toast */}
          {clipboardToast && (
            <div className="flex items-center gap-2 px-4 py-2.5 rounded-xl glass border border-border text-xs text-foreground shadow-2xl animate-fade-in pointer-events-auto">
              <IconClipboardCheck className="size-4 text-primary shrink-0" />
              <div className="truncate">
                <span className="font-semibold text-primary">Clipboard synced:</span> "{clipboardToast}"
              </div>
            </div>
          )}

          {/* File Transfer Progress Card */}
          {transferProgress && (
            <div className="p-4 rounded-xl glass border border-primary/30 text-xs shadow-2xl space-y-2 pointer-events-auto bg-card/90">
              <div className="flex items-center justify-between font-medium">
                <div className="flex items-center gap-2 truncate">
                  <IconDownload className="size-4 text-primary shrink-0 animate-bounce" />
                  <span className="truncate">{transferProgress.is_sender ? "Sending" : "Receiving"}: {transferProgress.file_name}</span>
                </div>
                <span className="font-mono text-primary font-bold ml-2">{transferProgress.percent}%</span>
              </div>
              <div className="w-full bg-muted/60 h-2 rounded-full overflow-hidden">
                <div
                  className="bg-primary h-full transition-all duration-300 rounded-full"
                  style={{ width: `${transferProgress.percent}%` }}
                />
              </div>
            </div>
          )}

          {/* File Transfer Completed Toast */}
          {transferCompleteMsg && (
            <div className="flex items-start gap-2.5 p-3.5 rounded-xl glass border border-success/30 text-xs shadow-2xl animate-fade-in pointer-events-auto bg-card/90">
              <IconFileCheck className="size-5 text-success shrink-0 mt-0.5" />
              <div className="space-y-0.5">
                <div className="font-semibold text-success">File Transfer Complete!</div>
                <div className="text-muted-foreground truncate">{transferCompleteMsg.file_name}</div>
                <div className="text-[10px] text-muted-foreground/75 font-mono truncate">Saved to: {transferCompleteMsg.saved_path}</div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;
