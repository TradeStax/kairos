use std::{
    borrow::Cow,
    collections::HashMap,
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
    let overrides = parse_rust_log();

    let default_level = if is_debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    let level_for = |target: &str| -> log::LevelFilter {
        overrides.get(target).copied().unwrap_or(default_level)
    };

    let io_sink = if is_debug {
        use fern::colors::{Color, ColoredLevelConfig};
        let colors = ColoredLevelConfig::new()
            .error(Color::Red)
            .warn(Color::Yellow)
            .info(Color::Green)
            .debug(Color::Cyan)
            .trace(Color::White);

        fern::Dispatch::new()
            .format(move |out, message, record| {
                out.finish(format_args!(
                    "{} {:<5} [{}] {}",
                    chrono::Local::now().format("%H:%M:%S%.3f"),
                    colors.color(record.level()),
                    shorten_target(record.target()),
                    message
                ));
            })
            .chain(std::io::stdout())
    } else {
        let log_path = data::log::path_under(crate::infra::platform::data_path(None).as_path());
        initial_rotation(&log_path)?;
        let logger: Box<dyn Write + Send> = Box::new(BackgroundLogger::new(log_path)?);

        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} {:<5} [{}] {}",
                    chrono::Local::now().format("%H:%M:%S%.3f"),
                    record.level(),
                    shorten_target(record.target()),
                    message
                ));
            })
            .chain(logger)
    };

    fern::Dispatch::new()
        .level(log::LevelFilter::Off)
        .level_for("panic", log::LevelFilter::Error)
        .level_for("iced_wgpu", log::LevelFilter::Warn)
        .level_for("kairos_data", level_for("kairos_data"))
        .level_for("kairos_study", level_for("kairos_study"))
        .level_for("kairos_backtest", level_for("kairos_backtest"))
        .level_for("kairos", level_for("kairos"))
        .chain(io_sink)
        .apply()?;

    Ok(())
}

fn shorten_target(target: &str) -> Cow<'_, str> {
    if let Some(rest) = target.strip_prefix("kairos_data::") {
        Cow::Owned(format!("data::{rest}"))
    } else if let Some(rest) = target.strip_prefix("kairos_study::") {
        Cow::Owned(format!("study::{rest}"))
    } else if let Some(rest) = target.strip_prefix("kairos_backtest::") {
        Cow::Owned(format!("backtest::{rest}"))
    } else if let Some(rest) = target.strip_prefix("kairos::") {
        Cow::Borrowed(rest)
    } else {
        Cow::Borrowed(target)
    }
}

fn parse_rust_log() -> HashMap<String, log::LevelFilter> {
    let mut map = HashMap::new();
    let Ok(val) = std::env::var("RUST_LOG") else {
        return map;
    };

    // Bare level: RUST_LOG=debug
    if let Ok(level) = val.parse::<log::Level>() {
        let filter = level.to_level_filter();
        map.insert("kairos".to_string(), filter);
        map.insert("kairos_data".to_string(), filter);
        map.insert("kairos_study".to_string(), filter);
        map.insert("kairos_backtest".to_string(), filter);
        return map;
    }

    // Per-target: RUST_LOG=kairos_data=debug,kairos=info
    for part in val.split(',') {
        let part = part.trim();
        if let Some((target, level_str)) = part.split_once('=')
            && let Ok(level) = level_str.parse::<log::Level>()
        {
            map.insert(target.to_string(), level.to_level_filter());
        }
    }

    map
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
    thread_handle: Option<thread::JoinHandle<()>>,
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
            thread_handle: Some(thread_handle),
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
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
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

        if self.current_size + buf_len > MAX_LOG_FILE_SIZE
            && let Err(e) = self.rotate()
        {
            eprintln!("Failed to rotate log file: {e}");
            return Ok(buf.len());
        }

        let bytes = self.file.write(buf)?;
        self.current_size += bytes as u64;

        Ok(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
