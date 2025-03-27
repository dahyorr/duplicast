#[derive(Debug, Clone)]
pub enum AppEvents {
  StreamStarted,
  StreamStopped,
}

impl AppEvents {
  pub fn as_str(&self) -> &'static str {
      match self {
          AppEvents::StreamStarted => "stream-active",
          AppEvents::StreamStopped => "stream-inactive",
      }
  }
}