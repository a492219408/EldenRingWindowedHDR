use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone)]
pub struct Logger {
    inner: Arc<LoggerInner>,
}

struct LoggerInner {
    file: Mutex<Option<File>>,
    started: Instant,
}

impl Logger {
    pub fn new(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|error| format!("cannot create log file {}: {error}", path.display()))?;

        Ok(Self {
            inner: Arc::new(LoggerInner {
                file: Mutex::new(Some(file)),
                started: Instant::now(),
            }),
        })
    }

    pub fn line(&self, message: impl AsRef<str>) {
        let Ok(mut file) = self.inner.file.lock() else {
            return;
        };
        let Some(file) = file.as_mut() else {
            return;
        };

        let _ = writeln!(
            file,
            "[{:>6} ms] {}",
            self.inner.started.elapsed().as_millis(),
            message.as_ref()
        );
        let _ = file.flush();
    }
}
