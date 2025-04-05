-- Add migration script here
CREATE TABLE IF NOT EXISTS encoder_settings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  video_bitrate INTEGER NOT NULL,
  audio_bitrate INTEGER NOT NULL,
  video_codec TEXT NOT NULL,
  audio_codec TEXT NOT NULL,
  preset TEXT NOT NULL,
  tune TEXT,
  bufsize INTEGER,
  framerate INTEGER,
  resolution TEXT,
  use_passthrough BOOLEAN DEFAULT false
);