//! Mock data generation for testing

use crate::Node;
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashSet;

/// Configuration for mock data generation
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Number of nodes to generate
    pub node_count: usize,
    /// Probability (0.0-1.0) that a node is a pending
    pub pending_probability: f64,
    /// Maximum number of dependencies per node
    pub max_dependencies: usize,
    /// Number of root nodes (nodes without dependencies)
    pub root_nodes: usize,
    /// Whether to shuffle the output
    pub shuffle: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            node_count: 50,
            pending_probability: 0.3,
            max_dependencies: 3,
            root_nodes: 6,
            shuffle: true,
        }
    }
}

/// Generates mock DAG data with randomized dependencies
///
/// Creates nodes where early nodes have no dependencies (roots),
/// and later nodes may depend on earlier ones (preventing cycles).
pub fn generate_mock_data(count: usize) -> Vec<Node> {
    generate_mock_data_with_config(MockConfig {
        node_count: count,
        ..Default::default()
    })
}

/// Generates mock DAG data with custom configuration
pub fn generate_mock_data_with_config(config: MockConfig) -> Vec<Node> {
    let mut rng = rand::thread_rng();
    let mut nodes: Vec<Node> = Vec::with_capacity(config.node_count);

    // Create nodes with IDs
    for i in 1..=config.node_count {
        let mut node = Node::new(format!("node_{}", i))
            .pending(rng.gen_bool(config.pending_probability))
            .with_payload(format!("Data for node_{}", i));

        // Assign dependencies only to non-root nodes
        if i > config.root_nodes {
            let dep_count = rng.gen_range(0..=config.max_dependencies);
            let mut deps = HashSet::new();

            // Only depend on earlier nodes to prevent cycles
            for _ in 0..dep_count {
                let dep_idx = rng.gen_range(0..nodes.len());
                deps.insert(nodes[dep_idx].id.clone());
            }

            node.dependencies = deps.into_iter().collect();
        }

        nodes.push(node);
    }

    // Shuffle to simulate random arrival order
    if config.shuffle {
        nodes.shuffle(&mut rng);
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_data_count() {
        let nodes = generate_mock_data(25);
        assert_eq!(nodes.len(), 25);
    }

    #[test]
    fn test_generate_mock_data_unique_ids() {
        let nodes = generate_mock_data(50);
        let ids: HashSet<_> = nodes.iter().map(|n| &n.id).collect();
        assert_eq!(ids.len(), 50);
    }

    #[test]
    fn test_no_self_dependencies() {
        let nodes = generate_mock_data(50);
        for node in &nodes {
            assert!(!node.dependencies.contains(&node.id));
        }
    }

    #[test]
    fn test_no_cyclic_dependencies() {
        // Create nodes without shuffling to verify the ordering
        let config = MockConfig {
            node_count: 50,
            shuffle: false,
            ..Default::default()
        };
        let nodes = generate_mock_data_with_config(config);

        // Build a map of node_id -> index in original order
        let id_to_idx: std::collections::HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();

        // Each dependency should point to a node with a lower index
        for (idx, node) in nodes.iter().enumerate() {
            for dep_id in &node.dependencies {
                let dep_idx = id_to_idx.get(dep_id).unwrap();
                assert!(
                    *dep_idx < idx,
                    "Node {} (idx {}) depends on {} (idx {})",
                    node.id,
                    idx,
                    dep_id,
                    dep_idx
                );
            }
        }
    }

    #[test]
    fn test_root_nodes_have_no_dependencies() {
        let config = MockConfig {
            node_count: 20,
            root_nodes: 5,
            shuffle: false,
            ..Default::default()
        };
        let nodes = generate_mock_data_with_config(config);

        // First 5 nodes should have no dependencies
        for node in nodes.iter().take(5) {
            assert!(
                node.dependencies.is_empty(),
                "Root node {} has dependencies: {:?}",
                node.id,
                node.dependencies
            );
        }
    }

    #[test]
    fn test_custom_config() {
        let config = MockConfig {
            node_count: 10,
            pending_probability: 1.0, // All pendings
            max_dependencies: 0,      // No dependencies
            root_nodes: 10,
            shuffle: false,
        };
        let nodes = generate_mock_data_with_config(config);

        assert_eq!(nodes.len(), 10);
        for node in &nodes {
            assert!(node.is_pending);
            assert!(node.dependencies.is_empty());
        }
    }
}
