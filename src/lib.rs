//! # Local DAG Scheduler
//!
//! A concurrent DAG (Directed Acyclic Graph) executor that maximizes parallelism
//! by running tasks as soon as their dependencies are resolved.
//!
//! ## Key Features
//!
//! - **Maximum concurrency**: Unlike layer-based approaches, tasks start immediately
//!   when their dependencies complete
//! - **Event-driven**: Uses channels to coordinate task completion and scheduling
//! - **Async/await**: Built on Tokio for efficient async execution
//! - **Type-safe**: Leverages Rust's type system for correctness

pub mod dag;
pub mod mock;

pub use dag::{DagExecutor, Node, ProcessFn};
pub use mock::generate_mock_data;
