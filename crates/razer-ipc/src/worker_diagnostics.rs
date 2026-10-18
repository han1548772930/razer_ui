//! Bounded process diagnostics, separate from the device response channel.
use anyhow::Context as _;
use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{Arc, Mutex},
};

const TAIL_BYTES: usize = 16 * 1024;

#[derive(Default)]
struct Tail {
    bytes: VecDeque<u8>,
    truncated: bool,
}

impl Tail {
    fn append(&mut self, bytes: &[u8]) {
        let excess = self
            .bytes
            .len()
            .saturating_add(bytes.len())
            .saturating_sub(TAIL_BYTES);
        if excess != 0 {
            self.truncated = true;
            self.bytes.drain(..excess.min(self.bytes.len()));
        }
        self.bytes
            .extend(bytes.iter().skip(bytes.len().saturating_sub(TAIL_BYTES)));
    }

    fn text(&self) -> String {
        let bytes: Vec<_> = self.bytes.iter().copied().collect();
        let text = String::from_utf8_lossy(&bytes);
        if self.truncated {
            format!("[earlier bytes omitted]\n{text}")
        } else {
            text.into_owned()
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct WorkerDiagnostics {
    stderr: Arc<Mutex<Tail>>,
    stdout: Arc<Mutex<Tail>>,
}

impl WorkerDiagnostics {
    pub(super) fn capture_stderr(&self, mut reader: impl Read) -> anyhow::Result<()> {
        let mut bytes = [0; 4096];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => return Ok(()),
                Ok(count) => self
                    .stderr
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Worker stderr diagnostic buffer poisoned"))?
                    .append(&bytes[..count]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error).context("读取设备服务 stderr 失败"),
            }
        }
    }

    pub(super) fn record_stdout(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.stdout
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker stdout diagnostic buffer poisoned"))?
            .append(bytes);
        Ok(())
    }

    pub(super) fn attach(&self, error: anyhow::Error) -> anyhow::Error {
        let mut diagnostic = String::new();
        for (name, tail) in [("stderr", &self.stderr), ("stdout", &self.stdout)] {
            match tail.lock() {
                Ok(tail) if !tail.bytes.is_empty() => {
                    diagnostic.push_str(&format!("\n{name}:\n{}", tail.text()));
                }
                Err(_) => diagnostic.push_str(&format!("\n{name}: diagnostic buffer poisoned")),
                _ => {}
            }
        }
        if diagnostic.is_empty() {
            error
        } else {
            anyhow::anyhow!("{error:#}\n设备服务诊断（有界尾部）:{diagnostic}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_tail_keeps_latest_bytes_across_fragments_and_large_chunks() {
        let mut tail = Tail::default();
        tail.append(&vec![b'a'; TAIL_BYTES - 2]);
        tail.append(b"bcde");
        assert_eq!(tail.bytes.len(), TAIL_BYTES);
        assert!(tail.text().starts_with("[earlier bytes omitted]\n"));
        assert!(tail.text().ends_with("bcde"));
        tail.append(&vec![b'z'; TAIL_BYTES + 400]);
        assert_eq!(tail.bytes.len(), TAIL_BYTES);
        assert!(tail.bytes.iter().all(|byte| *byte == b'z'));
    }

    #[test]
    fn stderr_without_newline_and_legacy_stdout_are_retained_separately() {
        let diagnostics = WorkerDiagnostics::default();
        diagnostics
            .capture_stderr(&b"cleanup failed before flush"[..])
            .unwrap();
        diagnostics.record_stdout(&[0xff, b'\n']).unwrap();
        let error = diagnostics.attach(anyhow::anyhow!("worker closed"));
        let text = format!("{error:#}");
        assert!(text.contains("worker closed"));
        assert!(text.contains("stderr:\ncleanup failed before flush"));
        assert!(text.contains("stdout:\n\u{fffd}\n"));
    }
}
