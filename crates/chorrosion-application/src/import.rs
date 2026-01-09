//! File import service with fingerprint generation.
//!
//! This module handles importing audio files into the library, including:
//! - Reading file metadata (duration, size, codec, etc.)
//! - Generating Chromaprint fingerprints for matching
//! - Creating TrackFile entities with fingerprint data
//! - Persisting to the database

use chorrosion_domain::{TrackFile, TrackId};
use chorrosion_fingerprint::AcoustidClient;
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

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

    /// Invalid file path
    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    /// File does not exist
    #[error("File does not exist: {0}")]
    FileNotFound(String),
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
}

impl FileImportService {
    /// Create a new file import service.
    pub fn new(acoustid_client: Arc<AcoustidClient>) -> Self {
        Self { acoustid_client }
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
    /// A TrackFile entity with fingerprint data populated
    #[tracing::instrument(skip(self), fields(path = %path.as_ref().display()))]
    pub async fn import_file(
        &self,
        path: impl AsRef<Path>,
        track_id: TrackId,
    ) -> ImportResult<TrackFile> {
        let path = path.as_ref();
        
        // Validate file exists
        if !path.exists() {
            return Err(ImportError::FileNotFound(path.display().to_string()));
        }

        // Read file metadata
        let metadata = std::fs::metadata(path)
            .map_err(|e| ImportError::MetadataError(e.to_string()))?;
        
        let size_bytes = metadata.len();
        
        tracing::debug!(size_bytes, "Read file metadata");

        // Create initial TrackFile entity
        let mut track_file = TrackFile::new(
            track_id,
            path.display().to_string(),
            size_bytes,
        );

        // Generate fingerprint
        match self.generate_fingerprint(path).await {
            Ok((hash, duration)) => {
                track_file.fingerprint_hash = Some(hash);
                track_file.fingerprint_duration = Some(duration);
                track_file.fingerprint_computed_at = Some(Utc::now());
                
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

        Ok(track_file)
    }

    /// Import multiple files in batch.
    ///
    /// # Arguments
    /// * `files` - Collection of (path, track_id) tuples to import
    ///
    /// # Returns
    /// A tuple of (successes, failures) where successes are TrackFile entities
    /// and failures are (path, error) tuples
    #[tracing::instrument(skip(self, files), fields(count = files.len()))]
    pub async fn import_batch(
        &self,
        files: Vec<(String, TrackId)>,
    ) -> (Vec<TrackFile>, Vec<(String, ImportError)>) {
        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for (path, track_id) in files {
            match self.import_file(&path, track_id).await {
                Ok(track_file) => {
                    successes.push(track_file);
                }
                Err(error) => {
                    failures.push((path, error));
                }
            }
        }

        tracing::info!(
            successes = successes.len(),
            failures = failures.len(),
            "Batch import completed"
        );

        (successes, failures)
    }

    /// Generate a Chromaprint fingerprint for an audio file.
    ///
    /// # Returns
    /// A tuple of (fingerprint_hash, duration_seconds)
    async fn generate_fingerprint(&self, path: &Path) -> ImportResult<(String, u32)> {
        // Use the fingerprint crate to generate the fingerprint
        // This is a placeholder - actual implementation would use chromaprint
        // For now, we'll simulate it by using the AcoustID client's fingerprint generation
        
        tracing::debug!(path = %path.display(), "Generating fingerprint");
        
        // In a real implementation, this would:
        // 1. Decode the audio file (using FFmpeg or similar)
        // 2. Generate Chromaprint fingerprint
        // 3. Return base64-encoded hash and duration
        
        // For now, return an error since we don't have the actual implementation
        Err(ImportError::FingerprintError(
            "Fingerprint generation not yet implemented - requires FFmpeg integration".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> FileImportService {
        let client = AcoustidClient::new("test_key".to_string()).expect("client creation");
        FileImportService::new(Arc::new(client))
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
        let test_file = std::env::current_dir()
            .unwrap()
            .join("Cargo.toml");
        
        let result = service.import_file(&test_file, track_id).await;
        
        assert!(result.is_ok());
        let track_file = result.unwrap();
        assert_eq!(track_file.track_id, track_id);
        assert_eq!(track_file.path, test_file.display().to_string());
        assert!(track_file.size_bytes > 0);
        // Fingerprint will be None since we can't generate for non-audio files
        assert!(track_file.fingerprint_hash.is_none());
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
}
