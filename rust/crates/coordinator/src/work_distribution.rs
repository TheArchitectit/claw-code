//! Work distribution strategies for multi-agent systems.
//!
//! This module provides algorithms for distributing work units among agents
//! based on various strategies.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::agent::{AgentId, WorkAssignment};

/// A unit of work to be distributed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkUnit {
    /// Unique work ID.
    pub id: String,
    /// Work description.
    pub description: String,
    /// Context data.
    pub context: serde_json::Value,
    /// Priority (1-10, 10 being highest).
    pub priority: u8,
    /// Estimated complexity (1-10).
    pub complexity: u8,
    /// Dependencies (work IDs that must complete first).
    pub dependencies: Vec<String>,
    /// Deadline for completion.
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    /// Required capabilities.
    pub required_capabilities: Vec<String>,
}

impl WorkUnit {
    /// Create a new work unit.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            context: serde_json::Value::Object(serde_json::Map::new()),
            priority: 5,
            complexity: 5,
            dependencies: Vec::new(),
            deadline: None,
            required_capabilities: Vec::new(),
        }
    }

    /// Set priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.clamp(1, 10);
        self
    }

    /// Set complexity.
    #[must_use]
    pub fn with_complexity(mut self, complexity: u8) -> Self {
        self.complexity = complexity.clamp(1, 10);
        self
    }

    /// Set context.
    #[must_use]
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    /// Add dependency.
    #[must_use]
    pub fn with_dependency(mut self, work_id: impl Into<String>) -> Self {
        self.dependencies.push(work_id.into());
        self
    }
}

/// Task partition for divide-and-conquer strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPartition {
    /// Partition ID.
    pub partition_id: String,
    /// Parent task ID.
    pub parent_id: String,
    /// Index in the partition sequence.
    pub index: usize,
    /// Total partitions.
    pub total: usize,
    /// Partition data.
    pub data: serde_json::Value,
}

/// Distribution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistributionStrategy {
    /// Round-robin distribution.
    RoundRobin,
    /// Send to least busy agent.
    LeastBusy,
    /// Send to agent with required capabilities.
    CapabilityMatch,
    /// Distribute based on load balancing.
    LoadBalanced,
    /// Divide work into partitions.
    Partitioned,
    /// Priority-based distribution.
    Priority,
}

impl std::fmt::Display for DistributionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistributionStrategy::RoundRobin => write!(f, "round_robin"),
            DistributionStrategy::LeastBusy => write!(f, "least_busy"),
            DistributionStrategy::CapabilityMatch => write!(f, "capability_match"),
            DistributionStrategy::LoadBalanced => write!(f, "load_balanced"),
            DistributionStrategy::Partitioned => write!(f, "partitioned"),
            DistributionStrategy::Priority => write!(f, "priority"),
        }
    }
}

/// Task distributor for work assignment.
pub struct TaskDistributor {
    /// Distribution strategy.
    strategy: DistributionStrategy,
    /// Round-robin index.
    round_robin_index: std::sync::atomic::AtomicUsize,
}

impl TaskDistributor {
    /// Create a new task distributor.
    #[must_use]
    pub fn new(strategy: DistributionStrategy) -> Self {
        Self {
            strategy,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Select an agent for the given work unit.
    pub fn select_agent(&self, agents: &[AgentId], work: &WorkUnit) -> Option<AgentId> {
        if agents.is_empty() {
            return None;
        }

        match self.strategy {
            DistributionStrategy::RoundRobin => self.round_robin(agents),
            DistributionStrategy::LeastBusy => self.least_busy(agents),
            DistributionStrategy::CapabilityMatch => self.capability_match(agents, work),
            DistributionStrategy::LoadBalanced => self.load_balanced(agents, work),
            DistributionStrategy::Partitioned => self.partitioned(agents, work),
            DistributionStrategy::Priority => self.priority_based(agents, work),
        }
    }

    /// Round-robin selection.
    fn round_robin(&self, agents: &[AgentId]) -> Option<AgentId> {
        let index = self
            .round_robin_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % agents.len();
        agents.get(index).copied()
    }

    /// Select least busy agent (simplified - would need agent status).
    fn least_busy(&self, agents: &[AgentId]) -> Option<AgentId> {
        // For now, just return first agent
        // In full implementation, would check agent load metrics
        agents.first().copied()
    }

    /// Select agent based on capability match.
    fn capability_match(&self, agents: &[AgentId], work: &WorkUnit) -> Option<AgentId> {
        if work.required_capabilities.is_empty() {
            return self.round_robin(agents);
        }

        // For now, return first agent
        // In full implementation, would check agent capabilities
        agents.first().copied()
    }

    /// Load-balanced selection.
    fn load_balanced(&self, agents: &[AgentId], work: &WorkUnit) -> Option<AgentId> {
        // Simple implementation: distribute based on complexity
        // Higher complexity work goes to "stronger" agents (simulated by index)
        let index = if work.complexity > 7 {
            0 // Assume first agent is "strongest"
        } else {
            self.round_robin_index
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                % agents.len()
        };
        agents.get(index).copied()
    }

    /// Partitioned work distribution.
    fn partitioned(&self, agents: &[AgentId], work: &WorkUnit) -> Option<AgentId> {
        // For partitioned work, select based on partition index if present
        let index = self
            .round_robin_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % agents.len();
        agents.get(index).copied()
    }

    /// Priority-based selection.
    fn priority_based(&self, agents: &[AgentId], work: &WorkUnit) -> Option<AgentId> {
        // High priority work goes to first available agent
        // For now, just round-robin with priority consideration
        if work.priority >= 8 {
            agents.first().copied() // High priority -> first agent
        } else {
            self.round_robin(agents)
        }
    }

    /// Partition work into chunks.
    pub fn partition_work(&self, work: &WorkUnit, num_partitions: usize) -> Vec<TaskPartition> {
        let mut partitions = Vec::with_capacity(num_partitions);

        for i in 0..num_partitions {
            partitions.push(TaskPartition {
                partition_id: format!("{}-partition-{}", work.id, i),
                parent_id: work.id.clone(),
                index: i,
                total: num_partitions,
                data: work.context.clone(), // Simplified: just clone context
            });
        }

        partitions
    }

    /// Check if all dependencies for a work unit are satisfied.
    pub fn dependencies_satisfied(&self, work: &WorkUnit, completed_work_ids: &[String]) -> bool {
        work.dependencies
            .iter()
            .all(|dep| completed_work_ids.contains(dep))
    }
}

/// Partition strategy for dividing work.
pub enum PartitionStrategy {
    /// Divide by count (N equal parts).
    ByCount(usize),
    /// Divide by size (chunks of N items).
    BySize(usize),
    /// Divide by content boundaries.
    ByBoundary,
}

/// Work queue with priority ordering.
pub struct PriorityWorkQueue {
    /// Work units organized by priority.
    queues: [VecDeque<WorkUnit>; 10], // One queue per priority level (1-10)
}

impl PriorityWorkQueue {
    /// Create a new priority work queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queues: std::array::from_fn(|_| VecDeque::new()),
        }
    }

    /// Add work to the queue.
    pub fn add(&mut self, work: WorkUnit) {
        let priority = work.priority.clamp(1, 10) as usize - 1;
        self.queues[priority].push_back(work);
    }

    /// Get next work unit (highest priority first).
    pub fn next(&mut self) -> Option<WorkUnit> {
        for i in (0..10).rev() {
            if let Some(work) = self.queues[i].pop_front() {
                return Some(work);
            }
        }
        None
    }

    /// Peek at next work without removing.
    pub fn peek(&self) -> Option<&WorkUnit> {
        for i in (0..10).rev() {
            if let Some(work) = self.queues[i].front() {
                return Some(work);
            }
        }
        None
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(|q| q.is_empty())
    }

    /// Get total count.
    pub fn len(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }
}

impl Default for PriorityWorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_unit_builder() {
        let work = WorkUnit::new("task-1", "Do something")
            .with_priority(8)
            .with_complexity(7)
            .with_context(serde_json::json!({"key": "value"}))
            .with_dependency("task-0");

        assert_eq!(work.id, "task-1");
        assert_eq!(work.description, "Do something");
        assert_eq!(work.priority, 8);
        assert_eq!(work.complexity, 7);
        assert_eq!(work.dependencies.len(), 1);
    }

    #[test]
    fn test_round_robin_distribution() {
        let distributor = TaskDistributor::new(DistributionStrategy::RoundRobin);

        let agents = vec![AgentId::new(), AgentId::new(), AgentId::new()];
        let work = WorkUnit::new("work-1", "Test work");

        // Should cycle through agents
        let agent1 = distributor.select_agent(&agents, &work);
        let agent2 = distributor.select_agent(&agents, &work);
        let agent3 = distributor.select_agent(&agents, &work);
        let agent4 = distributor.select_agent(&agents, &work); // Should wrap to first

        assert!(agent1.is_some());
        assert!(agent2.is_some());
        assert!(agent3.is_some());
        assert!(agent4.is_some());
        assert_eq!(agent1, agent4); // Should cycle back
    }

    #[test]
    fn test_priority_based_distribution() {
        let distributor = TaskDistributor::new(DistributionStrategy::Priority);

        let agents = vec![AgentId::new(), AgentId::new()];
        let high_priority = WorkUnit::new("work-1", "High").with_priority(9);
        let low_priority = WorkUnit::new("work-2", "Low").with_priority(3);

        // High priority should go to first agent
        let agent_high = distributor.select_agent(&agents, &high_priority);
        assert_eq!(agent_high, Some(agents[0]));

        // Low priority may go to different agent
        let _agent_low = distributor.select_agent(&agents, &low_priority);
    }

    #[test]
    fn test_load_balanced_distribution() {
        let distributor = TaskDistributor::new(DistributionStrategy::LoadBalanced);

        let agents = vec![AgentId::new(), AgentId::new()];
        let complex_work = WorkUnit::new("work-1", "Complex").with_complexity(9);
        let simple_work = WorkUnit::new("work-2", "Simple").with_complexity(3);

        // Complex work should go to first agent ("strongest")
        let complex_agent = distributor.select_agent(&agents, &complex_work);
        assert_eq!(complex_agent, Some(agents[0]));

        // Simple work can go anywhere
        let _simple_agent = distributor.select_agent(&agents, &simple_work);
    }

    #[test]
    fn test_partition_work() {
        let distributor = TaskDistributor::new(DistributionStrategy::Partitioned);
        let work = WorkUnit::new("task-1", "Partitionable task")
            .with_context(serde_json::json!({"data": [1, 2, 3, 4, 5]}));

        let partitions = distributor.partition_work(&work, 3);

        assert_eq!(partitions.len(), 3);
        assert_eq!(partitions[0].total, 3);
        assert_eq!(partitions[1].index, 1);
        assert_eq!(partitions[0].parent_id, "task-1");
    }

    #[test]
    fn test_dependencies_satisfied() {
        let distributor = TaskDistributor::new(DistributionStrategy::RoundRobin);

        let work = WorkUnit::new("task-3", "Dependent task")
            .with_dependency("task-1")
            .with_dependency("task-2");

        let completed = vec!["task-1".to_string()];
        assert!(!distributor.dependencies_satisfied(&work, &completed));

        let completed = vec!["task-1".to_string(), "task-2".to_string()];
        assert!(distributor.dependencies_satisfied(&work, &completed));
    }

    #[test]
    fn test_priority_work_queue() {
        let mut queue = PriorityWorkQueue::new();

        queue.add(WorkUnit::new("low", "Low priority").with_priority(3));
        queue.add(WorkUnit::new("high", "High priority").with_priority(9));
        queue.add(WorkUnit::new("medium", "Medium priority").with_priority(5));

        // Should return highest priority first
        let next = queue.next().unwrap();
        assert_eq!(next.id, "high");
        assert_eq!(next.priority, 9);

        let next = queue.next().unwrap();
        assert_eq!(next.id, "medium");

        let next = queue.next().unwrap();
        assert_eq!(next.id, "low");

        assert!(queue.is_empty());
    }

    #[test]
    fn test_priority_work_queue_peek() {
        let mut queue = PriorityWorkQueue::new();

        queue.add(WorkUnit::new("high", "High priority").with_priority(9));
        queue.add(WorkUnit::new("low", "Low priority").with_priority(2));

        // Peek should not remove
        let peeked = queue.peek().unwrap();
        assert_eq!(peeked.id, "high");

        let peeked_again = queue.peek().unwrap();
        assert_eq!(peeked_again.id, "high");

        // Length unchanged
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_distribution_strategy_display() {
        assert_eq!(DistributionStrategy::RoundRobin.to_string(), "round_robin");
        assert_eq!(DistributionStrategy::LeastBusy.to_string(), "least_busy");
        assert_eq!(DistributionStrategy::LoadBalanced.to_string(), "load_balanced");
    }

    #[test]
    fn test_empty_agent_list() {
        let distributor = TaskDistributor::new(DistributionStrategy::RoundRobin);
        let work = WorkUnit::new("work-1", "Test");

        let agent = distributor.select_agent(&[], &work);
        assert!(agent.is_none());
    }

    #[test]
    fn test_task_partition_fields() {
        let partition = TaskPartition {
            partition_id: "part-1-0".to_string(),
            parent_id: "part-1".to_string(),
            index: 0,
            total: 3,
            data: serde_json::json!({"chunk": "data"}),
        };

        assert_eq!(partition.partition_id, "part-1-0");
        assert_eq!(partition.index, 0);
        assert_eq!(partition.total, 3);
    }
}
