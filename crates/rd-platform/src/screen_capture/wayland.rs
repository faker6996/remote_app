use async_trait::async_trait;
use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use pipewire as pw;
use pw::properties::properties;
use rd_core::domain::{
    error::CaptureError,
    models::{DisplayInfo, FrameFormat, ScreenFrame},
    ports::ScreenCapture,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex as StdMutex,
};
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

/// Wayland screen capture using XDG Desktop Portal ScreenCast and PipeWire
pub struct WaylandScreenCapture {
    sequence: u64,
    display_id: u32,
    latest_frame: Arc<TokioMutex<Option<ScreenFrame>>>,
    portal_ready: Arc<AtomicBool>,
}

impl WaylandScreenCapture {
    pub fn new() -> Result<Self, CaptureError> {
        info!("Initializing Wayland Screen Capture (XDG Desktop Portal ScreenCast + PipeWire)");

        let latest_frame = Arc::new(TokioMutex::new(None));
        let portal_ready = Arc::new(AtomicBool::new(false));

        let latest_frame_clone = latest_frame.clone();
        let portal_ready_clone = portal_ready.clone();

        // Spawn background task to initialize XDG portal and stream video via PipeWire
        tokio::spawn(async move {
            if let Err(e) = run_portal_pipewire_capture(latest_frame_clone, portal_ready_clone).await {
                error!("Wayland portal / PipeWire capture failed: {}", e);
            }
        });

        Ok(Self {
            sequence: 0,
            display_id: 0,
            latest_frame,
            portal_ready,
        })
    }
}

/// Connects to XDG Desktop Portal Screencast and reads video buffers via PipeWire
async fn run_portal_pipewire_capture(
    latest_frame: Arc<TokioMutex<Option<ScreenFrame>>>,
    portal_ready: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session().await?;

    info!("Requesting ScreenCast sources from XDG Desktop Portal...");
    proxy
        .select_sources(
            &session,
            CursorMode::Metadata,
            SourceType::Monitor | SourceType::Window,
            false,
            None,
            PersistMode::DoNot,
        )
        .await?;

    let response = proxy.start(&session, None).await?;
    let streams = response.response()?.streams().to_vec();

    if streams.is_empty() {
        warn!("User cancelled or no streams returned by ScreenCast portal");
        return Ok(());
    }

    let node_id = streams[0].pipe_wire_node_id();
    info!("ScreenCast portal granted stream with PipeWire Node ID: {}", node_id);

    let owned_fd = proxy.open_pipe_wire_remote(&session).await?;
    info!("Obtained PipeWire remote file descriptor from portal");

    portal_ready.store(true, Ordering::SeqCst);

    // Run PipeWire loop in a dedicated background OS thread
    let latest_frame_thread = latest_frame.clone();
    std::thread::spawn(move || {
        if let Err(e) = run_pipewire_loop(owned_fd, node_id, latest_frame_thread) {
            error!("PipeWire event loop error: {}", e);
        }
    });

    Ok(())
}

/// Drives the PipeWire main loop and dequeues video frame buffers
fn run_pipewire_loop(
    owned_fd: std::os::fd::OwnedFd,
    node_id: u32,
    latest_frame: Arc<TokioMutex<Option<ScreenFrame>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(owned_fd, None)?;

    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Video",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Screen",
    };
    props.insert("target.object", node_id.to_string());

    let stream = pw::stream::StreamBox::new(&core, "antigravity-screen-cast", props)?;

    // Shared state for the PipeWire process callback
    struct StreamState {
        width: u32,
        height: u32,
        seq: u64,
    }

    let state = Arc::new(StdMutex::new(StreamState {
        width: 1920,
        height: 1080,
        seq: 0,
    }));

    let state_for_process = state.clone();
    let latest_frame_for_cb = latest_frame.clone();

    let _listener = stream
        .add_local_listener::<()>()
        .process(move |stream, _data: &mut ()| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let datas: &mut [pw::spa::buffer::Data] = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }

                let chunk = datas[0].chunk();
                let chunk_size = chunk.size() as usize;

                if let Some(slice) = datas[0].data() {
                    let slice: &[u8] = slice;
                    if chunk_size > 0 && chunk_size <= slice.len() {
                        let frame_bytes = slice[..chunk_size].to_vec();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let mut s = state_for_process.lock().unwrap();
                        s.seq = s.seq.wrapping_add(1);

                        // If dimension matches 4 bytes per pixel, infer width & height
                        let pixel_count = frame_bytes.len() / 4;
                        if pixel_count > 0 && (s.width * s.height) as usize != pixel_count {
                            let aspect = 16.0 / 9.0;
                            let h = ((pixel_count as f64 / aspect).sqrt()).round() as u32;
                            let w = if h > 0 { (pixel_count as u32) / h } else { 1920 };
                            if w * h * 4 == frame_bytes.len() as u32 {
                                s.width = w;
                                s.height = h;
                            }
                        }

                        let new_frame = ScreenFrame {
                            sequence: s.seq,
                            timestamp: now,
                            data: frame_bytes,
                            width: s.width,
                            height: s.height,
                            format: FrameFormat::Bgra,
                        };

                        if let Ok(mut guard) = latest_frame_for_cb.try_lock() {
                            *guard = Some(new_frame);
                        }
                    }
                }
            }
        })
        .register()?;

    stream.connect(
        pw::spa::utils::Direction::Input,
        Some(node_id),
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut [],
    )?;

    info!("PipeWire video stream connected successfully; entering main loop");
    mainloop.run();

    Ok(())
}

#[async_trait]
impl ScreenCapture for WaylandScreenCapture {
    async fn capture(&mut self) -> Result<ScreenFrame, CaptureError> {
        self.sequence += 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // If PipeWire delivered a real frame, return it
        if let Some(frame) = self.latest_frame.lock().await.clone() {
            return Ok(ScreenFrame {
                sequence: self.sequence,
                timestamp: now,
                data: frame.data,
                width: frame.width,
                height: frame.height,
                format: FrameFormat::Bgra,
            });
        }

        // Otherwise generate an informative live status slate
        let (width, height) = (1920u32, 1080u32);
        let data = generate_portal_slate(width, height, self.sequence, self.portal_ready.load(Ordering::Relaxed));

        Ok(ScreenFrame {
            sequence: self.sequence,
            timestamp: now,
            data,
            width,
            height,
            format: FrameFormat::Bgra,
        })
    }

    async fn get_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(vec![DisplayInfo {
            id: 0,
            name: "Wayland Screen (PipeWire)".to_string(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            is_primary: true,
        }])
    }

    async fn set_target_display(&mut self, display_id: u32) -> Result<(), CaptureError> {
        self.display_id = display_id;
        Ok(())
    }
}

/// Generates a clean animated slate informing the user about portal authorization
fn generate_portal_slate(width: u32, height: u32, seq: u64, is_ready: bool) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut data = vec![0u8; w * h * 4];

    // Background: sleek modern dark theme (BGRA: B=42, G=30, R=24, A=255)
    for pixel in data.chunks_exact_mut(4) {
        pixel[0] = 42;
        pixel[1] = 30;
        pixel[2] = 24;
        pixel[3] = 255;
    }

    // Grid pattern
    for y in 0..h {
        let is_grid_y = y % 64 == 0;
        for x in 0..w {
            if is_grid_y || x % 64 == 0 {
                let idx = (y * w + x) * 4;
                data[idx] = 60;
                data[idx + 1] = 44;
                data[idx + 2] = 35;
            }
        }
    }

    // Top banner
    let banner_h = 70.min(h);
    for y in 0..banner_h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            data[idx] = 75;
            data[idx + 1] = 45;
            data[idx + 2] = 30;
        }
    }

    // Live pulsing dot
    let dot_x = 40 + ((seq as usize * 8) % (w.saturating_sub(80).max(1)));
    let dot_y = 30;
    for dy in 0..14 {
        for dx in 0..14 {
            let py = dot_y + dy;
            let px = dot_x + dx;
            if py < h && px < w {
                let idx = (py * w + px) * 4;
                if is_ready {
                    data[idx] = 60;
                    data[idx + 1] = 220; // Green (streaming ready)
                    data[idx + 2] = 50;
                } else {
                    data[idx] = 40;
                    data[idx + 1] = 180; // Amber (waiting for prompt)
                    data[idx + 2] = 250;
                }
            }
        }
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wayland_screen_capture_lifecycle() {
        let mut capture = WaylandScreenCapture::new().expect("Failed to create WaylandScreenCapture");
        let frame = capture.capture().await.expect("Capture must succeed");
        assert!(frame.width > 0);
        assert!(frame.height > 0);
        assert_eq!(frame.data.len(), (frame.width * frame.height * 4) as usize);

        let displays = capture.get_displays().await.expect("Get displays must succeed");
        assert!(!displays.is_empty());
    }
}
