use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Cross-platform audio capture service handle (Send + Sync)
pub struct AudioCaptureService {
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl AudioCaptureService {
    /// Start capturing audio on a dedicated OS thread
    pub fn start_capture() -> (Self, Option<mpsc::UnboundedReceiver<Vec<u8>>>) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let (pcm_tx, pcm_rx) = mpsc::unbounded_channel();
            let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

            let _worker_thread = std::thread::spawn(move || {
                let host = cpal::default_host();
                let device = match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        warn!("No default audio input device found");
                        return;
                    }
                };

                let config = match device.default_input_config() {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Failed to query default audio input config: {}", e);
                        return;
                    }
                };

                let sample_format = config.sample_format();
                let err_fn = |err| error!("Audio input stream error: {}", err);

                let stream_res = match sample_format {
                    cpal::SampleFormat::F32 => {
                        let tx = pcm_tx.clone();
                        device.build_input_stream(
                            &config.into(),
                            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                let mut pcm = Vec::with_capacity(data.len() * 2);
                                for &sample in data {
                                    let clamped = sample.clamp(-1.0, 1.0);
                                    let s = (clamped * 32767.0) as i16;
                                    pcm.extend_from_slice(&s.to_le_bytes());
                                }
                                let _ = tx.send(pcm);
                            },
                            err_fn,
                            None,
                        )
                    }
                    cpal::SampleFormat::I16 => {
                        let tx = pcm_tx.clone();
                        device.build_input_stream(
                            &config.into(),
                            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                let mut pcm = Vec::with_capacity(data.len() * 2);
                                for &sample in data {
                                    pcm.extend_from_slice(&sample.to_le_bytes());
                                }
                                let _ = tx.send(pcm);
                            },
                            err_fn,
                            None,
                        )
                    }
                    _ => {
                        warn!("Unsupported audio sample format: {:?}", sample_format);
                        return;
                    }
                };

                let stream = match stream_res {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Failed to build audio stream: {}", e);
                        return;
                    }
                };

                if let Err(e) = stream.play() {
                    warn!("Failed to play audio stream: {}", e);
                    return;
                }

                info!("Audio capture stream started on dedicated thread");
                while stop_rx.try_recv().is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                drop(stream);
                info!("Audio capture stream stopped");
            });

            (Self { stop_tx: Some(stop_tx) }, Some(pcm_rx))
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            warn!("Audio capture not supported on this platform build");
            (Self { stop_tx: None }, None)
        }
    }
}

impl Drop for AudioCaptureService {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}
