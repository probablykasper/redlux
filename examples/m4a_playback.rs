use redlux::Decoder;
use rodio::{OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;

fn main() {
  let path = "tests/samples/Simbai & Elke Bay - Energy.m4a";
  let file = File::open(path).expect("Error opening file");

  let metadata = file.metadata().expect("Error getting file metadata");
  let size = metadata.len();
  let buf = BufReader::new(file);

  let decoder = Decoder::new_mpeg4(buf, size).expect("Error creating M4aDecoder");

  let output_stream = OutputStream::try_default();
  let (_stream, handle) = output_stream.expect("Error creating output stream");
  let sink = Sink::try_new(&handle).expect("Error creating sink");

  sink.append(decoder);
  sink.play();
  sink.set_volume(0.25);
  sink.sleep_until_end();
}
