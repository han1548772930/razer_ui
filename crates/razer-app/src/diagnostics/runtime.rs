//! Bounded, application-owned diagnostic I/O. Logging never waits for disk or stderr.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

const QUEUE_CAPACITY: usize = 1024;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

struct Queue {
    sender: Mutex<Option<SyncSender<String>>>,
    overflow: AtomicUsize,
}

impl Queue {
    fn record(&self, line: String) {
        let Ok(sender) = self.sender.lock() else { return };
        if let Some(sender) = sender.as_ref()
            && let Err(TrySendError::Full(_)) = sender.try_send(line)
        {
            self.overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn close(&self) {
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }
    }
}

struct Logger(Arc<Queue>);

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            || metadata.level() <= log::Level::Info && metadata.target().starts_with("razer_")
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let elapsed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            self.0.record(format!(
                "[unix={}.{:03}] pid={} {} {}: {}\n",
                elapsed.as_secs(),
                elapsed.subsec_millis(),
                std::process::id(),
                record.level(),
                record.target(),
                record.args(),
            ));
        }
    }

    // Schedule a flush without blocking a caller on I/O. The process owner drains
    // and joins the writer after the event loop returns.
    fn flush(&self) {
        self.0.record(String::new());
    }
}

pub(crate) struct Runtime {
    queue: Arc<Queue>,
    worker: Option<JoinHandle<()>>,
}

impl Runtime {
    pub(crate) fn install(path: PathBuf) -> Option<Self> {
        let runtime = Self::start(QUEUE_CAPACITY, move |receiver, queue| {
            write_records(receiver, queue, path);
        });
        let runtime = match runtime {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = writeln!(io::stderr(), "[diagnostics] cannot start writer: {error}");
                return None;
            }
        };
        if let Err(error) = log::set_boxed_logger(Box::new(Logger(runtime.queue.clone()))) {
            let _ = writeln!(io::stderr(), "[diagnostics] cannot install logger: {error}");
            return None;
        }
        log::set_max_level(log::LevelFilter::Info);
        log::info!(target: "razer_ui::diagnostics", "Application diagnostic writer started");
        Some(runtime)
    }

    fn start(
        capacity: usize,
        writer: impl FnOnce(mpsc::Receiver<String>, &Queue) + Send + 'static,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let queue = Arc::new(Queue {
            sender: Mutex::new(Some(sender)),
            overflow: AtomicUsize::new(0),
        });
        let worker_queue = queue.clone();
        let worker = thread::Builder::new()
            .name("diagnostic-writer".into())
            .spawn(move || writer(receiver, &worker_queue))?;
        Ok(Self {
            queue,
            worker: Some(worker),
        })
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.queue.close();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            let _ = writeln!(
                io::stderr(),
                "[diagnostics] writer panicked during shutdown"
            );
        }
    }
}

fn write_records(receiver: mpsc::Receiver<String>, queue: &Queue, path: PathBuf) {
    let mut output = FileOutput::open(path.clone())
        .map_err(|error| {
            let _ = writeln!(io::stderr(), "[diagnostics] {}: {error}", path.display());
            error
        })
        .ok();
    for line in receiver {
        write_overflow(queue, &mut output);
        write_line(&line, &mut output);
    }
    write_overflow(queue, &mut output);
}

fn write_overflow(queue: &Queue, output: &mut Option<FileOutput>) {
    let lost = queue.overflow.swap(0, Ordering::Relaxed);
    if lost > 0 {
        write_line(
            &format!("[diagnostics] queue full; {lost} records were dropped\n"),
            output,
        );
    }
}

fn write_line(line: &str, output: &mut Option<FileOutput>) {
    let _ = io::stderr().write_all(line.as_bytes());
    if let Some(writer) = output
        && let Err(error) = writer.append(line.as_bytes())
    {
        let _ = writeln!(
            io::stderr(),
            "[diagnostics] {}: {error}",
            writer.path.display()
        );
        *output = None;
    }
}

struct FileOutput {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
}

impl FileOutput {
    fn open(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path,
            file: Some(file),
            bytes,
        })
    }

    fn append(&mut self, line: &[u8]) -> io::Result<()> {
        if self.bytes > 0 && self.bytes.saturating_add(line.len() as u64) > MAX_FILE_BYTES {
            self.file.take();
            std::fs::rename(&self.path, self.path.with_extension("previous.log"))?;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
            self.bytes = 0;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("diagnostic file closed"))?;
        file.write_all(line)?;
        file.flush()?;
        self.bytes += line.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
