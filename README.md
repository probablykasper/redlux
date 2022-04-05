# redlux

[![Crates.io](https://img.shields.io/crates/v/redlux.svg)](https://crates.io/crates/redlux)
[![Documentation](https://docs.rs/redlux/badge.svg)](https://docs.rs/redlux)

AAC decoder for MPEG-4 (MP4, M4A etc) and AAC files, with rodio support

Reads MPEG-4 containers using [rust-mp4](https://crates.io/crates/mp4), and then constructs ADTS headers for it. Decodes AAC to PCM using [fdk-aac c-bindings](https://crates.io/crates/fdk-aac). Check the examples for usage with [rodio](https://crates.io/crates/rodio).

Supports AAC-LC, HE-AAC v1 (SBR) and HE-AAC v2 (PS).

## Caveats
Would appreciate any help with figuring these out:
1. It only decodes the first AAC track it finds in an MPEG-4 container.
2. MPEG files with CRC are probably not supported.
3. According to [this MultimediaWiki page](https://wiki.multimedia.cx/index.php/ADTS), 13 bits of the ADTS header is for specifying the frame length, and this number must include the ADTS header itself. For 8 channel audio, I would assume the frame length could be 8192 bytes, and if we add the header bytes on top of that, it would exceed what 13 bits can carry. Is this a potential issue?
4. Not sure about the licensing situation. Is fdk-aac free to use? Are AAC patent licenses needed?

## Dev instructions

### Get started

Install [Rust](https://www.rust-lang.org).

Run tests:
```
cargo test
```

Build:
```
cargo build
```

### Releasing a new version

1. Update `CHANGELOG.md`
2. Bump the version number in `Cargo.toml`
3. Run `cargo test`
4. Create a git tag in format `v#.#.#`
5. Publish on crates.io:
    1. Login by running `cargo login` and following the instructions
    2. Test publish to ensure there are no errors/warnings
        ```
        cargo publish --dry-run
        ```
    3. Publish
        ```
        cargo publish
        ```
6. Create GitHub release with release notes
