use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::mpsc,
    thread,
};

pub use data::log::Error;

const MAX_LOG_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

/// Filename for the rotated previous log (sibling of the current log file).
///
/// When the app starts or when the log exceeds `MAX_LOG_FILE_SIZE`, the current
/// log is renamed to this filename. Only one generation of previous log is kept.
const PREVIOUS_LOG_FILENAME: &str = "kairos-previous.log";

enum LogMessage {
    Content(Vec<u8>),
    Flush,
    Shutdown,
}

pub fn setup(is_debug: bool) -> Result<(), Error> {
    let default_level = if is_debug {
        log::Level::Debug
    } else {
        log::Level::Info
    };

    let level_filter = std::env::var("RUST_LOG")
        .ok()
        .as_deref()
        .map(str::parse::<log::Level>)
        .transpose()?
        .unwrap_or(default_level)
        .to_level_filter();

    let mut io_sink = fern::Dispatch::new().format(|out, message, record| {
        out.finish(format_args!(
            "{}:{} -- {}",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            record.level(),
            message
        ));
    });

    if is_debug {
        io_sink = io_sink.chain(std::io::stdout());
    } else {
        let log_path = data::log::path_under(crate::infra::platform::data_path(None).as_path());
        initial_rotation(&log_path)?;

        let logger: Box<dyn Write + Send> = Box::new(BackgroundLogger::new(log_path)?);

        io_sink = io_sink.chain(logger);
    }

    fern::Dispatch::new()
        .level(log::LevelFilter::Off)
        .level_for("panic", log::LevelFilter::Error)
        .level_for("iced_wgpu", log::LevelFilter::Info)
        .level_for("data", level_filter)
        .level_for("exchange", level_filter)
        .level_for("kairos", level_filter)
        .chain(io_sink)
        .apply()?;

    Ok(())
}

fn initial_rotation(log_path: &PathBuf) -> io::Result<()> {
    let path = PathBuf::from(".");

    let dir = log_path.parent().unwrap_or(&path);

    let previous_log_path = dir.join(PREVIOUS_LOG_FILENAME);

    if previous_log_path.exists() {
        fs::remove_file(&previous_log_path)?;
    }

    if log_path.exists() {
        fs::rename(log_path, &previous_log_path)?;
    }

    Ok(())
}

struct BackgroundLogger {
    sender: mpsc::Sender<LogMessage>,
    _thread_handle: thread::JoinHandle<()>,
}

impl BackgroundLogger {
    fn new(path: PathBuf) -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel();

        let thread_handle = thread::Builder::new()
            .name("logger-thread".to_string())
            .spawn(move || {
                let mut logger = match Logger::new(&path) {
                    Ok(logger) => logger,
                    Err(e) => {
                        eprintln!("Failed to initialize logger: {}", e);
                        return;
                    }
                };

                loop {
                    match receiver.recv() {
                        Ok(LogMessage::Content(data)) => {
                            if let Err(e) = logger.write_all(&data) {
                                eprintln!("Logging error: {}", e);
                            }
                        }
                        Ok(LogMessage::Flush) => {
                            if let Err(e) = logger.flush() {
                                eprintln!("Error flushing logs: {}", e);
                            }
                        }
                        Ok(LogMessage::Shutdown) | Err(_) => break,
                    }
                }
            })?;

        Ok(BackgroundLogger {
            sender,
            _thread_handle: thread_handle,
        })
    }
}

impl Write for BackgroundLogger {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len();
        self.sender
            .send(LogMessage::Content(buf.to_vec()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Logger thread disconnected"))?;
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sender
            .send(LogMessage::Flush)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Logger thread disconnected"))?;
        Ok(())
    }
}

impl Drop for BackgroundLogger {
    fn drop(&mut self) {
        let _ = self.sender.send(LogMessage::Shutdown);
    }
}

struct Logger {
    path: PathBuf,
    file: fs::File,
    current_size: u64,
}

impl Logger {
    fn new(path: &PathBuf) -> io::Result<Self> {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let size = file.metadata()?.len();

        Ok(Logger {
            path: path.clone(),
            file,
            current_size: size,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Flush and drop the current file handle by replacing with a new one
        self.file.flush()?;

        // Determine the .old path next to the current log file
        let old_path = self
            .path
            .parent()
            .map(|p| p.join(PREVIOUS_LOG_FILENAME))
            .unwrap_or_else(|| PathBuf::from(PREVIOUS_LOG_FILENAME));

        // Rename current log to .old (overwriting any previous)
        if let Err(e) = fs::rename(&self.path, &old_path) {
            eprintln!("Failed to rename log file during rotation: {e}");
        }

        // Open a fresh log file
        let new_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        self.file = new_file;
        self.current_size = 0;

        Ok(())
    }
}

impl Write for Logger {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len() as u64;

        if self.current_size + buf_len > MAX_LOG_FILE_SIZE {
            if let Err(e) = self.rotate() {
                eprintln!("Failed to rotate log file: {e}");
                return Ok(buf.len());
            }
        }

        let bytes = self.file.write(buf)?;
        self.current_size += bytes as u64;

        Ok(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
