use fdk_aac::dec::{Decoder as AacDecoder, DecoderError, Transport};
use std::io::{Read, Seek};
use std::time::Duration;

pub mod adts;

pub struct Decoder<R>
where
  R: Read + Seek,
{
  mp4_reader: mp4::Mp4Reader<R>,
  aac_decoder: AacDecoder,
  current_pcm_index: usize,
  current_pcm: Vec<i16>,
  track_id: u32,
  position: u32,
}

impl<R> Decoder<R>
where
  R: Read + Seek,
{
  pub fn new(reader: R, size: u64) -> Result<Decoder<R>, &'static str> {
    let decoder = AacDecoder::new(Transport::Adts);
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
          mp4_reader: mp4,
          aac_decoder: decoder,
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
      let result = match self.aac_decoder.decode_frame(&mut self.current_pcm) {
        Err(DecoderError::NOT_ENOUGH_BITS) => {
          let sample_result = self.mp4_reader.read_sample(self.track_id, self.position);
          let sample = match sample_result {
            Ok(sample) => sample?, // None if EOF
            Err(_) => {
              println!("Error reading sample");
              return None;
            }
          };
          let tracks = self.mp4_reader.tracks();
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
          let bytes = [adts_bytes, sample.bytes].concat();
          self.position += 1;
          let _bytes_read = match self.aac_decoder.fill(&bytes) {
            Ok(bytes_read) => bytes_read,
            Err(_) => return None,
          };
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
      let decoded_fram_size = self.aac_decoder.decoded_frame_size();
      if decoded_fram_size < pcm.len() {
        let _ = pcm.split_off(decoded_fram_size);
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
