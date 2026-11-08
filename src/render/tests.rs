use std::f32::consts::TAU;

use super::*;

const ACCENT: [u8; 4] = [42, 210, 110, 255];
const EMPTY: [u8; 4] = [24, 66, 44, 255];
const PALETTE: Palette = Palette {
    accent: ACCENT,
    empty: EMPTY,
    line: [83, 84, 90, 255],
    dot: [48, 48, 53, 255],
};

fn put(input: &mut [u8; HEADER_BYTES], offset: usize, value: u32) {
    input[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn header(width: u32, height: u32, target: u32) -> [u8; HEADER_BYTES] {
    let mut input = [0; HEADER_BYTES];
    for (offset, value) in [
        (0, 1),
        (4, width),
        (8, height),
        (12, target),
        (40, 16000),
        (44, 512),
    ] {
        put(&mut input, offset, value);
    }
    input
}

fn columns(pixels: &[u8], width: usize, color: [u8; 4]) -> Vec<usize> {
    let mut columns = vec![0; width];
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if *pixel == color {
            columns[index % width] += 1;
        }
    }
    columns
}

#[test]
fn silence_shows_twelve_panel_and_twenty_four_desktop_tracks() {
    for (target, width, height, tracks) in [(0, 58, 30, 12), (1, 320, 116, 24)] {
        let frame = Frame::parse(&header(width, height, target)).unwrap();
        let mut pixels = vec![99; MAX_OUTPUT_BYTES];
        let size = render(&frame, &[[0; 2]; SAMPLES], PALETTE, &mut pixels).unwrap();
        assert_eq!(size, (width * height * 4) as usize);
        assert_eq!(pixels[size], 99);
        let mut inside_track = false;
        let mut count = 0;
        for column in columns(&pixels[..size], width as usize, EMPTY) {
            if column != 0 && !inside_track {
                count += 1;
            }
            inside_track = column != 0;
        }
        assert_eq!(count, tracks);
        assert!(!pixels[..size].as_chunks::<4>().0.contains(&ACCENT));
        assert!(pixels[..size].as_chunks::<4>().0.contains(&[0; 4]));
    }
}

#[test]
fn bass_and_treble_peak_in_different_parts_of_the_desktop() {
    let mut pixels = vec![0; MAX_OUTPUT_BYTES];
    for (bin, expected) in [(5, 0..100), (160, 220..320)] {
        let frame = Frame::parse(&header(320, 116, 1)).unwrap();
        let audio = std::array::from_fn::<_, SAMPLES, _>(|index| {
            let phase = TAU * bin as f32 * index as f32 / 512.0;
            ((phase.sin() * 18000.0) as i16).to_le_bytes()
        });
        let size = render(&frame, &audio, PALETTE, &mut pixels).unwrap();
        let levels = columns(&pixels[..size], 320, ACCENT);
        let (peak, height) = levels
            .into_iter()
            .enumerate()
            .max_by_key(|(_, level)| *level)
            .unwrap();
        assert!(
            expected.contains(&peak),
            "bin {bin} peaked at column {peak}"
        );
        assert!(height >= 20);
    }
}

#[test]
fn unavailable_audio_and_dc_offset_are_idle() {
    let mut input = header(320, 116, 1);
    let frame = Frame::parse(&input).unwrap();
    let mut pixels = vec![0; MAX_OUTPUT_BYTES];
    let size = render(
        &frame,
        &[2000_i16.to_le_bytes(); SAMPLES],
        PALETTE,
        &mut pixels,
    )
    .unwrap();
    assert!(!pixels[..size].as_chunks::<4>().0.contains(&ACCENT));

    let idle = pixels[..size].to_vec();
    put(&mut input, 40, 0);
    put(&mut input, 44, 0);
    let frame = Frame::parse(&input).unwrap();
    let size = render(&frame, &[], PALETTE, &mut pixels).unwrap();
    assert_eq!(pixels[..size], idle);
}

#[test]
fn malformed_frames_fail_without_touching_output() {
    let mut pixels = [99; 1024];
    for (offset, value) in [
        (0, 2),
        (4, 0),
        (4, 513),
        (8, 257),
        (12, 2),
        (36, 1),
        (40, 8000),
        (44, 511),
    ] {
        let mut input = header(16, 16, 0);
        put(&mut input, offset, value);
        assert!(Frame::parse(&input).is_none());
    }
    let frame = Frame::parse(&header(58, 30, 0)).unwrap();
    assert_eq!(
        render(&frame, &[[0; 2]; SAMPLES], PALETTE, &mut pixels),
        None
    );
    assert_eq!(render(&frame, &[], PALETTE, &mut pixels), None);
    assert_eq!(pixels, [99; 1024]);
}
