use std::sync::LazyLock;

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};

use crossbeam_channel::{unbounded, Receiver, Sender};
use std::thread;

pub enum LogEvent {
    Message(String),
    Flush,
    Shutdown,
}

pub static LOG_TX: LazyLock<Sender<LogEvent>> = LazyLock::new(|| {
    let (tx, rx) = unbounded();
    // 启动后台线程
    thread::spawn(|| log_writer(rx));
    tx
});

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
