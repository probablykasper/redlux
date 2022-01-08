//! AAC decoder for MPEG-4 (MP4, M4A etc) and AAC files. Supports rodio.
use fdk_aac::dec::{Decoder as AacDecoder, DecoderError, Transport};
use mp4::AudioObjectType;
use std::io::{Read, Seek};
use std::time::Duration;
use std::{error, fmt, io};

pub mod adts;

/// Redlux error
#[derive(Debug)]
pub enum Error {
  /// Error reading header of file
  FileHeaderError,
  /// Unable to get information about a track, such as audio profile, sample
  /// frequency or channel config.
  TrackReadingError,
  /// Unsupported audio object type
  UnsupportedObjectType(AudioObjectType),
  // Unable to find track in file
  TrackNotFound,
  /// Error decoding track
  TrackDecodingError(DecoderError),
  /// Error getting samples
  SamplesError,
  /// Error from the underlying reader R
  ReaderError(io::Error),
}

impl error::Error for Error {}

impl Error {
  pub fn message(&self) -> &'static str {
    match &self {
      Error::FileHeaderError => "Error reading file header",
      Error::TrackReadingError => "Error reading file track info",
      Error::UnsupportedObjectType(_) => "Unsupported audio object type",
      Error::TrackNotFound => "Unable to find track in file",
      Error::TrackDecodingError(_) => "Error decoding track",
      Error::SamplesError => "Error reading samples",
      Error::ReaderError(_) => "Error reading file",
    }
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.message())
  }
}

/// File container format
pub enum Format {
  Mp4,
  Aac,
}

/// Underlying reader
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
  /// If there's an error while iterating over the Decoder, that error is added here
  pub iter_error: Option<Error>,
}

impl<R> Decoder<R>
where
  R: Read + Seek,
{
  /// Create from an aac buffer
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
      iter_error: None,
    };
    return aac_decoder;
  }
  /// Create from an mpeg buffer
  pub fn new_mpeg4(reader: R, size: u64) -> Result<Self, Error> {
    let aac_decoder = AacDecoder::new(Transport::Adts);
    let mp4 = mp4::Mp4Reader::read_header(reader, size).or(Err(Error::FileHeaderError))?;
    let mut track_id: Option<u32> = None;
    {
      for track in mp4.tracks().values() {
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
          iter_error: None,
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
  /// Consume and return the next sample, or None when finished
  pub fn decode_next_sample(&mut self) -> Result<Option<i16>, Error> {
    if self.current_pcm_index == self.current_pcm.len() {
      let mut pcm = vec![0; 8192];
      let result = match self.aac_decoder.decode_frame(&mut pcm) {
        Err(DecoderError::NOT_ENOUGH_BITS) | Err(DecoderError::TRANSPORT_SYNC_ERROR) => {
          match &mut self.reader {
            // mp4
            Reader::Mp4Reader(mp4_reader) => {
              println!("track_id {}, sample_id {}", self.track_id, self.position);
              let sample_result = mp4_reader.read_sample(self.track_id, self.position);
              println!("sample {:?}", sample_result);
              let sample_opt = sample_result.or(Err(Error::SamplesError))?;
              let sample = match sample_opt {
                Some(sample) => sample,
                None => return Ok(None), // EOF
              };
              let tracks = mp4_reader.tracks();
              let track = tracks
                .get(&(self.track_id - 1))
                .ok_or(Error::TrackNotFound)?;
              let object_type = track.audio_profile().or(Err(Error::TrackReadingError))?;
              let sample_freq_index = track
                .sample_freq_index()
                .or(Err(Error::TrackReadingError))?;
              let channel_config = track.channel_config().or(Err(Error::TrackReadingError))?;
              let adts_header = adts::construct_adts_header(
                object_type,
                sample_freq_index,
                channel_config,
                &sample,
              )?;
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
                Err(err) => return Err(Error::ReaderError(err)),
              };
              if bytes_read == 0 {
                return Ok(None); // EOF
              }
              // aac files already have adts headers
              self.bytes.extend(new_bytes);
            }
          }
          let bytes_filled = match self.aac_decoder.fill(&self.bytes) {
            Ok(bytes_filled) => bytes_filled,
            Err(err) => return Err(Error::TrackDecodingError(err)),
          };
          self.bytes = self.bytes[bytes_filled..].to_vec();
          self.aac_decoder.decode_frame(&mut pcm)
        }
        val => val,
      };
      if let Err(err) = result {
        return Err(Error::TrackDecodingError(err));
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
    return Ok(Some(value));
  }
}

impl<R> Iterator for Decoder<R>
where
  R: Read + Seek,
{
  type Item = i16;
  /// Runs decode_next_sample and returns the sample from that. Once the
  /// iterator is finished, it returns None. If there's an error, it's added
  /// to the iter_error error.
  fn next(&mut self) -> Option<i16> {
    match self.decode_next_sample() {
      Ok(sample) => sample,
      Err(err) => {
        self.iter_error = Some(err);
        return None;
      }
    }
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
