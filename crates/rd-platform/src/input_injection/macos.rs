use async_trait::async_trait;
use rd_core::domain::{
    models::*,
    ports::InputInjector,
    error::InjectionError,
};
use tracing::debug;

/// macOS input injection using CGEvent
pub struct MacOSInputInjector;

impl MacOSInputInjector {
    pub fn new() -> Result<Self, InjectionError> {
        debug!("Initializing macOS input injector");
        Ok(Self)
    }
}

#[async_trait]
impl InputInjector for MacOSInputInjector {
    async fn inject(&mut self, _event: InputEvent) -> Result<(), InjectionError> {
        // TODO: Implement CGEvent input injection
        
        Err(InjectionError::UnsupportedPlatform)
    }
}
