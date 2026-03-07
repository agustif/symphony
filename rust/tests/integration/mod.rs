#![forbid(unsafe_code)]

//! Integration tests for the Symphony runtime.
//!
//! These tests exercise full scenarios across multiple components to verify
//! correct integration behavior that unit tests cannot capture.

mod concurrent_workers;
mod graceful_shutdown;
mod lifecycle_e2e;
mod retry_exhaustion;
mod workspace_leaks;
