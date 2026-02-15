//! Core DAG execution logic

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// A node in the DAG with dependencies on other nodes
#[derive(Debug, Clone)]
pub struct Node {
    /// Unique identifier for this node
    pub id: String,
    /// Whether this node contains pending data that needs to be filled
    pub is_pending: bool,
    /// IDs of nodes that must complete before this node can run
    pub dependencies: Vec<String>,
    /// Optional payload data
    pub payload: Option<String>,
}

impl Node {
    /// Creates a new node with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            is_pending: false,
            dependencies: Vec::new(),
            payload: None,
        }
    }

    /// Sets whether this node is pending
    pub fn pending(mut self, is_pending: bool) -> Self {
        self.is_pending = is_pending;
        self
    }

    /// Adds a dependency to this node
    pub fn depends_on(mut self, dep_id: impl Into<String>) -> Self {
        self.dependencies.push(dep_id.into());
        self
    }

    /// Adds multiple dependencies to this node
    pub fn depends_on_all(mut self, deps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.dependencies.extend(deps.into_iter().map(Into::into));
        self
    }

    /// Sets the payload for this node
    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }
}

/// Type alias for the async processing function
pub type ProcessFn =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

/// Internal state for tracking DAG execution
struct ExecutorState {
    nodes: HashMap<String, Node>,
    in_degree: HashMap<String, usize>,
    dependents: HashMap<String, Vec<String>>,
    processed: HashSet<String>,
}

impl ExecutorState {
    fn new(nodes: Vec<Node>) -> Self {
        let mut nodes_map: HashMap<String, Node> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        // Build the node map
        for node in &nodes {
            nodes_map.insert(node.id.clone(), node.clone());
            dependents.insert(node.id.clone(), Vec::new());
        }

        // Calculate in-degrees and build reverse dependency map
        for node in &nodes {
            let mut pending = 0;
            for dep_id in &node.dependencies {
                if nodes_map.contains_key(dep_id) {
                    pending += 1;
                    dependents.get_mut(dep_id).unwrap().push(node.id.clone());
                }
            }
            in_degree.insert(node.id.clone(), pending);
        }

        Self {
            nodes: nodes_map,
            in_degree,
            dependents,
            processed: HashSet::new(),
        }
    }

    /// Returns IDs of nodes ready to execute (in_degree == 0 and not processed)
    fn get_ready_nodes(&self) -> Vec<String> {
        self.in_degree
            .iter()
            .filter(|(id, &deg)| deg == 0 && !self.processed.contains(*id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Marks a node as completed and returns IDs of newly unblocked nodes
    fn mark_completed(&mut self, node_id: &str) -> Vec<String> {
        self.processed.insert(node_id.to_string());

        // Mark pending as resolved
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.is_pending = false;
        }

        // Decrement in-degree of dependents, collect newly ready nodes
        let mut newly_ready = Vec::new();
        if let Some(deps) = self.dependents.get(node_id) {
            for dep_id in deps {
                if let Some(deg) = self.in_degree.get_mut(dep_id) {
                    *deg -= 1;
                    if *deg == 0 && !self.processed.contains(dep_id) {
                        newly_ready.push(dep_id.clone());
                    }
                }
            }
        }
        newly_ready
    }

    fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    fn processed_count(&self) -> usize {
        self.processed.len()
    }
}

/// Result of DAG execution
#[derive(Debug)]
pub struct ExecutionResult {
    /// Total number of nodes processed
    pub processed_count: usize,
    /// Total number of nodes in the DAG
    pub total_nodes: usize,
    /// IDs of nodes that could not be processed (missing deps or cycles)
    pub failed_nodes: Vec<String>,
}

impl ExecutionResult {
    /// Returns true if all nodes were successfully processed
    pub fn is_complete(&self) -> bool {
        self.processed_count == self.total_nodes
    }
}

/// Concurrent DAG executor that maximizes parallelism
pub struct DagExecutor {
    nodes: Vec<Node>,
    process_fn: Option<ProcessFn>,
    verbose: bool,
}

impl DagExecutor {
    /// Creates a new executor with the given nodes
    pub fn new(nodes: Vec<Node>) -> Self {
        Self {
            nodes,
            process_fn: None,
            verbose: false,
        }
    }

    /// Sets a custom processing function for nodes
    pub fn with_process_fn(mut self, f: ProcessFn) -> Self {
        self.process_fn = Some(f);
        self
    }

    /// Enables verbose logging
    pub fn verbose(mut self, enabled: bool) -> Self {
        self.verbose = enabled;
        self
    }

    /// Executes the DAG with maximum concurrency
    pub async fn run(self) -> ExecutionResult {
        let total = self.nodes.len();
        if total == 0 {
            return ExecutionResult {
                processed_count: 0,
                total_nodes: 0,
                failed_nodes: Vec::new(),
            };
        }

        let state = Arc::new(Mutex::new(ExecutorState::new(self.nodes)));
        let process_fn = self.process_fn.unwrap_or_else(|| {
            Arc::new(|id| {
                Box::pin(async move { id }) as Pin<Box<dyn Future<Output = String> + Send>>
            })
        });
        let verbose = self.verbose;

        // Channel for completed tasks
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        // Start initial ready nodes
        {
            let state_guard = state.lock().await;
            let ready = state_guard.get_ready_nodes();

            if verbose {
                println!("[Initial] Starting {} ready nodes", ready.len());
            }

            for node_id in ready {
                let tx_clone = tx.clone();
                let process_fn = process_fn.clone();
                if verbose {
                    println!("  -> Starting: {}", node_id);
                }
                tokio::spawn(async move {
                    let completed_id = process_fn(node_id).await;
                    let _ = tx_clone.send(completed_id);
                });
            }
        }

        // Main loop: wait for completions and spawn newly ready tasks
        let mut processed = 0;
        while processed < total {
            match rx.recv().await {
                Some(completed_id) => {
                    processed += 1;
                    if verbose {
                        println!("[{}/{}] Completed: {}", processed, total, completed_id);
                    }

                    // Get newly ready nodes
                    let newly_ready = {
                        let mut state_lock = state.lock().await;
                        state_lock.mark_completed(&completed_id)
                    };

                    // Spawn newly ready tasks immediately
                    if !newly_ready.is_empty() {
                        if verbose {
                            println!(
                                "  -> Unblocked {} nodes: {:?}",
                                newly_ready.len(),
                                newly_ready
                            );
                        }
                        for node_id in newly_ready {
                            let tx_clone = tx.clone();
                            let process_fn = process_fn.clone();
                            tokio::spawn(async move {
                                let completed_id = process_fn(node_id).await;
                                let _ = tx_clone.send(completed_id);
                            });
                        }
                    }
                }
                None => break,
            }
        }

        // Check for unprocessed nodes
        let final_state = state.lock().await;
        let failed_nodes: Vec<String> = final_state
            .in_degree
            .iter()
            .filter(|(id, &deg)| deg > 0 && !final_state.processed.contains(*id))
            .map(|(id, _)| id.clone())
            .collect();

        if verbose && !failed_nodes.is_empty() {
            println!(
                "\nWarning: {} nodes could not be processed (circular deps or missing inputs)",
                failed_nodes.len()
            );
        }

        ExecutionResult {
            processed_count: final_state.processed_count(),
            total_nodes: final_state.total_nodes(),
            failed_nodes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_empty_dag() {
        let result = DagExecutor::new(vec![]).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 0);
    }

    #[tokio::test]
    async fn test_single_node() {
        let nodes = vec![Node::new("a")];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 1);
    }

    #[tokio::test]
    async fn test_linear_chain() {
        // a -> b -> c
        let nodes = vec![
            Node::new("a"),
            Node::new("b").depends_on("a"),
            Node::new("c").depends_on("b"),
        ];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 3);
    }

    #[tokio::test]
    async fn test_parallel_nodes() {
        // Three independent nodes should all run
        let nodes = vec![Node::new("a"), Node::new("b"), Node::new("c")];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 3);
    }

    #[tokio::test]
    async fn test_diamond_dependency() {
        //     a
        //    / \
        //   b   c
        //    \ /
        //     d
        let nodes = vec![
            Node::new("a"),
            Node::new("b").depends_on("a"),
            Node::new("c").depends_on("a"),
            Node::new("d").depends_on_all(["b", "c"]),
        ];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 4);
    }

    #[tokio::test]
    async fn test_execution_order() {
        // Track execution order to verify dependencies are respected
        let order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = order.clone();

        let process_fn: ProcessFn = Arc::new(move |id: String| {
            let order = order_clone.clone();
            Box::pin(async move {
                order.lock().await.push(id.clone());
                id
            })
        });

        // a -> b -> c
        let nodes = vec![
            Node::new("a"),
            Node::new("b").depends_on("a"),
            Node::new("c").depends_on("b"),
        ];

        DagExecutor::new(nodes)
            .with_process_fn(process_fn)
            .run()
            .await;

        let execution_order = order.lock().await;
        assert_eq!(execution_order.len(), 3);

        // Verify order: a must come before b, b must come before c
        let pos_a = execution_order.iter().position(|x| x == "a").unwrap();
        let pos_b = execution_order.iter().position(|x| x == "b").unwrap();
        let pos_c = execution_order.iter().position(|x| x == "c").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[tokio::test]
    async fn test_concurrent_execution() {
        // Verify that independent nodes run concurrently
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let current_concurrent = Arc::new(AtomicUsize::new(0));

        let max_clone = max_concurrent.clone();
        let current_clone = current_concurrent.clone();

        let process_fn: ProcessFn = Arc::new(move |id: String| {
            let max = max_clone.clone();
            let current = current_clone.clone();
            Box::pin(async move {
                // Increment current count
                let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                // Update max if needed
                max.fetch_max(now, Ordering::SeqCst);

                // Simulate work
                sleep(Duration::from_millis(50)).await;

                // Decrement current count
                current.fetch_sub(1, Ordering::SeqCst);
                id
            })
        });

        // 5 independent nodes should run concurrently
        let nodes = vec![
            Node::new("a"),
            Node::new("b"),
            Node::new("c"),
            Node::new("d"),
            Node::new("e"),
        ];

        DagExecutor::new(nodes)
            .with_process_fn(process_fn)
            .run()
            .await;

        // All 5 should have run concurrently
        assert_eq!(max_concurrent.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_missing_dependency() {
        // Node b depends on non-existent node "missing"
        let nodes = vec![Node::new("a"), Node::new("b").depends_on("missing")];
        let result = DagExecutor::new(nodes).run().await;
        // "a" should complete, "b" should complete (missing dep is ignored)
        assert!(result.is_complete());
    }

    #[tokio::test]
    async fn test_pending_nodes() {
        let nodes = vec![
            Node::new("a").pending(true),
            Node::new("b").pending(false).depends_on("a"),
        ];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 2);
    }

    #[tokio::test]
    async fn test_complex_dag() {
        //     a     b
        //    /|\    |
        //   c d e   f
        //    \|/   /
        //     g---+
        let nodes = vec![
            Node::new("a"),
            Node::new("b"),
            Node::new("c").depends_on("a"),
            Node::new("d").depends_on("a"),
            Node::new("e").depends_on("a"),
            Node::new("f").depends_on("b"),
            Node::new("g").depends_on_all(["c", "d", "e", "f"]),
        ];
        let result = DagExecutor::new(nodes).run().await;
        assert!(result.is_complete());
        assert_eq!(result.processed_count, 7);
    }
}
