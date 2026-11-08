use std::{ffi::OsString, fs::File, io, os::unix::fs::FileExt};

use crate::render::{Frame, Palette};

pub struct Environment {
    file: File,
    palette: Option<(u32, Palette)>,
}

impl Environment {
    pub fn open(mut arguments: impl Iterator<Item = OsString>) -> io::Result<Self> {
        let path = match arguments.next() {
            None => OsString::from("/environment"),
            Some(flag) if flag == "--environment" => {
                let path = arguments.next().ok_or(io::ErrorKind::InvalidInput)?;
                if arguments.next().is_some() {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
                path
            }
            Some(_) => return Err(io::ErrorKind::InvalidInput.into()),
        };
        Ok(Self {
            file: File::open(path)?,
            palette: None,
        })
    }

    pub fn palette(&mut self, frame: &Frame) -> io::Result<Palette> {
        if let Some((revision, palette)) = self.palette
            && !frame.environment_changed
            && revision == frame.environment_revision
        {
            return Ok(palette);
        }
        let palette = self.read_palette(frame.environment_revision)?;
        self.palette = Some((frame.environment_revision, palette));
        Ok(palette)
    }

    fn read_palette(&self, revision: u32) -> io::Result<Palette> {
        let mut version = [0; 4];
        self.file.read_exact_at(&mut version, 0)?;
        if u32::from_le_bytes(version) != 1 {
            return Err(io::ErrorKind::InvalidData.into());
        }

        let mut sequence = [0; 4];
        for _ in 0..128 {
            self.file.read_exact_at(&mut sequence, 4)?;
            let before = u32::from_le_bytes(sequence);
            // A concurrent writer marks an update with an odd sequence. The
            // subtraction also handles an even sequence wrapping back to zero.
            if before & 1 != 0 || before.wrapping_sub(revision) > i32::MAX as u32 {
                continue;
            }
            let mut palette = Palette {
                accent: [0; 4],
                line: [0; 4],
                dot: [0; 4],
                empty: [0; 4],
            };
            self.file.read_exact_at(&mut palette.accent, 16)?;
            self.file.read_exact_at(&mut palette.line, 24)?;
            self.file.read_exact_at(&mut palette.dot, 28)?;
            self.file.read_exact_at(&mut palette.empty, 36)?;
            self.file.read_exact_at(&mut sequence, 4)?;
            if u32::from_le_bytes(sequence) == before {
                return Ok(palette);
            }
        }
        Err(io::ErrorKind::InvalidData.into())
    }
}
