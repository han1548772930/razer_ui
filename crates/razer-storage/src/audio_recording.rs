//! Temporary local PCM files stay owned until their playback service releases them.
use anyhow::{Context as _, ensure};
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct RecordingFile {
    path: PathBuf,
    removed: bool,
}

impl RecordingFile {
    pub fn write(samples: &[f32], canceled: impl Fn() -> bool) -> anyhow::Result<Self> {
        Self::write_in(
            &std::env::temp_dir().join("razer-ui-microphone"),
            samples,
            canceled,
        )
    }

    fn write_in(
        directory: &Path,
        samples: &[f32],
        canceled: impl Fn() -> bool,
    ) -> anyhow::Result<Self> {
        ensure!(
            !canceled(),
            "Microphone playback canceled before preparing PCM"
        );
        ensure!(!samples.is_empty(), "Recorded PCM has no samples");
        std::fs::create_dir_all(directory)?;
        let (owner, file) = Self::create(directory)?;
        let mut output = BufWriter::new(file);
        for packet in samples.chunks(4096) {
            ensure!(
                !canceled(),
                "Microphone playback canceled while preparing actual PCM"
            );
            for sample in packet {
                output.write_all(&sample.to_le_bytes())?;
            }
        }
        output.flush()?;
        drop(output);
        ensure!(
            !canceled(),
            "Microphone playback canceled after preparing actual PCM"
        );
        Ok(owner)
    }

    fn create(directory: &Path) -> std::io::Result<(Self, File)> {
        static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = directory.join(format!(
                "{}-{}.f32",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok((
                        Self {
                            path,
                            removed: false,
                        },
                        file,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn cleanup(mut self) -> anyhow::Result<()> {
        self.remove().context("Microphone recorded PCM cleanup")
    }

    fn remove(&mut self) -> std::io::Result<()> {
        if !self.removed {
            match std::fs::remove_file(&self.path) {
                Ok(()) => self.removed = true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => self.removed = true,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for RecordingFile {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            eprintln!("Microphone recorded PCM cleanup: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "razer-recording-tests-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir(&self.0).unwrap();
        }
    }

    #[test]
    fn independent_recordings_keep_exact_pcm_until_each_owner_releases() {
        let directory = Directory::new();
        let samples = [0.25f32, -0.5, 0.0];
        let first = RecordingFile::write_in(&directory.0, &samples, || false).unwrap();
        let second = RecordingFile::write_in(&directory.0, &samples, || false).unwrap();
        assert_ne!(first.path(), second.path());
        assert_eq!(
            std::fs::read(first.path()).unwrap(),
            [0, 0, 128, 62, 0, 0, 0, 191, 0, 0, 0, 0]
        );
        let first_path = first.path().to_owned();
        first.cleanup().unwrap();
        assert!(!first_path.exists());
        assert!(second.path().is_file());
        let second_path = second.path().to_owned();
        drop(second);
        assert!(!second_path.exists());
    }

    #[test]
    fn canceled_recording_only_removes_its_new_partial_file() {
        let directory = Directory::new();
        let retained = directory.0.join("retained.f32");
        std::fs::write(&retained, b"previous recording").unwrap();
        let calls = Cell::new(0);
        let result = RecordingFile::write_in(&directory.0, &vec![1.0; 8192], || {
            calls.set(calls.get() + 1);
            calls.get() == 3
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
        assert_eq!(std::fs::read(&retained).unwrap(), b"previous recording");
        std::fs::remove_file(retained).unwrap();
    }
}
