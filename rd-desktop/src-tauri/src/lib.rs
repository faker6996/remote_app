use std::sync::Arc;
use tokio::sync::Mutex;
use rd_client::RemoteSession;
use rd_transport::quic::QuicClient;

// State to hold the active remote session
struct AppState {
    session: Option<Arc<Mutex<RemoteSession>>>,
}

// Connect to a remote agent
#[tauri::command]
async fn connect_agent(
    server_addr: String,
    agent_id: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    // Install crypto provider
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    
    // Create QUIC client and connect
    let client = QuicClient::new().map_err(|e| e.to_string())?;
    let connection = client
        .connect(server_addr.parse().map_err(|e: std::net::AddrParseError| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?;
    
    // Create transport
    let transport = rd_transport::QuicTransport::new(connection)
        .await
        .map_err(|e| e.to_string())?;
    
    // Create remote session
    let mut session = RemoteSession::new(Arc::new(Mutex::new(transport)))
        .await
        .map_err(|e| e.to_string())?;
    
    // Connect to agent
    let session_id = session
        .connect(agent_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    
    // Store session in state
    let mut app_state = state.lock().await;
    app_state.session = Some(Arc::new(Mutex::new(session)));
    
    Ok(format!("Connected to agent {} with session {}", agent_id, session_id))
}

// Disconnect from current session
#[tauri::command]
async fn disconnect(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let mut app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        session.lock().await.disconnect().await.map_err(|e| e.to_string())?;
        app_state.session = None;
        Ok("Disconnected".to_string())
    } else {
        Err("No active session".to_string())
    }
}

// Get next frame from session
#[tauri::command]
async fn get_frame(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<Option<Vec<u8>>, String> {
    let app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        if let Some(frame) = session.receive_frame().await {
            // Return frame data as base64 or raw bytes
            Ok(Some(frame.data))
        } else {
            Ok(None)
        }
    } else {
        Err("No active session".to_string())
    }
}

// Send input event
#[tauri::command]
async fn send_input(
    event_type: String,
    x: i32,
    y: i32,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        
        // Create input event based on type
        let event = match event_type.as_str() {
            "mouse_move" => rd_core::domain::models::InputEvent::MouseMove { x, y },
            "mouse_down" => rd_core::domain::models::InputEvent::MouseButton {
                button: rd_core::domain::models::MouseButton::Left,
                pressed: true,
            },
            "mouse_up" => rd_core::domain::models::InputEvent::MouseButton {
                button: rd_core::domain::models::MouseButton::Left,
                pressed: false,
            },
            _ => return Err(format!("Unknown event type: {}", event_type)),
        };
        
        session.send_input(event).await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("No active session".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(Mutex::new(AppState { session: None }));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            connect_agent,
            disconnect,
            get_frame,
            send_input
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
