use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{StorageConfig, VectorBackend};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorStoreTopology {
    pub backend: String,
    pub endpoint: String,
    pub query_path: String,
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheStoreTopology {
    pub backend: String,
    pub endpoint: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryStoreTopology {
    pub backend: String,
    pub endpoint: String,
    pub schema_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryTopology {
    pub vector: VectorStoreTopology,
    pub cache: CacheStoreTopology,
    pub history: HistoryStoreTopology,
    pub notes: Vec<String>,
}

pub fn build_repository_topology(
    config: &StorageConfig,
) -> Result<RepositoryTopology, RepositoryError> {
    if config.redis_url.trim().is_empty() {
        return Err(RepositoryError::Operation(
            "storage.redis_url must not be empty".to_string(),
        ));
    }

    if config.postgres_url.trim().is_empty() {
        return Err(RepositoryError::Operation(
            "storage.postgres_url must not be empty".to_string(),
        ));
    }

    if config.vector_url.trim().is_empty() {
        return Err(RepositoryError::Operation(
            "storage.vector_url must not be empty".to_string(),
        ));
    }

    let (vector_backend, query_path, capability, vector_note) = match config.vector_backend {
        VectorBackend::Surrealdb => (
            "surrealdb",
            "/sql",
            "hybrid_context_query",
            "Use SurrealDB for low-latency semantic context retrieval close to the validator fleet.",
        ),
        VectorBackend::CloudflareVectorize => (
            "cloudflare_vectorize",
            "/indexes/query",
            "managed_edge_vector_search",
            "Use Cloudflare Vectorize when the context plane needs globally distributed edge retrieval.",
        ),
    };

    Ok(RepositoryTopology {
        vector: VectorStoreTopology {
            backend: vector_backend.to_string(),
            endpoint: config.vector_url.clone(),
            query_path: query_path.to_string(),
            capability: capability.to_string(),
        },
        cache: CacheStoreTopology {
            backend: "redis".to_string(),
            endpoint: config.redis_url.clone(),
            purpose: "rate_limits_and_score_cache".to_string(),
        },
        history: HistoryStoreTopology {
            backend: "postgres".to_string(),
            endpoint: config.postgres_url.clone(),
            schema_hint: "epoch_scores".to_string(),
        },
        notes: vec![
            vector_note.to_string(),
            "Use Redis for local scoring matrices, rate limits, and inter-process coordination."
                .to_string(),
            "Use PostgreSQL for epoch history, pruning scores, and longer-horizon analytics."
                .to_string(),
        ],
    })
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

#[cfg(test)]
mod tests {
    use super::build_repository_topology;
    use crate::config::{StorageConfig, VectorBackend};

    fn sample_storage(vector_backend: VectorBackend) -> StorageConfig {
        StorageConfig {
            vector_backend,
            redis_url: "redis://redis.hermes.svc.cluster.local:6379".to_string(),
            postgres_url: "postgres://hermes:hermes@postgres.hermes.svc.cluster.local:5432/hermes"
                .to_string(),
            vector_url: "http://vector.hermes.svc.cluster.local:8000".to_string(),
        }
    }

    #[test]
    fn builds_surrealdb_repository_topology() {
        let topology = build_repository_topology(&sample_storage(VectorBackend::Surrealdb))
            .expect("surrealdb topology should build");

        assert_eq!(topology.vector.backend, "surrealdb");
        assert_eq!(topology.vector.query_path, "/sql");
        assert_eq!(topology.cache.backend, "redis");
        assert_eq!(topology.history.backend, "postgres");
    }

    #[test]
    fn builds_cloudflare_vectorize_repository_topology() {
        let topology =
            build_repository_topology(&sample_storage(VectorBackend::CloudflareVectorize))
                .expect("cloudflare vectorize topology should build");

        assert_eq!(topology.vector.backend, "cloudflare_vectorize");
        assert_eq!(topology.vector.query_path, "/indexes/query");
        assert!(topology
            .notes
            .iter()
            .any(|note| note.contains("Cloudflare Vectorize")));
    }
}
