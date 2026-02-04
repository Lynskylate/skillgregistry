// E2E tests - imports from the e2e-tests crate src/
// Test scenarios are defined in src/test_scenarios.rs
// They are automatically included because they have #[tokio::test] attributes

// Re-export for convenience (this will make the tests accessible)
pub use e2e_tests::test_scenarios::*;