mod file;

pub use file::*;

// -----------------------------------------------------------------------------
// Interface

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::error::Error;

use thiserror::Error;

use crate::BoxedFuture;
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// LogEntry

#[derive(Debug)]
pub enum LogEntry {
    BeginProcessing(AssetPath<'static>),
    EndProcessing(AssetPath<'static>),
    UnrecoverableError,
}

// -----------------------------------------------------------------------------
// TransactionLogFactory

pub trait TransactionLogFactory: Send + Sync + 'static {
    fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, TransactionError>>;

    fn new_log(&self) -> BoxedFuture<'_, Result<Box<dyn TransactionLog>, TransactionError>>;
}

// -----------------------------------------------------------------------------
// TransactionLog

pub trait TransactionLog: Send + Sync + 'static {
    fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), TransactionError>>;
    fn begin<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), TransactionError>>;
    fn end<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), TransactionError>>;
}

// -----------------------------------------------------------------------------
// Errors

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TransactionError {
    #[error("Encountered an invalid log line: '{0}'")]
    InvalidLine(Box<str>),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] Box<dyn Error + Send + 'static>),
}

#[derive(Error, Debug)]
pub enum LogEntryError {
    /// A duplicate process asset transaction occurred for the given asset path.
    #[error("Encountered a duplicate process asset transaction: {0}")]
    DuplicateTransaction(AssetPath<'static>),
    /// A transaction was ended that never started for the given asset path.
    #[error("A transaction was ended that never started {0}")]
    EndedMissingTransaction(AssetPath<'static>),
    /// An asset started processing but never finished at the given asset path.
    #[error("An asset started processing but never finished: {0}")]
    UnfinishedTransaction(AssetPath<'static>),
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ValidateLogError {
    /// An error that could not be recovered from. All assets will be reprocessed.
    #[error("Encountered an unrecoverable error. All assets will be reprocessed.")]
    UnrecoverableError,
    /// A [`ReadLogError`].
    #[error("Failed to read transcation log: {0}")]
    TransactionError(#[from] TransactionError),
    /// Duplicated process asset transactions occurred.
    #[error("Encountered a duplicate process asset transaction: {0:?}")]
    LogEntryErrors(Vec<LogEntryError>),
}

impl From<std::io::Error> for ValidateLogError {
    fn from(value: std::io::Error) -> Self {
        Self::TransactionError(TransactionError::Io(value))
    }
}

// -----------------------------------------------------------------------------
// Validate

pub async fn validate_transaction_log(
    log_factory: &dyn TransactionLogFactory,
) -> Result<(), ValidateLogError> {
    use voker_utils::hash::HashSet;

    let mut transactions: HashSet<AssetPath<'static>> = HashSet::new();
    let mut errors: Vec<LogEntryError> = Vec::new();
    let entries = log_factory.read().await?;

    for entry in entries {
        match entry {
            LogEntry::BeginProcessing(path) => {
                // There should never be duplicate "start transactions" in a log
                // Every start should be followed by:
                //    * nothing (if there was an abrupt stop)
                //    * an End (if the transaction was completed)
                if !transactions.insert(path.clone()) {
                    errors.push(LogEntryError::DuplicateTransaction(path));
                }
            }
            LogEntry::EndProcessing(path) => {
                if !transactions.remove(&path) {
                    errors.push(LogEntryError::EndedMissingTransaction(path));
                }
            }
            LogEntry::UnrecoverableError => return Err(ValidateLogError::UnrecoverableError),
        }
    }

    for transaction in transactions {
        errors.push(LogEntryError::UnfinishedTransaction(transaction));
    }

    if !errors.is_empty() {
        return Err(ValidateLogError::LogEntryErrors(errors));
    }

    Ok(())
}
