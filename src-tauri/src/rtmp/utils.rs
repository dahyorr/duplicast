use std::collections::HashMap;

use byteorder::{BigEndian, WriteBytesExt};
use rml_amf0::Amf0Value;
use rml_rtmp::sessions::StreamMetadata;

#[derive(Debug, Clone, Copy)]
pub enum FlvTagType {
    Audio = 0x08,
    Video = 0x09,
    ScriptData = 0x12,
}

pub fn flv_header() -> Vec<u8> {
    let mut header = Vec::new();

    // Signature: "FLV"
    header.extend_from_slice(b"FLV");

    // Version: 1
    header.push(0x01);

    // Flags: 0x05 = audio + video
    header.push(0x05);

    // DataOffset: header size (9)
    header.extend_from_slice(&[0x00, 0x00, 0x00, 0x09]);

    // PreviousTagSize0: always 0
    header.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

    header
}

pub fn flv_tag(tag_type: FlvTagType, timestamp: u32, data: &[u8]) -> Vec<u8> {
    let mut tag = Vec::new();
    let data_size = data.len() as u32;

    // Tag header (11 bytes)
    tag.push(tag_type as u8); // 0x08 = audio, 0x09 = video

    tag.write_u24::<BigEndian>(data_size).unwrap(); // DataSize
    tag.write_u24::<BigEndian>(timestamp & 0xFFFFFF).unwrap(); // Timestamp (lower 24 bits)
    tag.push(((timestamp >> 24) & 0xFF) as u8); // TimestampExtended
    tag.write_u24::<BigEndian>(0).unwrap(); // StreamID (always 0)

    // Payload
    tag.extend_from_slice(data);

    // PreviousTagSize
    let total_size = 11 + data.len();
    byteorder::WriteBytesExt::write_u32::<BigEndian>(&mut tag, total_size as u32).unwrap();
    tag
}

pub fn create_metadata_tag(metadata: &StreamMetadata) -> Vec<u8> {
    let mut props = HashMap::new(); // 👈 use HashMap instead of BTreeMap

    if let Some(width) = metadata.video_width {
        props.insert("width".into(), Amf0Value::Number(width as f64));
    }
    if let Some(height) = metadata.video_height {
        props.insert("height".into(), Amf0Value::Number(height as f64));
    }
    if let Some(fps) = metadata.video_frame_rate {
        props.insert("framerate".into(), Amf0Value::Number(fps as f64));
    }
    if let Some(audio_sample_rate) = metadata.audio_sample_rate {
        props.insert(
            "audiosamplerate".into(),
            Amf0Value::Number(audio_sample_rate as f64),
        );
    }
    if let Some(channels) = metadata.audio_channels {
        props.insert("audiochannels".into(), Amf0Value::Number(channels as f64));
    }
    if let Some(bitrate) = metadata.video_bitrate_kbps {
        props.insert("videodatarate".into(), Amf0Value::Number(bitrate as f64));
    }
    if let Some(encoder) = &metadata.encoder {
        props.insert("encoder".into(), Amf0Value::Utf8String(encoder.clone()));
    }

    let values = vec![
        Amf0Value::Utf8String("onMetaData".into()),
        Amf0Value::Object(props), // ✅ HashMap is expected here
    ];

    let body = rml_amf0::serialize(&values).expect("Failed to encode AMF");

    flv_tag(FlvTagType::ScriptData, 0, &body) // FLV script tag with timestamp 0
}

pub fn is_video_keyframe_avc_sequence_header(tag: &[u8]) -> bool {
  tag.len() > 13 && tag[0] == 0x09 && (tag[11] & 0xF0) == 0x10 && tag[12] == 0
}

pub fn is_audio_aac_sequence_header(tag: &[u8]) -> bool {
  tag.len() > 13 && tag[0] == 0x08 && (tag[11] & 0xF0) == 0xA0 && tag[12] == 0
}