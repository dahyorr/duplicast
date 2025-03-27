#[derive(Debug, Clone)]
pub enum AppEvents {
    StreamStarted,
    StreamStopped,
    StreamPreviewFailed,
}

impl AppEvents {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppEvents::StreamStarted => "stream-active",
            AppEvents::StreamStopped => "stream-ended",
            AppEvents::StreamPreviewFailed => "stream-preview-failed",
        }
    }
}
