//! Asynchronous logging system with background writer thread.
//!
//! All log messages are sent through a channel to a dedicated writer thread,
//! avoiding blocking the main async runtime on file I/O.

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::sync::LazyLock;

use crossbeam_channel::{unbounded, Receiver, Sender};
use std::thread;

/// Log event types passed through the logging channel.
pub enum LogEvent {
    /// A log message with timestamp and prefix
    Message(String),
    /// Flush the writer buffer
    Flush,
    /// Shutdown the writer thread
    Shutdown,
}

/// Global log sender — lazily initialized on first access.
/// Spawns a background thread that writes to `LOG_FILE` (default: `app.log`).
pub static LOG_TX: LazyLock<Sender<LogEvent>> = LazyLock::new(|| {
    let (tx, rx) = unbounded();
    thread::spawn(|| log_writer(rx));
    tx
});

/// Background thread that receives log events and writes them to the log file.
/// Runs until it receives `LogEvent::Shutdown`.
fn log_writer(rx: Receiver<LogEvent>) {
    let log_path = std::env::var("LOG_FILE").unwrap_or_else(|_| "app.log".into());

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("Failed to open log file");

    let mut log = BufWriter::new(file);
    for event in rx {
        match event {
            LogEvent::Message(msg) => writeln!(log, "{}", msg).ok(),
            LogEvent::Flush => log.flush().ok(),
            LogEvent::Shutdown => break,
        };
    }
    let _ = log.flush();
}

/// Log a message with a prefix tag.
///
/// # Example
/// ```ignore
/// log!("REQUEST", &json_string);
/// ```
#[macro_export]
macro_rules! log {
    ($prefix:expr, $content:expr) => {
        $crate::logger::LOG_TX
            .send($crate::logger::LogEvent::Message(format!(
                "\n=== {} [{}] ===\n{}\n=== END {} ===",
                $prefix,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                $content,
                $prefix
            )))
            .unwrap();
        $crate::logger::LOG_TX
            .send($crate::logger::LogEvent::Flush)
            .unwrap();
    };
}
