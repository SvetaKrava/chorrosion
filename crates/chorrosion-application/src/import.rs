//! File import service with fingerprint generation.
//!
//! This module handles importing audio files into the library, including:
//! - Reading file metadata (duration, size, codec, etc.)
//! - Generating Chromaprint fingerprints for matching
//! - Creating TrackFile entities with fingerprint data
//!
//! Note: This service creates TrackFile entities but does not persist them.
//! The caller is responsible for saving entities via the TrackFileRepository.

use chorrosion_domain::{TrackFile, TrackId};
use chorrosion_fingerprint::{AcoustidClient, FingerprintGenerator};
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing::Instrument as _;

/// Errors that can occur during file import.
#[derive(Debug, Error)]
pub enum ImportError {
    /// Failed to read file metadata
    #[error("Failed to read file metadata: {0}")]
    MetadataError(String),

    /// Failed to generate fingerprint
    #[error("Failed to generate fingerprint: {0}")]
    FingerprintError(String),

    /// Failed to persist to database
    #[error("Failed to persist to database: {0}")]
    DatabaseError(String),

    /// File does not exist
    #[error("File does not exist: {0}")]
    FileNotFound(String),

    /// Spawned import task panicked or was cancelled by the runtime
    #[error("Import task failed unexpectedly: {0}")]
    TaskFailed(String),
}

/// Result type for import operations.
pub type ImportResult<T> = Result<T, ImportError>;

/// Information about an imported file.
#[derive(Debug, Clone)]
pub struct ImportedFile {
    /// The created or updated TrackFile entity
    pub track_file: TrackFile,

    /// Whether the file was newly created (true) or updated (false)
    pub was_created: bool,

    /// Whether a fingerprint was successfully generated
    pub has_fingerprint: bool,
}

/// Service for importing audio files with fingerprint generation.
#[derive(Clone)]
pub struct FileImportService {
    /// AcoustID client for fingerprint generation
    #[allow(dead_code)]
    acoustid_client: Arc<AcoustidClient>,
    /// Maximum number of files processed concurrently in a batch import.
    /// Validated to be >= 1 at construction time.
    max_concurrent_imports: usize,
}

impl FileImportService {
    /// Create a new file import service.
    ///
    /// # Panics
    /// Panics if `max_concurrent_imports` is 0.
    pub fn new(acoustid_client: Arc<AcoustidClient>, max_concurrent_imports: usize) -> Self {
        assert!(
            max_concurrent_imports >= 1,
            "max_concurrent_imports must be >= 1"
        );
        Self {
            acoustid_client,
            max_concurrent_imports,
        }
    }

    /// Import a single audio file, generating its fingerprint.
    ///
    /// This method:
    /// 1. Validates the file path exists
    /// 2. Reads file metadata (size, duration, etc.)
    /// 3. Generates a Chromaprint fingerprint
    /// 4. Creates a TrackFile entity with the fingerprint data
    ///
    /// # Arguments
    /// * `path` - Path to the audio file to import
    /// * `track_id` - The track this file belongs to
    ///
    /// # Returns
    /// An ImportedFile containing the TrackFile entity and import metadata
    #[tracing::instrument(skip(self), fields(path = %path.as_ref().display()))]
    pub async fn import_file(
        &self,
        path: impl AsRef<Path>,
        track_id: TrackId,
    ) -> ImportResult<ImportedFile> {
        let path = path.as_ref();

        // Validate file exists and read metadata without blocking the async runtime.
        let metadata = tokio::fs::metadata(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ImportError::FileNotFound(path.display().to_string())
            } else {
                ImportError::MetadataError(e.to_string())
            }
        })?;

        let size_bytes = metadata.len();

        tracing::debug!(size_bytes, "Read file metadata");

        // Create initial TrackFile entity
        let mut track_file = TrackFile::new(track_id, path.display().to_string(), size_bytes);

        let mut has_fingerprint = false;

        // Generate fingerprint
        match self.generate_fingerprint(path).await {
            Ok((hash, duration)) => {
                track_file.fingerprint_hash = Some(hash);
                track_file.fingerprint_duration = Some(duration);
                track_file.fingerprint_computed_at = Some(Utc::now());
                has_fingerprint = true;

                tracing::info!(
                    duration_seconds = duration,
                    "Successfully generated fingerprint"
                );
            }
            Err(e) => {
                // Log error but don't fail the import - fingerprint is optional
                tracing::warn!(
                    error = %e,
                    "Failed to generate fingerprint, continuing without it"
                );
            }
        }

        Ok(ImportedFile {
            track_file,
            was_created: true,
            has_fingerprint,
        })
    }

    /// Import multiple files in batch, processing up to `max_concurrent_imports` concurrently.
    ///
    /// Permits are acquired *before* spawning each task so the number of live Tokio tasks is
    /// bounded to `max_concurrent_imports` at any point in time, avoiding the overhead of
    /// parking a large number of idle tasks for very large batches.
    ///
    /// Spawned tasks are instrumented with the caller's tracing span so per-file log lines
    /// remain correlated with the batch.
    ///
    /// # Arguments
    /// * `files` - Collection of (path, track_id) tuples to import
    ///
    /// # Returns
    /// A tuple of (successes, failures) where successes are ImportedFile entries
    /// and failures are (path, error) tuples.  The sum `successes + failures` always
    /// equals `files.len()`.
    #[tracing::instrument(skip(self, files), fields(count = files.len()))]
    pub async fn import_batch(
        &self,
        files: Vec<(String, TrackId)>,
    ) -> (Vec<ImportedFile>, Vec<(String, ImportError)>) {
        use tokio::sync::Semaphore;
        use tokio::task::JoinSet;

        let total = files.len();
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent_imports));
        // Task return type always includes the path so join errors can be attributed.
        let mut set: JoinSet<(String, Result<ImportedFile, ImportError>)> = JoinSet::new();

        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for (path, track_id) in files {
            // Acquire the permit *before* spawning so we only create tasks when capacity
            // is available, keeping the number of in-flight Tokio tasks bounded.
            // The semaphore is created locally and never explicitly closed, so acquire_owned()
            // is infallible here.
            let permit = Arc::clone(&semaphore)
                .acquire_owned()
                .await
                .expect("import semaphore closed unexpectedly");

            let service = self.clone();
            // Propagate the current span into the spawned task so per-file logs are
            // correlated with the batch span.
            let span = tracing::Span::current();
            set.spawn(
                async move {
                    let _permit = permit;
                    let result = service.import_file(&path, track_id).await;
                    (path, result)
                }
                .instrument(span),
            );
        }

        loop {
            let joined = set.join_next().await;
            let Some(result) = joined else {
                break;
            };
            match result {
                Ok((_path, Ok(imported))) => successes.push(imported),
                Ok((path, Err(error))) => failures.push((path, error)),
                Err(join_err) => {
                    // Task panicked or was cancelled by the runtime; when a task panics,
                    // JoinSet::join_next() returns Err(JoinError) and the task's return
                    // value (which held the path) is lost.  We still record a failure entry
                    // to preserve the `successes + failures == total` invariant.
                    tracing::warn!(error = %join_err, "import task panicked unexpectedly");
                    failures.push((
                        "<unknown>".to_string(),
                        ImportError::TaskFailed(join_err.to_string()),
                    ));
                }
            }
        }

        tracing::info!(
            successes = successes.len(),
            failures = failures.len(),
            total,
            "Batch import completed"
        );

        (successes, failures)
    }

    /// Generate a Chromaprint fingerprint for an audio file.
    ///
    /// # Returns
    /// A tuple of (fingerprint_hash, duration_seconds)
    async fn generate_fingerprint(&self, path: &Path) -> ImportResult<(String, u32)> {
        tracing::debug!(path = %path.display(), "Generating fingerprint");

        let generator = FingerprintGenerator::new();

        generator
            .generate_from_file(path)
            .await
            .map(|fp| (fp.hash, fp.duration))
            .map_err(|e| ImportError::FingerprintError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> FileImportService {
        let client = AcoustidClient::new("test_key".to_string()).expect("client creation");
        FileImportService::new(Arc::new(client), 8)
    }

    #[tokio::test]
    async fn test_import_nonexistent_file_fails() {
        let service = create_test_service();
        let track_id = TrackId::new();

        let result = service.import_file("nonexistent_file.mp3", track_id).await;

        assert!(matches!(result, Err(ImportError::FileNotFound(_))));
    }

    #[tokio::test]
    async fn test_import_creates_track_file() {
        let service = create_test_service();
        let track_id = TrackId::new();

        // Use the Cargo.toml file as a test file (guaranteed to exist)
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");

        let result = service.import_file(&test_file, track_id).await;

        assert!(result.is_ok());
        let imported = result.unwrap();
        assert_eq!(imported.track_file.track_id, track_id);
        assert_eq!(imported.track_file.path, test_file.display().to_string());
        assert!(imported.track_file.size_bytes > 0);
        assert!(imported.was_created);
        // Fingerprint will be None since we can't generate for non-audio files
        assert!(!imported.has_fingerprint);
        assert!(imported.track_file.fingerprint_hash.is_none());
    }

    #[tokio::test]
    async fn test_batch_import_handles_mixed_results() {
        let service = create_test_service();

        let test_file = std::env::current_dir()
            .unwrap()
            .join("Cargo.toml")
            .display()
            .to_string();

        let files = vec![
            (test_file.clone(), TrackId::new()),
            ("nonexistent.mp3".to_string(), TrackId::new()),
            (test_file, TrackId::new()),
        ];

        let (successes, failures) = service.import_batch(files).await;

        assert_eq!(successes.len(), 2);
        assert_eq!(failures.len(), 1);
        assert!(matches!(failures[0].1, ImportError::FileNotFound(_)));
    }

    #[tokio::test]
    async fn test_batch_import_total_equals_successes_plus_failures() {
        let service = create_test_service();

        let test_file = std::env::current_dir()
            .unwrap()
            .join("Cargo.toml")
            .display()
            .to_string();

        let files: Vec<(String, TrackId)> = (0..5)
            .map(|i| {
                if i % 2 == 0 {
                    (test_file.clone(), TrackId::new())
                } else {
                    (format!("nonexistent_{}.mp3", i), TrackId::new())
                }
            })
            .collect();
        let total = files.len();

        let (successes, failures) = service.import_batch(files).await;

        assert_eq!(
            successes.len() + failures.len(),
            total,
            "successes + failures must equal total"
        );
    }

    #[test]
    #[should_panic(expected = "max_concurrent_imports must be >= 1")]
    fn test_zero_concurrency_panics() {
        let client = AcoustidClient::new("test_key".to_string()).expect("client creation");
        FileImportService::new(Arc::new(client), 0);
    }
}
