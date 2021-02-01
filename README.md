# redlux

AAC decoder for MPEG-4 (MP4, M4A etc) and AAC files. Supports rodio.

[![Crates.io](https://img.shields.io/crates/v/redlux.svg)](https://crates.io/crates/redlux)
[![Documentation](https://docs.rs/redlux/badge.svg)](https://docs.rs/redlux)

## Caveats
1. It only decodes the first AAC track it finds in an MPEG-4 container.
2. Not sure if MPEG files with CRC are supported.
3. According to [this MultimediaWiki page](https://wiki.multimedia.cx/index.php/ADTS), 13 bits of the ADTS header is for specifying the frame length, and this number must include the ADTS header itself. For 8 channel audio, I would assume the frame length could be 8192 bytes, and if we add the header bytes on top of that, it would exceed what 13 bits can carry. Is this a potential issue? No idea.

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
4. Commit and tag in format `v#.#.#`
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
