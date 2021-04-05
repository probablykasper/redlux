use mp4::{AudioObjectType, ChannelConfig, Mp4Sample, SampleFreqIndex};
use std::ops::Range;
use crate::Error;

fn get_bits(byte: u16, range: Range<u16>) -> u16 {
  let shaved_left = byte << range.start - 1;
  let moved_back = shaved_left >> range.start - 1;
  let shave_right = moved_back >> 16 - range.end;
  return shave_right;
}

fn get_bits_u8(byte: u8, range: Range<u8>) -> u8 {
  let shaved_left = byte << range.start - 1;
  let moved_back = shaved_left >> range.start - 1;
  let shave_right = moved_back >> 8 - range.end;
  return shave_right;
}

pub fn construct_adts_header(
  object_type: AudioObjectType,
  sample_freq_index: SampleFreqIndex,
  channel_config: ChannelConfig,
  sample: &Mp4Sample,
) -> Result<Vec<u8>, Error> {
  // ADTS header wiki reference: https://wiki.multimedia.cx/index.php/ADTS#:~:text=Audio%20Data%20Transport%20Stream%20(ADTS,to%20stream%20audio%2C%20usually%20AAC.

  // byte7 and byte9 not included without CRC
  let adts_header_length = 7;

  // AAAA_AAAA
  let byte0 = 0b1111_1111;

  // AAAA_BCCD
  // D: Only support 1 (without CRC)
  let byte1 = 0b1111_0001;

  // EEFF_FFGH
  let mut byte2 = 0b0000_0000;
  let object_type = match object_type {
    AudioObjectType::AacLowComplexity => 2,
    // Audio object types 5 (SBR) and 29 (PS) are coerced to type 2 (AAC-LC).
    // The decoder will have to detect SBR/PS. This is called "Implicit
    // Signaling" and it's the only option for ADTS.
    AudioObjectType::SpectralBandReplication => 2, // SBR, needed to support HE-AAC v1
    AudioObjectType::ParametricStereo => 2, // PS, needed to support HE-AAC v2
    aot => return Err(Error::UnsupportedObjectType(aot)),
  };
  let adts_object_type = object_type - 1;
  byte2 = (byte2 << 2) | adts_object_type; // EE

  let sample_freq_index = match sample_freq_index {
    SampleFreqIndex::Freq96000 => 0,
    SampleFreqIndex::Freq88200 => 1,
    SampleFreqIndex::Freq64000 => 2,
    SampleFreqIndex::Freq48000 => 3,
    SampleFreqIndex::Freq44100 => 4,
    SampleFreqIndex::Freq32000 => 5,
    SampleFreqIndex::Freq24000 => 6,
    SampleFreqIndex::Freq22050 => 7,
    SampleFreqIndex::Freq16000 => 8,
    SampleFreqIndex::Freq12000 => 9,
    SampleFreqIndex::Freq11025 => 10,
    SampleFreqIndex::Freq8000 => 11,
    SampleFreqIndex::Freq7350 => 12,
    // 13-14 = reserved
    // 15 = explicit frequency (forbidden in adts)
  };
  byte2 = (byte2 << 4) | sample_freq_index; // FFFF
  byte2 = (byte2 << 1) | 0b1; // G

  let channel_config = match channel_config {
    // 0 = for when channel config is sent via an inband PCE
    ChannelConfig::Mono => 1,
    ChannelConfig::Stereo => 2,
    ChannelConfig::Three => 3,
    ChannelConfig::Four => 4,
    ChannelConfig::Five => 5,
    ChannelConfig::FiveOne => 6,
    ChannelConfig::SevenOne => 7,
    // 8-15 = reserved
  };
  byte2 = (byte2 << 1) | get_bits_u8(channel_config, 6..6); // H

  // HHIJ_KLMM
  let mut byte3 = 0b0000_0000;
  byte3 = (byte3 << 2) | get_bits_u8(channel_config, 7..8); // HH
  byte3 = (byte3 << 4) | 0b1111; // IJKL

  let frame_length = adts_header_length + sample.bytes.len() as u16;
  byte3 = (byte3 << 2) | get_bits(frame_length, 3..5) as u8; // MM

  // MMMM_MMMM
  let byte4 = get_bits(frame_length, 6..13) as u8;

  // MMMO_OOOO
  let mut byte5 = 0b0000_0000;
  byte5 = (byte5 << 3) | get_bits(frame_length, 14..16) as u8;
  byte5 = (byte5 << 5) | 0b11111; // OOOOO

  // OOOO_OOPP
  let mut byte6 = 0b0000_0000;
  byte6 = (byte6 << 6) | 0b111111; // OOOOOO
  byte6 = (byte6 << 2) | 0b00; // PP

  return Ok(vec![byte0, byte1, byte2, byte3, byte4, byte5, byte6])
}
