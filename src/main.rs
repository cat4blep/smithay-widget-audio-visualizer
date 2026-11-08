mod environment;
mod render;

use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    os::{fd::FromRawFd, unix::fs::FileExt},
    process::ExitCode,
};

use environment::Environment;
use render::{AUDIO_BYTES, Frame, HEADER_BYTES, MAX_OUTPUT_BYTES};

fn main() -> ExitCode {
    // Stderr is a protocol stream, including if a frame fails.
    std::panic::set_hook(Box::new(|_| {}));
    // The supervisor passes its writable frame buffer as fd 1. This is the
    // only owner in the widget, and nothing writes through std::io::stdout.
    let output = unsafe { File::from_raw_fd(1) };
    let result = Environment::open(env::args_os().skip(1)).and_then(|environment| {
        serve(
            io::stdin().lock(),
            &output,
            io::stderr().lock(),
            environment,
        )
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn serve(
    mut input: impl Read,
    output: &File,
    mut ready: impl Write,
    mut environment: Environment,
) -> io::Result<()> {
    let mut header = [0; HEADER_BYTES];
    let mut audio = [0; AUDIO_BYTES];
    let mut pixels = vec![0; MAX_OUTPUT_BYTES];
    while read_header(&mut input, &mut header)? {
        let frame = Frame::parse(&header).ok_or(io::ErrorKind::InvalidData)?;
        let audio = &mut audio[..frame.audio_bytes()];
        input.read_exact(audio)?;
        let palette = environment.palette(&frame)?;
        let length = render::render(&frame, audio.as_chunks::<2>().0, palette, &mut pixels)
            .ok_or(io::ErrorKind::InvalidData)?;
        output.write_all_at(&pixels[..length], 0)?;
        ready.write_all(b"SMD1")?;
        ready.flush()?;
    }
    Ok(())
}

fn read_header(input: &mut impl Read, frame: &mut [u8; HEADER_BYTES]) -> io::Result<bool> {
    let count = loop {
        match input.read(frame) {
            Ok(count) => break count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    };
    if count == 0 {
        return Ok(false);
    }
    input.read_exact(&mut frame[count..])?;
    Ok(true)
}
