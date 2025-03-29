use byteorder::{BigEndian, WriteBytesExt};

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

pub fn flv_tag(tag_type: u8, timestamp: u32, data: &[u8]) -> Vec<u8> {
  let mut tag = Vec::new();
  let data_size = data.len() as u32;

  // Tag header (11 bytes)
  tag.push(tag_type); // 0x08 = audio, 0x09 = video

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