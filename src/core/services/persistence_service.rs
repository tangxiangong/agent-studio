//! Persistence Service - Handles message persistence to JSONL files
//!
//! This service saves session updates to disk in JSONL format (one JSON object per line)
//! and loads historical messages when needed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::{ContentBlock, ContentChunk, SessionUpdate, TextContent};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Persisted message entry with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    /// Timestamp in ISO 8601 format
    pub timestamp: String,
    /// The session update
    pub update: SessionUpdate,
}

impl PersistedMessage {
    /// Create a new persisted message with current timestamp
    pub fn new(update: SessionUpdate) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self { timestamp, update }
    }

    /// Create from existing timestamp
    pub fn with_timestamp(timestamp: String, update: SessionUpdate) -> Self {
        Self { timestamp, update }
    }
}

/// Type of chunk being accumulated
#[derive(Debug, Clone, PartialEq)]
enum AccumulatedChunkType {
    AgentMessage,
    AgentThought,
    UserMessage,
    Empty, // Initial state, no chunks yet
}

/// Accumulates chunks for a session before flushing to disk
struct ChunkAccumulator {
    /// Timestamp of first chunk in the sequence
    first_timestamp: String,
    /// Type of chunks being accumulated
    chunk_type: AccumulatedChunkType,
    /// Accumulated chunks for AgentMessageChunk
    agent_message_chunks: Vec<ContentChunk>,
    /// Accumulated text for AgentThoughtChunk
    agent_thought_text: String,
    /// Accumulated chunks for UserMessageChunk
    user_message_chunks: Vec<ContentChunk>,
}

impl ChunkAccumulator {
    /// Create a new empty accumulator
    fn new() -> Self {
        Self {
            first_timestamp: String::new(),
            chunk_type: AccumulatedChunkType::Empty,
            agent_message_chunks: Vec::new(),
            agent_thought_text: String::new(),
            user_message_chunks: Vec::new(),
        }
    }

    /// Try to append an AgentMessageChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                // First chunk - initialize
                self.chunk_type = AccumulatedChunkType::AgentMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_message_chunks.push(chunk);
                None // No flush needed
            }
            AccumulatedChunkType::AgentMessage => {
                // Same type - accumulate
                self.agent_message_chunks.push(chunk);
                None
            }
            _ => {
                // Type changed - flush old, start new
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::AgentMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_message_chunks.push(chunk);
                Some(FlushData::Accumulated(flushed))
            }
        }
    }

    /// Try to append an AgentThoughtChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_agent_thought_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        let text = extract_text_from_content_chunk(&chunk);

        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                self.chunk_type = AccumulatedChunkType::AgentThought;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_thought_text = text;
                None
            }
            AccumulatedChunkType::AgentThought => {
                // Append text (same as ConversationPanel logic)
                self.agent_thought_text.push_str(&text);
                None
            }
            _ => {
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::AgentThought;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_thought_text = text;
                Some(FlushData::Accumulated(flushed))
            }
        }
    }

    /// Try to append a UserMessageChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_user_message_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                self.chunk_type = AccumulatedChunkType::UserMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.user_message_chunks.push(chunk);
                None
            }
            AccumulatedChunkType::UserMessage => {
                self.user_message_chunks.push(chunk);
                None
            }
            _ => {
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::UserMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.user_message_chunks.push(chunk);
                Some(FlushData::Accumulated(flushed))
            }
        }
    }

    /// Flush accumulated chunks into a SessionUpdate
    /// Returns None if nothing accumulated, Some((timestamp, update)) otherwise
    fn flush(&mut self) -> Option<(String, SessionUpdate)> {
        if matches!(self.chunk_type, AccumulatedChunkType::Empty) {
            return None; // Nothing to flush
        }

        let timestamp = self.first_timestamp.clone();
        let update = match self.chunk_type {
            AccumulatedChunkType::AgentMessage => {
                let merged_chunk = merge_text_chunks(&self.agent_message_chunks);
                SessionUpdate::AgentMessageChunk(merged_chunk)
            }
            AccumulatedChunkType::AgentThought => {
                let text_block = ContentBlock::Text(TextContent::new(self.agent_thought_text.clone()));
                SessionUpdate::AgentThoughtChunk(ContentChunk::new(text_block))
            }
            AccumulatedChunkType::UserMessage => {
                let merged_chunk = merge_text_chunks(&self.user_message_chunks);
                SessionUpdate::UserMessageChunk(merged_chunk)
            }
            AccumulatedChunkType::Empty => unreachable!(),
        };

        // Reset state
        *self = ChunkAccumulator::new();
        Some((timestamp, update))
    }
}

/// Data to be flushed to disk
enum FlushData {
    /// Only accumulated data to write
    Accumulated(Option<(String, SessionUpdate)>),
    /// Both accumulated data and a new non-chunk update
    Both(Option<(String, SessionUpdate)>, SessionUpdate),
}

/// Merge multiple ContentChunks into a single ContentChunk
/// For text chunks, concatenates all text into one chunk
fn merge_text_chunks(chunks: &[ContentChunk]) -> ContentChunk {
    if chunks.is_empty() {
        return ContentChunk::new(ContentBlock::Text(TextContent::new(String::new())));
    }

    if chunks.len() == 1 {
        return chunks[0].clone();
    }

    // Check if all chunks are text
    let all_text = chunks.iter().all(|chunk| matches!(chunk.content, ContentBlock::Text(_)));

    if all_text {
        // Concatenate all text
        let merged_text = chunks
            .iter()
            .filter_map(|chunk| {
                if let ContentBlock::Text(text) = &chunk.content {
                    Some(text.text.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();

        let text_block = ContentBlock::Text(TextContent::new(merged_text));
        let mut merged_chunk = ContentChunk::new(text_block);

        // Preserve meta from first chunk if any
        if let Some(meta) = &chunks[0].meta {
            merged_chunk.meta = Some(meta.clone());
        }

        return merged_chunk;
    }

    // Mixed content (text + images) - should not happen in practice
    // but handle defensively by returning first chunk
    log::warn!(
        "Attempted to merge heterogeneous chunks, returning first chunk only"
    );
    chunks[0].clone()
}

/// Extract text from ContentChunk (for AgentThoughtChunk)
fn extract_text_from_content_chunk(chunk: &ContentChunk) -> String {
    match &chunk.content {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(img) => format!("[Image: {}]", img.mime_type),
        _ => "[Non-text content]".to_string(),
    }
}

/// Message persistence service
pub struct PersistenceService {
    /// Base directory for session files
    base_dir: PathBuf,
    /// Thread-safe storage for chunk accumulators per session
    accumulators: Arc<Mutex<HashMap<String, ChunkAccumulator>>>,
}

impl PersistenceService {
    /// Create a new persistence service
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for storing session files (e.g., "target/sessions")
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            accumulators: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the file path for a session
    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.jsonl", session_id))
    }

    /// Ensure the base directory exists
    fn ensure_base_dir_sync(&self) -> Result<()> {
        if !self.base_dir.exists() {
            std::fs::create_dir_all(&self.base_dir).context("Failed to create base directory")?;
        }
        Ok(())
    }

    /// Save a session update to disk
    ///
    /// Accumulates chunk updates in memory and flushes when needed.
    /// Non-chunk updates trigger immediate flush and write.
    pub async fn save_update(&self, session_id: &str, update: SessionUpdate) -> Result<()> {
        let flush_data = {
            let mut accumulators = self.accumulators.lock().unwrap();
            let accumulator = accumulators
                .entry(session_id.to_string())
                .or_insert_with(ChunkAccumulator::new);

            match update {
                SessionUpdate::AgentMessageChunk(chunk) => {
                    log::debug!("Accumulating AgentMessageChunk for session: {}", session_id);
                    accumulator.try_append_agent_message_chunk(chunk)
                }
                SessionUpdate::AgentThoughtChunk(chunk) => {
                    log::debug!("Accumulating AgentThoughtChunk for session: {}", session_id);
                    accumulator.try_append_agent_thought_chunk(chunk)
                }
                SessionUpdate::UserMessageChunk(chunk) => {
                    log::debug!("Accumulating UserMessageChunk for session: {}", session_id);
                    accumulator.try_append_user_message_chunk(chunk)
                }
                _ => {
                    // Non-chunk update: flush accumulator, then write both
                    log::debug!(
                        "Non-chunk update received, flushing accumulator for session: {}",
                        session_id
                    );
                    let flushed = accumulator.flush();
                    Some(FlushData::Both(flushed, update))
                }
            }
        }; // Lock released here

        // Write outside lock to avoid blocking
        if let Some(data) = flush_data {
            self.write_flush_data(session_id, data).await?;
        }

        Ok(())
    }

    /// Write flush data to disk
    async fn write_flush_data(&self, session_id: &str, data: FlushData) -> Result<()> {
        match data {
            FlushData::Accumulated(Some((timestamp, update))) => {
                // Write only accumulated data
                self.write_with_timestamp(session_id, update, timestamp)
                    .await?;
            }
            FlushData::Both(accumulated, non_chunk) => {
                // Write accumulated data first (if any)
                if let Some((timestamp, update)) = accumulated {
                    self.write_with_timestamp(session_id, update, timestamp)
                        .await?;
                }
                // Then write non-chunk update with current timestamp
                self.write_with_current_timestamp(session_id, non_chunk)
                    .await?;
            }
            FlushData::Accumulated(None) => {
                // Nothing to write
            }
        }
        Ok(())
    }

    /// Write update with specific timestamp
    async fn write_with_timestamp(
        &self,
        session_id: &str,
        update: SessionUpdate,
        timestamp: String,
    ) -> Result<()> {
        let file_path = self.session_file_path(session_id);
        let base_dir = self.base_dir.clone();
        let message = PersistedMessage::with_timestamp(timestamp, update);

        smol::unblock(move || {
            // Ensure directory exists
            if !base_dir.exists() {
                std::fs::create_dir_all(&base_dir).context("Failed to create base directory")?;
            }

            // Serialize to JSON and append newline
            let json = serde_json::to_string(&message).context("Failed to serialize message")?;

            // Open file in append mode
            use std::fs::OpenOptions;
            use std::io::Write;

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .context("Failed to open session file")?;

            // Write JSON line
            write!(file, "{}\n", json).context("Failed to write message")?;

            log::debug!(
                "Wrote merged message to session file: {}",
                file_path.display()
            );
            Ok(())
        })
        .await
    }

    /// Write update with current timestamp
    async fn write_with_current_timestamp(
        &self,
        session_id: &str,
        update: SessionUpdate,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        self.write_with_timestamp(session_id, update, timestamp).await
    }

    /// Flush accumulated chunks for a specific session
    ///
    /// This should be called when a session completes or becomes idle
    pub async fn flush_session(&self, session_id: &str) -> Result<()> {
        let flush_data = {
            let mut accumulators = self.accumulators.lock().unwrap();
            accumulators.get_mut(session_id).and_then(|acc| acc.flush())
        };

        if let Some((timestamp, update)) = flush_data {
            log::info!("Flushing accumulated chunks for session: {}", session_id);
            self.write_with_timestamp(session_id, update, timestamp)
                .await?;
        } else {
            log::debug!(
                "No accumulated chunks to flush for session: {}",
                session_id
            );
        }

        Ok(())
    }

    /// Load all messages for a session
    ///
    /// Returns messages in chronological order
    pub async fn load_messages(&self, session_id: &str) -> Result<Vec<PersistedMessage>> {
        let file_path = self.session_file_path(session_id);
        let session_id = session_id.to_string(); // Clone for the closure

        // Use smol::unblock to run blocking I/O
        smol::unblock(move || {
            // Check if file exists
            if !file_path.exists() {
                log::debug!("No history file found for session: {}", session_id);
                return Ok(Vec::new());
            }

            use std::fs::File;
            use std::io::{BufRead, BufReader};

            let file = File::open(&file_path).context("Failed to open session file")?;

            let reader = BufReader::new(file);
            let mut messages = Vec::new();

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<PersistedMessage>(&line) {
                    Ok(message) => messages.push(message),
                    Err(e) => {
                        log::warn!("Failed to parse line in session file: {}", e);
                        // Continue reading other lines
                    }
                }
            }

            log::info!(
                "Loaded {} messages from session file: {}",
                messages.len(),
                file_path.display()
            );
            Ok(messages)
        })
        .await
    }

    /// Delete a session's history file
    ///
    /// Flushes any pending chunks before deleting
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        // Flush pending chunks first
        self.flush_session(session_id).await?;

        // Remove accumulator
        {
            let mut accumulators = self.accumulators.lock().unwrap();
            accumulators.remove(session_id);
        }

        // Delete file
        let file_path = self.session_file_path(session_id);

        smol::unblock(move || {
            if file_path.exists() {
                std::fs::remove_file(&file_path).context("Failed to delete session file")?;
                log::info!("Deleted session file: {}", file_path.display());
            }
            Ok(())
        })
        .await
    }

    /// List all available sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let base_dir = self.base_dir.clone();

        smol::unblock(move || {
            if !base_dir.exists() {
                return Ok(Vec::new());
            }

            let mut sessions = Vec::new();

            for entry in
                std::fs::read_dir(&base_dir).context("Failed to read sessions directory")?
            {
                let entry = entry?;
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension == "jsonl" {
                        if let Some(stem) = path.file_stem() {
                            if let Some(session_id) = stem.to_str() {
                                sessions.push(session_id.to_string());
                            }
                        }
                    }
                }
            }

            Ok(sessions)
        })
        .await
    }
}
