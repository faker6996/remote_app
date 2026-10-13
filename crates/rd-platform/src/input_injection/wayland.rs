use async_trait::async_trait;
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop};
use ashpd::desktop::PersistMode;
use rd_core::domain::{
    error::InjectionError,
    models::{InputEvent, MouseButton},
    ports::InputInjector,
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Wayland input injector using the XDG Desktop Portal RemoteDesktop interface
pub struct WaylandInputInjector {
    tx: mpsc::Sender<InputEvent>,
}

fn to_evdev_button(btn: MouseButton) -> i32 {
    match btn {
        MouseButton::Left => 0x110,   // BTN_LEFT (272)
        MouseButton::Right => 0x111,  // BTN_RIGHT (273)
        MouseButton::Middle => 0x112, // BTN_MIDDLE (274)
        MouseButton::X1 => 0x113,     // BTN_SIDE (275)
        MouseButton::X2 => 0x114,     // BTN_EXTRA (276)
    }
}

fn to_evdev_keycode(key: u32) -> u32 {
    match key {
        // Numbers 0-9
        18 => 2,  // 1
        19 => 3,  // 2
        20 => 4,  // 3
        21 => 5,  // 4
        23 => 6,  // 5
        22 => 7,  // 6
        26 => 8,  // 7
        28 => 9,  // 8
        25 => 10, // 9
        29 => 11, // 0
        // Letters A-Z
        0 => 30,  // A
        11 => 48, // B
        8 => 46,  // C
        2 => 32,  // D
        14 => 18, // E
        3 => 33,  // F
        5 => 34,  // G
        4 => 35,  // H
        34 => 23, // I
        38 => 36, // J
        40 => 37, // K
        37 => 38, // L
        46 => 50, // M
        45 => 49, // N
        31 => 24, // O
        35 => 25, // P
        12 => 16, // Q
        15 => 19, // R
        1 => 31,  // S
        17 => 20, // T
        32 => 22, // U
        9 => 47,  // V
        13 => 17, // W
        7 => 45,  // X
        16 => 21, // Y
        6 => 44,  // Z
        // Common editing and control keys
        36 => 28,  // Enter
        48 => 15,  // Tab
        49 => 57,  // Space
        51 => 14,  // Backspace
        53 => 1,   // Escape
        56 | 60 => 42, // Shift
        59 | 62 => 29, // Ctrl
        58 | 61 => 56, // Alt
        55 => 125, // Meta / Super
        123 => 105, // Arrow Left
        124 => 106, // Arrow Right
        125 => 108, // Arrow Down
        126 => 103, // Arrow Up
        other => {
            if (1..=255).contains(&other) {
                other
            } else {
                30
            }
        }
    }
}

impl WaylandInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        info!("Initializing Wayland Input Injector (XDG Desktop Portal RemoteDesktop)");
        let (tx, mut rx) = mpsc::channel::<InputEvent>(128);

        tokio::spawn(async move {
            let proxy = match RemoteDesktop::new().await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to create RemoteDesktop portal proxy: {}", e);
                    return;
                }
            };

            let session = match proxy.create_session().await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create RemoteDesktop portal session: {}", e);
                    return;
                }
            };

            if let Err(e) = proxy
                .select_devices(
                    &session,
                    DeviceType::Keyboard | DeviceType::Pointer,
                    None,
                    PersistMode::DoNot,
                )
                .await
            {
                error!("Failed to select devices on RemoteDesktop portal: {}", e);
                return;
            }

            match proxy.start(&session, None).await {
                Ok(res) => {
                    info!("RemoteDesktop portal session started: {:?}", res.response());
                }
                Err(e) => {
                    warn!("RemoteDesktop portal start prompt dismissed or failed: {}", e);
                    return;
                }
            }

            let mut last_x = 0.0;
            let mut last_y = 0.0;

            while let Some(event) = rx.recv().await {
                match event {
                    InputEvent::MouseMove { x, y } => {
                        let cur_x = x as f64;
                        let cur_y = y as f64;
                        let dx = cur_x - last_x;
                        let dy = cur_y - last_y;
                        last_x = cur_x;
                        last_y = cur_y;

                        // Try absolute pointer motion first, fall back to relative
                        if proxy.notify_pointer_motion_absolute(&session, 0, cur_x, cur_y).await.is_err() {
                            let _ = proxy.notify_pointer_motion(&session, dx, dy).await;
                        }
                    }
                    InputEvent::MouseButton { button, pressed } => {
                        let btn_code = to_evdev_button(button);
                        let state = if pressed {
                            KeyState::Pressed
                        } else {
                            KeyState::Released
                        };
                        let _ = proxy.notify_pointer_button(&session, btn_code, state).await;
                    }
                    InputEvent::MouseScroll { delta_x, delta_y } => {
                        let dx = delta_x as f64;
                        let dy = delta_y as f64;
                        let _ = proxy.notify_pointer_axis(&session, dx, dy, false).await;
                    }
                    InputEvent::KeyPress { key, pressed } => {
                        let code = to_evdev_keycode(key.0) as i32;
                        let state = if pressed {
                            KeyState::Pressed
                        } else {
                            KeyState::Released
                        };
                        let _ = proxy.notify_keyboard_keycode(&session, code, state).await;
                    }
                }
            }
        });

        Ok(Self { tx })
    }
}

#[async_trait]
impl InputInjector for WaylandInputInjector {
    async fn inject(&mut self, event: InputEvent) -> Result<(), InjectionError> {
        self.tx
            .send(event)
            .await
            .map_err(|e| InjectionError::InjectionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wayland_input_injector_lifecycle() {
        let mut injector = WaylandInputInjector::new().expect("Failed to create WaylandInputInjector");
        let res = injector
            .inject(InputEvent::MouseMove { x: 100, y: 150 })
            .await;
        assert!(res.is_ok());

        let res_btn = injector
            .inject(InputEvent::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            })
            .await;
        assert!(res_btn.is_ok());
    }
}
