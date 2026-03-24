use std::{path::Path, time::Duration};

use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncSeekExt, BufReader},
    time::sleep,
};

use crate::logging::log;

pub async fn wait_for_log_file(path: &Path) {
    loop {
        if tokio::fs::metadata(path).await.is_ok() {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub struct LogReader {
    path: std::path::PathBuf,
    reader: BufReader<File>,
}

impl LogReader {
    pub async fn open(path: std::path::PathBuf) -> Result<Self, String> {
        let file = File::open(&path)
            .await
            .map_err(|e| format!("failed to open log file: {e}"))?;
        let mut reader = BufReader::new(file);

        reader
            .seek(std::io::SeekFrom::Start(0))
            .await
            .map_err(|e| format!("failed to seek log file: {e}"))?;

        Ok(Self { path, reader })
    }

    pub async fn read_next_line(&mut self) -> Result<Option<String>, String> {
        let mut line = String::new();

        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("failed reading log file: {e}"))?;

        if bytes_read == 0 {
            if tokio::fs::metadata(&self.path).await.is_err() {
                log("Log file disappeared, waiting for recreation");
                wait_for_log_file(&self.path).await;

                let file = File::open(&self.path)
                    .await
                    .map_err(|e| format!("failed to reopen log file: {e}"))?;
                self.reader = BufReader::new(file);
            } else {
                sleep(Duration::from_secs(1)).await;
            }

            return Ok(None);
        }

        Ok(Some(
            line.trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string(),
        ))
    }
}
