//! Result aggregation for multi-agent coordination.
//!
//! This module provides strategies for combining results from multiple agents
//! into a coherent output.

use std::collections::HashMap;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::agent::AgentId;

/// Entry in the result aggregator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEntry {
    /// Agent that produced the result.
    pub agent_id: AgentId,
    /// Work ID.
    pub work_id: String,
    /// Result data.
    pub data: serde_json::Value,
    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
}

/// Aggregation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationStrategy {
    /// Concatenate all results in order.
    Concatenate,
    /// Merge results (for JSON objects).
    Merge,
    /// Vote on the best result.
    Vote,
    /// Take the first result.
    First,
    /// Take the last result.
    Last,
    /// Custom aggregation.
    Custom,
}

impl std::fmt::Display for AggregationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregationStrategy::Concatenate => write!(f, "concatenate"),
            AggregationStrategy::Merge => write!(f, "merge"),
            AggregationStrategy::Vote => write!(f, "vote"),
            AggregationStrategy::First => write!(f, "first"),
            AggregationStrategy::Last => write!(f, "last"),
            AggregationStrategy::Custom => write!(f, "custom"),
        }
    }
}

/// Vote result with confidence score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResult {
    /// The winning result.
    pub winner: serde_json::Value,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Number of votes.
    pub vote_count: usize,
    /// Total votes cast.
    pub total_votes: usize,
}

/// Result aggregator for collecting and combining agent outputs.
pub struct ResultAggregator {
    /// Stored results.
    results: DashMap<String, ResultEntry>,
    /// Aggregation strategy.
    strategy: AggregationStrategy,
    /// Expected number of results (for progress tracking).
    expected_count: std::sync::atomic::AtomicUsize,
}

impl ResultAggregator {
    /// Create a new result aggregator.
    #[must_use]
    pub fn new(strategy: AggregationStrategy) -> Self {
        Self {
            results: DashMap::new(),
            strategy,
            expected_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Set the expected result count.
    pub fn set_expected_count(&self, count: usize) {
        self.expected_count.store(count, std::sync::atomic::Ordering::Relaxed);
    }

    /// Add a result to the aggregator.
    pub async fn add_result(
        &self,
        agent_id: AgentId,
        work_id: String,
        data: serde_json::Value,
    ) {
        let entry = ResultEntry {
            agent_id,
            work_id: work_id.clone(),
            data,
            timestamp: chrono::Utc::now(),
            confidence: 1.0, // Default confidence
        };

        self.results.insert(work_id, entry);
    }

    /// Add a result with confidence score.
    pub async fn add_result_with_confidence(
        &self,
        agent_id: AgentId,
        work_id: String,
        data: serde_json::Value,
        confidence: f64,
    ) {
        let entry = ResultEntry {
            agent_id,
            work_id: work_id.clone(),
            data,
            timestamp: chrono::Utc::now(),
            confidence,
        };

        self.results.insert(work_id, entry);
    }

    /// Get the count of collected results.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Check if all expected results have been received.
    pub fn is_complete(&self) -> bool {
        let expected = self.expected_count.load(std::sync::atomic::Ordering::Relaxed);
        self.results.len() >= expected
    }

    /// Get the aggregated result.
    pub async fn get_aggregated_result(&self) -> Option<serde_json::Value> {
        match self.strategy {
            AggregationStrategy::Concatenate => self.concatenate_results(),
            AggregationStrategy::Merge => self.merge_results(),
            AggregationStrategy::Vote => self.vote_results().map(|v| v.winner),
            AggregationStrategy::First => self.first_result(),
            AggregationStrategy::Last => self.last_result(),
            AggregationStrategy::Custom => None, // Custom requires external handling
        }
    }

    /// Concatenate all results.
    fn concatenate_results(&self) -> Option<serde_json::Value> {
        let entries: Vec<_> = self
            .results
            .iter()
            .map(|e| e.value().clone())
            .collect();

        if entries.is_empty() {
            return None;
        }

        // Try to concatenate as strings
        let strings: Vec<String> = entries
            .iter()
            .map(|e| e.data.to_string())
            .collect();

        Some(serde_json::Value::String(strings.join("\n")))
    }

    /// Merge JSON object results.
    fn merge_results(&self) -> Option<serde_json::Value> {
        let mut merged = serde_json::Map::new();

        for entry in self.results.iter() {
            if let Some(obj) = entry.data.as_object() {
                for (key, value) in obj {
                    merged.insert(key.clone(), value.clone());
                }
            }
        }

        if merged.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(merged))
        }
    }

    /// Vote on the best result (simple majority for now).
    fn vote_results(&self) -> Option<VoteResult> {
        let entries: Vec<_> = self
            .results
            .iter()
            .map(|e| e.value().clone())
            .collect();

        if entries.is_empty() {
            return None;
        }

        // Group by result value
        let mut votes: HashMap<String, Vec<&ResultEntry>> = HashMap::new();
        for entry in &entries {
            let key = entry.data.to_string();
            votes.entry(key).or_default().push(entry);
        }

        // Find winner
        let mut winner_key = String::new();
        let mut max_votes = 0;

        for (key, voters) in &votes {
            if voters.len() > max_votes {
                max_votes = voters.len();
                winner_key = key.clone();
            }
        }

        // Calculate confidence
        let confidence = max_votes as f64 / entries.len() as f64;

        // Get the actual winner value
        let winner = votes[&winner_key][0].data.clone();

        Some(VoteResult {
            winner,
            confidence,
            vote_count: max_votes,
            total_votes: entries.len(),
        })
    }

    /// Get the first result (by timestamp).
    fn first_result(&self) -> Option<serde_json::Value> {
        self.results
            .iter()
            .min_by(|a, b| a.timestamp.cmp(&b.timestamp))
            .map(|e| e.data.clone())
    }

    /// Get the last result (by timestamp).
    fn last_result(&self) -> Option<serde_json::Value> {
        self.results
            .iter()
            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
            .map(|e| e.data.clone())
    }

    /// Get all raw results.
    pub fn get_all_results(&self) -> Vec<ResultEntry> {
        self.results
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Clear all results.
    pub fn clear(&self) {
        self.results.clear();
        self.expected_count.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_result() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Concatenate);

        aggregator
            .add_result(AgentId::new(), "work-1".to_string(), serde_json::json!("result-1"))
            .await;

        assert_eq!(aggregator.result_count(), 1);
    }

    #[tokio::test]
    async fn test_concatenate_results() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Concatenate);

        aggregator
            .add_result(AgentId::new(), "work-1".to_string(), serde_json::json!("Hello"))
            .await;
        aggregator
            .add_result(AgentId::new(), "work-2".to_string(), serde_json::json!("World"))
            .await;

        let result = aggregator.get_aggregated_result().await;
        assert!(result.is_some());
        let binding = result.unwrap();
        let result_str = binding.as_str().unwrap();
        assert!(result_str.contains("Hello"));
        assert!(result_str.contains("World"));
    }

    #[tokio::test]
    async fn test_merge_results() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Merge);

        aggregator
            .add_result(
                AgentId::new(),
                "work-1".to_string(),
                serde_json::json!({"key1": "value1"}),
            )
            .await;
        aggregator
            .add_result(
                AgentId::new(),
                "work-2".to_string(),
                serde_json::json!({"key2": "value2"}),
            )
            .await;

        let result = aggregator.get_aggregated_result().await;
        assert!(result.is_some());

        let obj = result.unwrap().as_object().unwrap().clone();
        assert_eq!(obj["key1"], "value1");
        assert_eq!(obj["key2"], "value2");
    }

    #[tokio::test]
    async fn test_vote_results() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Vote);
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();
        let agent3 = AgentId::new();

        // Two agents agree, one disagrees
        aggregator
            .add_result(agent1, "work-1".to_string(), serde_json::json!("option-a"))
            .await;
        aggregator
            .add_result(agent2, "work-2".to_string(), serde_json::json!("option-a"))
            .await;
        aggregator
            .add_result(agent3, "work-3".to_string(), serde_json::json!("option-b"))
            .await;

        let result = aggregator.get_aggregated_result().await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "option-a");
    }

    #[tokio::test]
    async fn test_first_last_results() {
        let agent1 = AgentId::new();

        // First aggregator
        let first_agg = ResultAggregator::new(AggregationStrategy::First);
        first_agg
            .add_result(agent1, "work-1".to_string(), serde_json::json!("first"))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        first_agg
            .add_result(agent1, "work-2".to_string(), serde_json::json!("second"))
            .await;

        let first_result = first_agg.get_aggregated_result().await;
        assert_eq!(first_result.unwrap(), "first");

        // Last aggregator
        let last_agg = ResultAggregator::new(AggregationStrategy::Last);
        last_agg
            .add_result(agent1, "work-1".to_string(), serde_json::json!("first"))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        last_agg
            .add_result(agent1, "work-2".to_string(), serde_json::json!("second"))
            .await;

        let last_result = last_agg.get_aggregated_result().await;
        assert_eq!(last_result.unwrap(), "second");
    }

    #[tokio::test]
    async fn test_expected_count() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Concatenate);

        aggregator.set_expected_count(3);
        assert!(!aggregator.is_complete());

        aggregator
            .add_result(AgentId::new(), "work-1".to_string(), serde_json::json!("1"))
            .await;
        assert!(!aggregator.is_complete());

        aggregator
            .add_result(AgentId::new(), "work-2".to_string(), serde_json::json!("2"))
            .await;
        assert!(!aggregator.is_complete());

        aggregator
            .add_result(AgentId::new(), "work-3".to_string(), serde_json::json!("3"))
            .await;
        assert!(aggregator.is_complete());
    }

    #[tokio::test]
    async fn test_clear() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Concatenate);

        aggregator
            .add_result(AgentId::new(), "work-1".to_string(), serde_json::json!("result"))
            .await;

        assert_eq!(aggregator.result_count(), 1);

        aggregator.clear();

        assert_eq!(aggregator.result_count(), 0);
        assert!(aggregator.get_aggregated_result().await.is_none());
    }

    #[tokio::test]
    async fn test_vote_confidence() {
        let aggregator = ResultAggregator::new(AggregationStrategy::Vote);

        // 4 agents agree, 1 disagrees = 80% confidence
        for i in 0..4 {
            aggregator
                .add_result(
                    AgentId::new(),
                    format!("work-{}", i),
                    serde_json::json!("majority"),
                )
                .await;
        }
        aggregator
            .add_result(AgentId::new(), "work-4".to_string(), serde_json::json!("minority"))
            .await;

        let vote_result = aggregator.vote_results();
        assert!(vote_result.is_some());

        let result = vote_result.unwrap();
        assert_eq!(result.confidence, 0.8);
        assert_eq!(result.vote_count, 4);
        assert_eq!(result.total_votes, 5);
    }
}
