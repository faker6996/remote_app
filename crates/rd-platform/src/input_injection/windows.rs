use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::InputInjector,
    error::InjectionError,
};
use tracing::debug;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYBD_EVENT_FLAGS,
    MOUSEINPUT,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_HWHEEL, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

/// Windows input injection using Win32 SendInput API
pub struct WindowsInputInjector {
    screen_width: i32,
    screen_height: i32,
}

impl WindowsInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        debug!("Initializing Windows input injector");
        let (screen_width, screen_height) = unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            (if w <= 0 { 1920 } else { w }, if h <= 0 { 1080 } else { h })
        };
        
        Ok(Self {
            screen_width,
            screen_height,
        })
    }
    
    fn send_input_raw(&self, input: INPUT) -> Result<(), InjectionError> {
        let sent = unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32)
        };
        if sent != 1 {
            return Err(InjectionError::InjectionFailed(
                "Windows SendInput returned 0 events sent".to_string()
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl InputInjector for WindowsInputInjector {
    async fn inject(&mut self, event: InputEvent) -> Result<(), InjectionError> {
        match event {
            InputEvent::MouseMove { x, y } => {
                // Normalize to 0..65535 range for MOUSEEVENTF_ABSOLUTE
                let norm_x = ((x as i64 * 65535) / (self.screen_width as i64).max(1)) as i32;
                let norm_y = ((y as i64 * 65535) / (self.screen_height as i64).max(1)) as i32;
                
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: norm_x,
                            dy: norm_y,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                self.send_input_raw(input)
            }
            
            InputEvent::MouseButton { button, pressed } => {
                let flags = match (button, pressed) {
                    (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                    (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                    (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                    (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                    (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                    _ => return Ok(()),
                };
                
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                self.send_input_raw(input)
            }
            
            InputEvent::MouseScroll { delta_x, delta_y } => {
                if delta_y != 0 {
                    let input = INPUT {
                        r#type: INPUT_MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: 0,
                                dy: 0,
                                mouseData: (delta_y * 120) as u32,
                                dwFlags: MOUSEEVENTF_WHEEL,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    };
                    self.send_input_raw(input)?;
                }
                
                if delta_x != 0 {
                    let input = INPUT {
                        r#type: INPUT_MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: 0,
                                dy: 0,
                                mouseData: (delta_x * 120) as u32,
                                dwFlags: MOUSEEVENTF_HWHEEL,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    };
                    self.send_input_raw(input)?;
                }
                
                Ok(())
            }
            
            InputEvent::KeyPress { key, pressed } => {
                let flags = if pressed {
                    KEYBD_EVENT_FLAGS(0)
                } else {
                    KEYEVENTF_KEYUP
                };
                
                let input = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(key.0 as u16),
                            wScan: 0,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                self.send_input_raw(input)
            }
        }
    }
}
