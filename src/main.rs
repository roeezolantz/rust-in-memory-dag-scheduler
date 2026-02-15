use local_dag_scheduler::{generate_mock_data, DagExecutor, ProcessFn};
use rand::Rng;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Creates a processing function that simulates work with random delays
fn create_work_simulator() -> ProcessFn {
    Arc::new(|node_id: String| {
        Box::pin(async move {
            // Generate delay before the await to avoid Send issues
            let delay_ms = {
                let mut rng = rand::thread_rng();
                rng.gen_range(100..=500)
            };
            sleep(Duration::from_millis(delay_ms)).await;
            node_id
        })
    })
}

#[tokio::main]
async fn main() {
    println!("=== Concurrent DAG Executor ===\n");
    println!("Generating 50 mock nodes with random dependencies...");

    let nodes = generate_mock_data(50);

    // Print statistics
    let pending_count = nodes.iter().filter(|n| n.is_pending).count();
    let with_deps = nodes.iter().filter(|n| !n.dependencies.is_empty()).count();
    println!("  - Total nodes: {}", nodes.len());
    println!("  - pendings: {}", pending_count);
    println!("  - Nodes with dependencies: {}", with_deps);

    // Run the DAG
    let result = DagExecutor::new(nodes)
        .with_process_fn(create_work_simulator())
        .verbose(true)
        .run()
        .await;

    println!("\n=== Execution Summary ===");
    println!(
        "  - Processed: {}/{}",
        result.processed_count, result.total_nodes
    );

    if result.is_complete() {
        println!("  - Status: SUCCESS - All nodes processed");
    } else {
        println!(
            "  - Status: PARTIAL - {} nodes failed",
            result.failed_nodes.len()
        );
        println!("  - Failed nodes: {:?}", result.failed_nodes);
    }
}
