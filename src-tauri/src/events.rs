#[derive(Debug, Clone)]
pub enum AppEvents {
    StreamPreviewActive,
    StreamActive,
    StreamPreviewFailed,
    // ServersReady,
    StreamEnded,
    StreamPreviewEnded,
    RelayActive, 
    RelayEnded,
    RelayFailed,
}

impl AppEvents {
    pub fn as_str(&self) -> &'static str {
        match self {
            // AppEvents::ServersReady => "servers-ready",
            AppEvents::StreamActive => "stream-active",
            AppEvents::StreamPreviewActive => "stream-preview-active",
            AppEvents::StreamPreviewEnded => "stream-preview-ended",
            AppEvents::StreamEnded => "stream-ended",
            AppEvents::StreamPreviewFailed => "stream-preview-failed",
            AppEvents::RelayActive => "relay-active",
            AppEvents::RelayEnded => "relay-ended",
            AppEvents::RelayFailed => "relay-failed",
        }
    }
}
