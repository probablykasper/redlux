use fdk_aac::dec::{Decoder as AacDecoder, DecoderError, Transport};
use std::error;
use std::fmt;
use std::io::{Read, Seek};
use std::time::Duration;

pub mod adts;

#[derive(Debug)]
pub enum Error {
  /// Error reading header of file
  FileHeaderError,
  /// Unable to get information about a track, such as audio profile, sample
  /// frequency or channel config.
  TrackReadingError,
  // Unable to find track  in file
  TrackNotFound,
  /// Error decoding track
  TrackDecodingError(DecoderError),
  /// Error getting samples
  SamplesError,
}

impl error::Error for Error {}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Oh no, something bad went down")
  }
}

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
  pub decoder_error: Option<Error>,
}

impl<R> Decoder<R>
where
  R: Read + Seek,
{
  pub fn new_aac(reader: R) -> Self {
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
      decoder_error: None,
    };
    return aac_decoder;
  }
  pub fn new_mpeg4(reader: R, size: u64) -> Result<Self, Error> {
    let aac_decoder = AacDecoder::new(Transport::Adts);
    let mp4 = mp4::Mp4Reader::read_header(reader, size).or(Err(Error::FileHeaderError))?;
    let mut track_id: Option<u32> = None;
    {
      for track in mp4.tracks().iter() {
        let media_type = match track.media_type() {
          Ok(media_type) => media_type,
          Err(_) => continue,
        };
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
          decoder_error: None,
        });
      }
      None => {
        return Err(Error::TrackNotFound);
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
        Err(DecoderError::NOT_ENOUGH_BITS) | Err(DecoderError::TRANSPORT_SYNC_ERROR) => {
          match &mut self.reader {
            // mp4
            Reader::Mp4Reader(mp4_reader) => {
              let sample_result = mp4_reader.read_sample(self.track_id, self.position);
              let sample = match sample_result {
                Ok(sample) => sample?, // None if EOF
                Err(_) => {
                  self.decoder_error = Some(Error::SamplesError);
                  return None;
                }
              };
              let tracks = mp4_reader.tracks();
              let track = match tracks.get(self.track_id as usize - 1) {
                Some(track) => track,
                None => {
                  self.decoder_error = Some(Error::TrackNotFound);
                  return None;
                }
              };
              let object_type = match track.audio_profile() {
                Ok(value) => value,
                Err(_) => {
                  self.decoder_error = Some(Error::TrackReadingError);
                  return None;
                }
              };
              let sample_freq_index = match track.sample_freq_index() {
                Ok(value) => value,
                Err(_) => {
                  self.decoder_error = Some(Error::TrackReadingError);
                  return None;
                }
              };
              let channel_config = match track.channel_config() {
                Ok(value) => value,
                Err(_) => {
                  self.decoder_error = Some(Error::TrackReadingError);
                  return None;
                }
              };
              let adts_header = adts::construct_adts_header(
                object_type,
                sample_freq_index,
                channel_config,
                &sample,
              );
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
              if bytes_read == 0 {
                return None;
              }
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
          self.decoder_error = Some(Error::TrackDecodingError(err));
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
