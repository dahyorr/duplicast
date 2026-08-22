//! Wraps RTMP audio/video message payloads (which are byte-identical to FLV tag
//! bodies) into a minimal streamed FLV container so they can be fed into a
//! GStreamer `appsrc ! flvdemux ! ...` pipeline.

const TAG_TYPE_AUDIO: u8 = 8;
const TAG_TYPE_VIDEO: u8 = 9;

pub fn flv_header() -> Vec<u8> {
    vec![
        0x46, 0x4C, 0x56, // "FLV"
        0x01, // version 1
        0x05, // flags: audio + video present
        0x00, 0x00, 0x00, 0x09, // data offset (9, size of this header)
        0x00, 0x00, 0x00, 0x00, // PreviousTagSize0 (always 0)
    ]
}

pub fn wrap_video_tag(timestamp_ms: u32, payload: &[u8]) -> Vec<u8> {
    wrap_tag(TAG_TYPE_VIDEO, timestamp_ms, payload)
}

pub fn wrap_audio_tag(timestamp_ms: u32, payload: &[u8]) -> Vec<u8> {
    wrap_tag(TAG_TYPE_AUDIO, timestamp_ms, payload)
}

fn wrap_tag(tag_type: u8, timestamp_ms: u32, payload: &[u8]) -> Vec<u8> {
    let size = payload.len() as u32;
    let mut out = Vec::with_capacity(11 + payload.len() + 4);

    out.push(tag_type);
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.push((timestamp_ms >> 16) as u8);
    out.push((timestamp_ms >> 8) as u8);
    out.push(timestamp_ms as u8);
    out.push((timestamp_ms >> 24) as u8); // timestamp extended byte
    out.push(0);
    out.push(0);
    out.push(0); // stream id, always 0

    out.extend_from_slice(payload);

    let tag_size = 11 + size;
    out.push((tag_size >> 24) as u8);
    out.push((tag_size >> 16) as u8);
    out.push((tag_size >> 8) as u8);
    out.push(tag_size as u8);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_nine_bytes_plus_previous_tag_size() {
        let header = flv_header();
        assert_eq!(header.len(), 13);
        assert_eq!(&header[0..3], b"FLV");
        assert_eq!(header[3], 1);
        assert_eq!(header[4], 0x05);
        assert_eq!(&header[5..9], &[0, 0, 0, 9]);
        assert_eq!(&header[9..13], &[0, 0, 0, 0]);
    }

    #[test]
    fn wrap_tag_builds_correct_header_and_trailer() {
        let payload = [0xAA, 0xBB, 0xCC, 0xDD];
        let tag = wrap_video_tag(0x0102_0304, &payload);

        assert_eq!(tag.len(), 11 + payload.len() + 4);
        assert_eq!(tag[0], 9); // video tag type
        assert_eq!(&tag[1..4], &[0, 0, 4]); // data size = 4
        assert_eq!(&tag[4..7], &[0x02, 0x03, 0x04]); // lower 24 bits of timestamp
        assert_eq!(tag[7], 0x01); // extended timestamp byte (upper 8 bits)
        assert_eq!(&tag[8..11], &[0, 0, 0]); // stream id
        assert_eq!(&tag[11..15], &payload);

        let previous_tag_size = u32::from_be_bytes([tag[15], tag[16], tag[17], tag[18]]);
        assert_eq!(previous_tag_size, 11 + payload.len() as u32);
    }

    #[test]
    fn audio_tag_uses_type_eight() {
        let tag = wrap_audio_tag(0, &[0x01]);
        assert_eq!(tag[0], 8);
    }
}
