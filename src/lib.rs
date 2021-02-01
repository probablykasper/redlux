use fdk_aac::dec::{Decoder as AacDecoder, DecoderError, Transport};
use std::io::{Read, Seek};
use std::time::Duration;

pub mod adts;

pub enum Format {
  Mp4,
  Aac,
}

pub enum Reader<R> {
  Mp4Reader(mp4::Mp4Reader<R>),
  AacReader(R),
}

pub struct Decoder<R>
where
  R: Read + Seek,
{
  pub format: Format,
  reader: Reader<R>,
  aac_decoder: AacDecoder,
  bytes: Vec<u8>,
  current_pcm_index: usize,
  current_pcm: Vec<i16>,
  track_id: u32,
  position: u32,
}

impl<R> Decoder<R>
where
  R: Read + Seek,
{
  pub fn new_aac(reader: R) -> Result<Decoder<R>, &'static str> {
    let aac_decoder = AacDecoder::new(Transport::Adts);
    let aac_decoder = Decoder {
      format: Format::Aac,
      reader: Reader::AacReader(reader),
      aac_decoder: aac_decoder,
      bytes: Vec::new(),
      current_pcm_index: 0,
      current_pcm: Vec::new(),
      track_id: 0,
      position: 1,
    };
    // aac_decoder.next();
    Ok(aac_decoder)
  }
  pub fn new_mpeg4(reader: R, size: u64) -> Result<Decoder<R>, &'static str> {
    let aac_decoder = AacDecoder::new(Transport::Adts);
    let mp4 = mp4::Mp4Reader::read_header(reader, size).or(Err("Error reading MPEG header"))?;
    let mut track_id: Option<u32> = None;
    {
      for track in mp4.tracks().iter() {
        let media_type = track.media_type().or(Err("Error getting media type"))?;
        match media_type {
          mp4::MediaType::AAC => {
            track_id = Some(track.track_id());
            break;
          }
          _ => {}
        }
      }
    }
    match track_id {
      Some(track_id) => {
        return Ok(Decoder {
          format: Format::Mp4,
          reader: Reader::Mp4Reader(mp4),
          aac_decoder: aac_decoder,
          bytes: Vec::new(),
          current_pcm_index: 0,
          current_pcm: Vec::new(),
          track_id: track_id,
          position: 1,
        });
      }
      None => {
        return Err("No aac track found");
      }
    }
  }
  pub fn current_frame_len(&self) -> Option<usize> {
    let frame_size: usize = self.aac_decoder.decoded_frame_size();
    Some(frame_size)
  }
  pub fn channels(&self) -> u16 {
    let num_channels: i32 = self.aac_decoder.stream_info().numChannels;
    num_channels as _
  }
  pub fn sample_rate(&self) -> u32 {
    let sample_rate: i32 = self.aac_decoder.stream_info().sampleRate;
    sample_rate as _
  }
  pub fn total_duration(&self) -> Option<Duration> {
    return None;
  }
}

impl<R> Iterator for Decoder<R>
where
  R: Read + Seek,
{
  type Item = i16;
  fn next(&mut self) -> Option<i16> {
    if self.current_pcm_index == self.current_pcm.len() {
      let mut pcm = vec![0; 8192];
      let result = match self.aac_decoder.decode_frame(&mut pcm) {
        Err(DecoderError::NOT_ENOUGH_BITS) => {
          match &mut self.reader {
            // mp4
            Reader::Mp4Reader(mp4_reader) => {
              let sample_result = mp4_reader.read_sample(self.track_id, self.position);
              let sample = match sample_result {
                Ok(sample) => sample?, // None if EOF
                Err(_) => {
                  println!("Error reading sample");
                  return None;
                }
              };
              let tracks = mp4_reader.tracks();
              let track = match tracks.get(self.track_id as usize - 1) {
                Some(track) => track,
                None => {
                  println!("No track ID there");
                  return None;
                }
              };
              let adts_header = match adts::construct_adts_header(track, &sample) {
                Some(bytes) => bytes,
                None => {
                  println!("Error getting adts header bytes");
                  return None;
                }
              };
              let adts_bytes = mp4::Bytes::copy_from_slice(&adts_header);
              self.bytes = [adts_bytes, sample.bytes].concat();
              self.position += 1;
            }
            // aac
            Reader::AacReader(aac_reader) => {
              let old_bytes_len = self.bytes.len();
              let mut new_bytes = vec![0; 8192 - old_bytes_len];
              let bytes_read = match aac_reader.read(&mut new_bytes) {
                Ok(bytes_read) => bytes_read,
                Err(_) => return None,
              };
              // aac files already have adts headers
              self.bytes.extend(new_bytes);
            }
          }
          let bytes_filled = match self.aac_decoder.fill(&self.bytes) {
            Ok(bytes_filled) => bytes_filled,
            Err(_) => return None,
          };
          self.bytes = self.bytes[bytes_filled..].to_vec();
          self.aac_decoder.decode_frame(&mut pcm)
        }
        val => val,
      };
      match result {
        Ok(_) => {}
        Err(err) => {
          println!("DecoderError: {}", err);
          return None;
        }
      }
      let decoded_frame_size = self.aac_decoder.decoded_frame_size();
      if decoded_frame_size < pcm.len() {
        let _ = pcm.split_off(decoded_frame_size);
      }
      self.current_pcm = pcm;
      self.current_pcm_index = 0;
    }
    let value = self.current_pcm[self.current_pcm_index];
    self.current_pcm_index += 1;
    return Some(value);
  }
}

impl<R> rodio::Source for Decoder<R>
where
  R: Read + Seek,
{
  fn current_frame_len(&self) -> Option<usize> {
    return self.current_frame_len();
  }
  fn channels(&self) -> u16 {
    return self.channels();
  }
  fn sample_rate(&self) -> u32 {
    return self.sample_rate();
  }
  fn total_duration(&self) -> Option<Duration> {
    return self.total_duration();
  }
}
