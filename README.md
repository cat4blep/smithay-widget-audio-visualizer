# Audio visualizer for Smithay Desktop

Shows frequencies of audio playing on the default output. The panel displays 12 bars and the resizable desktop card displays 24. The widget computes a 512-point, Hann-windowed FFT from the host's mono PCM and draws the bars in the active Smithay palette. Silence and unavailable audio show idle tracks.

## Install

In **Widgets**, paste `https://github.com/cat4blep/smithay-widget-audio-visualizer` into the GitHub field. Review the requested output audio permission, choose **Install widget**, then enable **Panel**, **Desktop**, or both. This build targets Linux x86-64. The host obtains samples through PulseAudio or PipeWire with PulseAudio compatibility and `parec`.

## Build and test

Run these commands from the repository on Linux x86-64 with Rustup. The build uses the linker bundled with Rust.

```sh
rustup toolchain install 1.88.0 --profile minimal
rustup target add --toolchain 1.88.0 x86_64-unknown-linux-musl
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
    cargo +1.88.0 build --locked --release --target x86_64-unknown-linux-musl
cmp widget-linux-x86_64 target/x86_64-unknown-linux-musl/release/smithay-audio-visualizer
install -m 755 target/x86_64-unknown-linux-musl/release/smithay-audio-visualizer widget-linux-x86_64
```

```sh
cargo +1.88.0 test --locked
python3 tests/protocol.py ./widget-linux-x86_64
```

The Rust tests exercise silence, tones, and render bounds. The executable reads `/environment` by default. The Python test runs it with pipes and a memfd and supplies a fixture through `--environment PATH`.

## Native widget API v1

Each stdin record starts with a 48-byte little-endian header followed by exactly twice the sample count in bytes.

| Offset | Type | Field |
| --- | --- | --- |
| 0 | u32 | API version, 1 |
| 4, 8 | u32 | Width and height, up to 512 × 256 |
| 12 | u32 | Target, 0 for panel and 1 for desktop |
| 16 | u64 | Monotonic time in milliseconds |
| 24, 28 | i32 | Click x and y, or −1 for no click |
| 32 | u32 | Flags. Bit 0 signals an environment change |
| 36 | u32 | Even environment revision |
| 40, 44 | u32 | Sample rate and count. Audio uses 16,000 Hz and 512 samples. Count 0 carries no samples |
| 48 | i16[] | Mono PCM samples |

The host mounts a 64-byte read-only `/environment` file. The u32 version and sequence occupy offsets 0 and 4. Starting at offset 8, the ten RGBA colors are background, outline, accent, dim, line, dot, selected, empty meter, surface, and navigation. Font size and panel edge follow at offsets 48 and 52 as u32 fields. The panel edge is 0 for top and 1 for bottom. Offsets 56 and 60 are reserved.

The visualizer reads accent, line, dot, and empty meter on the first frame and after an environment change. It accepts a palette when the sequence is even and agrees before and after the color reads.

For each complete frame the widget writes top-down RGBA at offset zero to the memfd on stdout. It then writes the four bytes `SMD1` to stderr. EOF between frames exits successfully. Invalid or incomplete records exit with an error status.
