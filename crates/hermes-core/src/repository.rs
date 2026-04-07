use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextRecord {
    pub id: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreEvent {
    pub validator_id: String,
    pub subnet_id: u16,
    pub epoch: u64,
    pub reward_share: f64,
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("repository operation failed: {0}")]
    Operation(String),
}

pub trait VectorStore {
    fn backend_name(&self) -> &'static str;
    fn upsert(&mut self, records: Vec<ContextRecord>) -> Result<(), RepositoryError>;
    fn query(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<ContextRecord>, RepositoryError>;
}

pub trait CacheStore {
    fn put(&mut self, key: String, value: String) -> Result<(), RepositoryError>;
    fn get(&self, key: &str) -> Result<Option<String>, RepositoryError>;
}

pub trait HistoryStore {
    fn append(&mut self, event: ScoreEvent) -> Result<(), RepositoryError>;
    fn recent(&self, limit: usize) -> Result<Vec<ScoreEvent>, RepositoryError>;
}

#[derive(Debug, Default)]
pub struct InMemoryVectorStore {
    records: Vec<ContextRecord>,
}

impl VectorStore for InMemoryVectorStore {
    fn backend_name(&self) -> &'static str {
        "in_memory"
    }

    fn upsert(&mut self, records: Vec<ContextRecord>) -> Result<(), RepositoryError> {
        for record in records {
            self.records.retain(|existing| existing.id != record.id);
            self.records.push(record);
        }

        Ok(())
    }

    fn query(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<ContextRecord>, RepositoryError> {
        let mut scored = self
            .records
            .iter()
            .filter(|record| record.embedding.len() == embedding.len())
            .map(|record| {
                let score = record
                    .embedding
                    .iter()
                    .zip(embedding.iter())
                    .map(|(lhs, rhs)| lhs * rhs)
                    .sum::<f32>();
                (score, record.clone())
            })
            .collect::<Vec<_>>();

        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        Ok(scored.into_iter().take(limit).map(|(_, record)| record).collect())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryCacheStore {
    data: HashMap<String, String>,
}

impl CacheStore for InMemoryCacheStore {
    fn put(&mut self, key: String, value: String) -> Result<(), RepositoryError> {
        self.data.insert(key, value);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, RepositoryError> {
        Ok(self.data.get(key).cloned())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryHistoryStore {
    events: Vec<ScoreEvent>,
}

impl HistoryStore for InMemoryHistoryStore {
    fn append(&mut self, event: ScoreEvent) -> Result<(), RepositoryError> {
        self.events.push(event);
        Ok(())
    }

    fn recent(&self, limit: usize) -> Result<Vec<ScoreEvent>, RepositoryError> {
        let start = self.events.len().saturating_sub(limit);
        Ok(self.events[start..].to_vec())
    }
}
