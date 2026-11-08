use std::f32::consts::FRAC_1_SQRT_2;

const MAX_WIDTH: usize = 512;
const MAX_HEIGHT: usize = 256;
const RATE: u32 = 16000;
const SAMPLES: usize = 512;
const BANDS: usize = 24;
const EDGES: [usize; BANDS + 1] = [
    1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 15, 19, 24, 30, 38, 48, 61, 77, 97, 122, 153, 178, 204, 231,
    257,
];

pub const HEADER_BYTES: usize = 48;
pub const AUDIO_BYTES: usize = SAMPLES * 2;
pub const MAX_OUTPUT_BYTES: usize = MAX_WIDTH * MAX_HEIGHT * 4;

type Color = [u8; 4];
type Complex = (f32, f32);

// Unit rotations exp(-2πi / n) for n = 2, 4, …, 512.
const ROTATIONS: [Complex; 9] = [
    (-1.0, 0.0),
    (0.0, -1.0),
    (FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    (0.9238795, -0.38268343),
    (0.98078525, -0.19509032),
    (0.9951847, -0.09801714),
    (0.99879545, -0.049067676),
    (0.9996988, -0.024541229),
    (0.9999247, -0.012271538),
];

#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color,
    pub line: Color,
    pub dot: Color,
    pub empty: Color,
}

pub struct Frame {
    width: usize,
    height: usize,
    panel: bool,
    samples: usize,
    pub environment_changed: bool,
    pub environment_revision: u32,
}

impl Frame {
    pub fn parse(input: &[u8; HEADER_BYTES]) -> Option<Self> {
        let field = |offset| u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap());
        let width = field(4) as usize;
        let height = field(8) as usize;
        let target = field(12);
        let environment_revision = field(36);
        if field(0) != 1
            || !(1..=MAX_WIDTH).contains(&width)
            || !(1..=MAX_HEIGHT).contains(&height)
            || target > 1
            || environment_revision & 1 != 0
        {
            return None;
        }
        let samples = match (field(40), field(44) as usize) {
            (0 | RATE, 0) => 0,
            (RATE, SAMPLES) => SAMPLES,
            _ => return None,
        };
        Some(Self {
            width,
            height,
            panel: target == 0,
            samples,
            environment_changed: field(32) & 1 != 0,
            environment_revision,
        })
    }

    pub fn audio_bytes(&self) -> usize {
        self.samples * 2
    }
}

pub fn render(
    frame: &Frame,
    audio: &[[u8; 2]],
    palette: Palette,
    pixels: &mut [u8],
) -> Option<usize> {
    if audio.len() != frame.samples {
        return None;
    }
    let Frame {
        width,
        height,
        panel,
        ..
    } = *frame;
    let length = width * height * 4;
    let output = pixels.get_mut(..length)?;
    let levels = if audio.is_empty() {
        [0.0; BANDS]
    } else {
        analyze(audio)
    };
    output.fill(0);
    let output = output.as_chunks_mut::<4>().0;
    let span = if panel { height.min(18) } else { height };
    let bottom = (height + span) / 2;
    if !panel && span >= 8 {
        draw_grid(output, width, bottom, span, palette.line, palette.dot);
    }

    let gap = if panel { 1 } else { 2 };
    let ideal_bars = if panel { BANDS / 2 } else { BANDS };
    let bars = ideal_bars.min((width / (gap + 1)).max(1));
    for bar in 0..bars {
        let left = width * bar / bars;
        let next = width * (bar + 1) / bars;
        let right = next.saturating_sub(gap).max(left + 1);
        let bands = &levels[BANDS * bar / bars..BANDS * (bar + 1) / bars];
        let level = bands.iter().copied().fold(0.0, f32::max);
        let color = if level > 0.0 {
            palette.accent
        } else {
            palette.empty
        };
        let rows = (level * span as f32).round().max(1.0) as usize;
        for y in bottom - rows..bottom {
            output[y * width + left..y * width + right].fill(color);
        }
    }
    Some(length)
}

fn draw_grid(
    output: &mut [Color],
    width: usize,
    bottom: usize,
    span: usize,
    line: Color,
    dot: Color,
) {
    let baseline = (bottom - 1) * width;
    output[baseline..baseline + width].fill(line);
    for quarter in 1..=3 {
        let y = bottom - 1 - span * quarter / 4;
        for x in (0..width).step_by(7) {
            output[y * width + x] = dot;
        }
    }
}

fn analyze(samples: &[[u8; 2]]) -> [f32; BANDS] {
    let sum: i32 = samples
        .iter()
        .map(|&pcm| i32::from(i16::from_le_bytes(pcm)))
        .sum();
    let mean = sum as f32 / SAMPLES as f32;
    let mut bins = [(0.0, 0.0); SAMPLES];
    let mut phase = (1.0, 0.0);
    for (index, &pcm) in samples.iter().enumerate() {
        let reversed = index.reverse_bits() >> (usize::BITS - SAMPLES.trailing_zeros());
        let window = 0.5 - 0.5 * phase.0;
        bins[reversed].0 = (f32::from(i16::from_le_bytes(pcm)) - mean) * window / 32768.0;
        phase = multiply(phase, ROTATIONS[ROTATIONS.len() - 1]);
    }

    for (stage, rotation) in ROTATIONS.into_iter().enumerate() {
        let half = 1 << stage;
        for group in bins.chunks_exact_mut(2 * half) {
            let (even, odd) = group.split_at_mut(half);
            let mut phase = (1.0, 0.0);
            for (even, odd) in even.iter_mut().zip(odd) {
                let (real, imaginary) = multiply(*odd, phase);
                *odd = (even.0 - real, even.1 - imaginary);
                *even = (even.0 + real, even.1 + imaginary);
                phase = multiply(phase, rotation);
            }
        }
    }

    std::array::from_fn(|band| {
        let power = bins[EDGES[band]..EDGES[band + 1]]
            .iter()
            .map(|&(real, imaginary)| real * real + imaginary * imaginary)
            .fold(0.0, f32::max);
        // The Hann window halves a bin-centred tone's coherent amplitude.
        let amplitude = power.sqrt() * (4.0 / SAMPLES as f32);
        // Keep near-silence flat and compress the remaining range for short bars.
        ((amplitude - 0.003).max(0.0) / 0.997).sqrt().min(1.0)
    })
}

fn multiply((a, b): Complex, (c, d): Complex) -> Complex {
    (a * c - b * d, a * d + b * c)
}

#[cfg(test)]
mod tests;
