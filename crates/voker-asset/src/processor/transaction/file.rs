use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use std::path::PathBuf;

use async_fs::File;
use futures_lite::{AsyncReadExt, AsyncWriteExt};

use crate::BoxedFuture;
use crate::path::AssetPath;

use super::{LogEntry, TransactionError, TransactionLog, TransactionLogFactory};

const LOG_PATH: &str = "imported_assets/log";
const ENTRY_BEGIN: &str = "Begin ";
const ENTRY_END: &str = "End ";
const UNRECOVERABLE_ERROR: &str = "UnrecoverableError";

pub struct FileTransactionLogFactory {
    /// The file path that the transaction log should write to.
    pub file_path: PathBuf,
}

impl Default for FileTransactionLogFactory {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let base_path = crate::io::file::base_path();
        #[cfg(target_arch = "wasm32")]
        let base_path = PathBuf::new();
        let file_path = base_path.join(LOG_PATH);
        Self { file_path }
    }
}

impl TransactionLogFactory for FileTransactionLogFactory {
    fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, TransactionError>> {
        let path = self.file_path.clone();
        Box::pin(async move {
            let mut log_lines = Vec::new();
            let mut file = match File::open(path).await {
                Ok(file) => file,
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        // if the log file doesn't exist, this is equivalent to an empty file
                        return Ok(log_lines);
                    }
                    return Err(err.into());
                }
            };
            let mut string = String::new();
            file.read_to_string(&mut string).await?;

            for line in string.lines() {
                if let Some(path_str) = line.strip_prefix(ENTRY_BEGIN) {
                    log_lines.push(LogEntry::BeginProcessing(
                        AssetPath::parse(path_str).into_owned(),
                    ));
                } else if let Some(path_str) = line.strip_prefix(ENTRY_END) {
                    log_lines.push(LogEntry::EndProcessing(
                        AssetPath::parse(path_str).into_owned(),
                    ));
                } else if line.is_empty() {
                    continue;
                } else {
                    return Err(TransactionError::InvalidLine(line.into()));
                }
            }
            Ok(log_lines)
        })
    }

    fn new_log(&self) -> BoxedFuture<'_, Result<Box<dyn TransactionLog>, TransactionError>> {
        let path = self.file_path.clone();
        Box::pin(async move {
            match async_fs::remove_file(&path).await {
                Ok(_) => { /* successfully removed file */ }
                Err(err) => {
                    // if the log file is not found, we assume we are starting in a fresh (or good) state
                    if err.kind() != std::io::ErrorKind::NotFound {
                        tracing::error!("Failed to remove previous log file {}", err);
                    }
                }
            }

            if let Some(parent_folder) = path.parent() {
                async_fs::create_dir_all(parent_folder).await?;
            }

            let log_file = File::create(path).await?;
            Ok(Box::new(FileTransactionLog { log_file }) as Box<dyn TransactionLog>)
        })
    }
}

struct FileTransactionLog {
    /// The file to write logs to.
    log_file: File,
}

impl FileTransactionLog {
    /// Write `line` to the file and flush it.
    async fn write(&mut self, line: &str) -> Result<(), TransactionError> {
        self.log_file.write_all(line.as_bytes()).await?;
        self.log_file.flush().await?;
        Ok(())
    }
}

impl TransactionLog for FileTransactionLog {
    fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), TransactionError>> {
        Box::pin(async move { self.write(UNRECOVERABLE_ERROR).await })
    }

    fn begin<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), TransactionError>> {
        Box::pin(async move { self.write(&alloc::format!("{ENTRY_BEGIN}{asset}\n")).await })
    }

    fn end<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), TransactionError>> {
        Box::pin(async move { self.write(&alloc::format!("{ENTRY_END}{asset}\n")).await })
    }
}
