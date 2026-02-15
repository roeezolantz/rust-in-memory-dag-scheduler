# Local DAG Executor

A high-performance concurrent DAG (Directed Acyclic Graph) executor in Rust that maximizes parallelism by running tasks as soon as their dependencies are resolved.

## Features

- **Maximum Concurrency**: Unlike layer-based approaches that wait for all tasks in a layer to complete, this executor starts new tasks immediately when their dependencies resolve
- **Event-Driven Architecture**: Uses channels to coordinate task completion and scheduling
- **Async/Await**: Built on Tokio for efficient async execution
- **Type-Safe**: Leverages Rust's type system for correctness
- **Flexible Processing**: Custom async processing functions for real workloads
- **Mock Data Generation**: Built-in utilities for testing with configurable DAG generation

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
local-dag = { git = "https://github.com/YOUR_USERNAME/local-dag" }
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use local_dag::{DagExecutor, Node};

#[tokio::main]
async fn main() {
    // Create nodes with dependencies
    let nodes = vec![
        Node::new("fetch-users"),
        Node::new("fetch-orders"),
        Node::new("process-users").depends_on("fetch-users"),
        Node::new("process-orders").depends_on("fetch-orders"),
        Node::new("generate-report").depends_on_all(["process-users", "process-orders"]),
    ];

    // Execute with maximum concurrency
    let result = DagExecutor::new(nodes)
        .verbose(true)
        .run()
        .await;

    println!("Processed {} nodes", result.processed_count);
}
```

## How It Works

### Layer-Based vs Event-Driven

**Traditional Layer-Based Approach** (what most implementations do):

```
Layer 1: [A, B, C] all start -> wait for ALL to complete
Layer 2: [D, E] all start -> wait for ALL to complete
Layer 3: [F] starts
```

If A takes 100ms and B takes 500ms, D (which only depends on A) waits 500ms unnecessarily.

**This Implementation** (event-driven):

```
t=0ms:   A, B, C start
t=100ms: A completes -> D starts immediately
t=150ms: D completes -> F starts (if ready)
t=200ms: C completes -> E starts immediately
t=500ms: B completes
```

Tasks start the moment their dependencies are satisfied, not when a whole "layer" completes.

## API Reference

### Node

```rust
// Create a simple node
let node = Node::new("task-1");

// With dependencies
let node = Node::new("task-2")
    .depends_on("task-1")
    .depends_on("other-task");

// With multiple dependencies at once
let node = Node::new("final")
    .depends_on_all(["task-1", "task-2", "task-3"]);

// Mark as pending (for tracking data that needs to be filled)
let node = Node::new("data-node")
    .pending(true)
    .with_payload("initial data");
```

### DagExecutor

```rust
let result = DagExecutor::new(nodes)
    // Custom processing function
    .with_process_fn(Arc::new(|id| Box::pin(async move {
        // Your async work here
        do_work(&id).await;
        id
    })))
    // Enable logging
    .verbose(true)
    .run()
    .await;

// Check results
if result.is_complete() {
    println!("All {} nodes processed!", result.processed_count);
} else {
    println!("Failed nodes: {:?}", result.failed_nodes);
}
```

### Mock Data Generation

```rust
use local_dag::mock::{generate_mock_data, generate_mock_data_with_config, MockConfig};

// Simple: generate 50 nodes
let nodes = generate_mock_data(50);

// Custom configuration
let config = MockConfig {
    node_count: 100,
    pending_probability: 0.5,
    max_dependencies: 4,
    root_nodes: 10,
    shuffle: true,
};
let nodes = generate_mock_data_with_config(config);
```

## Running

```bash
# Build
cargo build --release

# Run the example
cargo run

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

## Example Output

```
=== Concurrent DAG Executor ===

Generating 50 mock nodes with random dependencies...
  - Total nodes: 50
  - pendings: 15
  - Nodes with dependencies: 28

[Initial] Starting 22 ready nodes
  -> Starting: node_1
  -> Starting: node_5
  -> Starting: node_3
  ...
[1/50] Completed: node_3
[2/50] Completed: node_5
  -> Unblocked 2 nodes: ["node_12", "node_18"]
[3/50] Completed: node_1
  -> Unblocked 1 nodes: ["node_25"]
...

=== Execution Summary ===
  - Processed: 50/50
  - Status: SUCCESS - All nodes processed
```

## Use Cases

- **Build Systems**: Compile dependencies in optimal order
- **Data Pipelines**: Process data transformations with dependencies
- **Task Schedulers**: Execute jobs with prerequisite requirements
- **Workflow Engines**: Run complex multi-step workflows efficiently

## License

MIT
