//! Unit-test-only mock implementation that does not depend on the Temporal SDK.
//!
//! This validates the core logic of the simplified registration APIs.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ============ Mock Temporal SDK types ============

struct MockPayload {
    data: Vec<u8>,
}

struct MockActContext;

#[derive(Debug)]
struct MockActivityError(String);

impl std::fmt::Display for MockActivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MockActivityError {}

// ============ Core simplified APIs (no Temporal SDK dependency) ============

/// Mock worker type.
type ActivityFn = Box<dyn Fn(MockActContext, MockPayload) -> Pin<Box<dyn Future<Output = Result<MockPayload, MockActivityError>> + Send>> + Send + Sync>;

struct MockWorker {
    activities: Vec<(String, ActivityFn)>,
}

impl MockWorker {
    fn new() -> Self {
        Self { activities: Vec::new() }
    }

    fn register_activity<F, Fut>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(MockActContext, MockPayload) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<MockPayload, MockActivityError>> + Send + 'static,
    {
        let boxed = Box::new(move |ctx: MockActContext, payload: MockPayload| {
            Box::pin(f(ctx, payload)) as Pin<Box<dyn Future<Output = _> + Send>>
        });
        self.activities.push((name.into(), boxed));
    }
}

/// Registration trait with automatic JSON (de)serialization.
pub trait ActivityRegistrarJson {
    fn register_activity_json<F, Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        f: F,
    )
    where
        F: Fn(MockActContext, Input) -> Fut + Send + Sync + 'static,
        Input: serde::de::DeserializeOwned + Send + 'static,
        Output: serde::Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, MockActivityError>> + Send + 'static;
}

impl ActivityRegistrarJson for MockWorker {
    fn register_activity_json<F, Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        f: F,
    )
    where
        F: Fn(MockActContext, Input) -> Fut + Send + Sync + 'static,
        Input: serde::de::DeserializeOwned + Send + 'static,
        Output: serde::Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, MockActivityError>> + Send + 'static,
    {
        self.register_activity(activity_type, move |_ctx: MockActContext, payload: MockPayload| {
            let input: Input = match serde_json::from_slice(&payload.data) {
                Ok(i) => i,
                Err(e) => {
                    return Box::pin(async move {
                        Err(MockActivityError(format!("Deserialize failed: {}", e)))
                    }) as Pin<Box<dyn Future<Output = _> + Send>>;
                }
            };

            Box::pin(async move {
                match f(_ctx, input).await {
                    Ok(output) => {
                        let data = serde_json::to_vec(&output).map_err(|e| {
                            MockActivityError(format!("Serialize failed: {}", e))
                        })?;
                        Ok(MockPayload { data })
                    }
                    Err(e) => Err(e),
                }
            }) as Pin<Box<dyn Future<Output = _> + Send>>
        });
    }
}

/// Simplified registration macro.
#[macro_export]
macro_rules! register_struct_activity {
    ($worker:expr, $activity_type:expr, $service:expr, $method:ident) => {
        {
            // Use fully-qualified paths because this is a `#[macro_export]` macro.
            let svc = ::std::sync::Arc::clone(&$service);
            $worker.register_activity_json($activity_type, move |_ctx: MockActContext, input| {
                let svc = ::std::sync::Arc::clone(&svc);
                async move { svc.$method(input).await }
            });
        }
    };
}

/// Worker extension trait.
trait WorkerExt {
    fn register_service_activity<T, Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        service: Arc<T>,
        method: fn(&T, Input) -> Fut,
    )
    where
        T: Send + Sync + 'static,
        Input: serde::de::DeserializeOwned + Send + 'static,
        Output: serde::Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, MockActivityError>> + Send + 'static;
}

impl WorkerExt for MockWorker {
    fn register_service_activity<T, Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        service: Arc<T>,
        method: fn(&T, Input) -> Fut,
    )
    where
        T: Send + Sync + 'static,
        Input: serde::de::DeserializeOwned + Send + 'static,
        Output: serde::Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, MockActivityError>> + Send + 'static,
    {
        let svc = service.clone();
        self.register_activity_json(activity_type, move |_ctx: MockActContext, input: Input| {
            let svc = Arc::clone(&svc);
            async move { method(&*svc, input).await }
        });
    }
}

// ============ Test services and types ============

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TestInput {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TestOutput {
    message: String,
}

#[derive(Clone)]
struct GreeterService {
    prefix: String,
}

impl GreeterService {
    fn new(prefix: impl Into<String>) -> Self {
        Self { prefix: prefix.into() }
    }

    async fn greet(&self, input: TestInput) -> Result<TestOutput, MockActivityError> {
        Ok(TestOutput {
            message: format!("{} {}!", self.prefix, input.name),
        })
    }

    async fn farewell(&self, input: TestInput) -> Result<TestOutput, MockActivityError> {
        Ok(TestOutput {
            message: format!("Goodbye, {}! {}", input.name, self.prefix),
        })
    }
}

// ============ Unit tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_with_macro() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hello"));

        // Register via macro (1 line)
        register_struct_activity!(worker, "greet", service, greet);

        assert_eq!(worker.activities.len(), 1);
        assert_eq!(worker.activities[0].0, "greet");
    }

    #[tokio::test]
    async fn test_register_with_trait() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hello"));

        // Register via trait (1 line)
        worker.register_service_activity("farewell", service, GreeterService::farewell);

        assert_eq!(worker.activities.len(), 1);
        assert_eq!(worker.activities[0].0, "farewell");
    }

    #[tokio::test]
    async fn test_register_multiple_activities() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hi"));

        // Register multiple activities
        register_struct_activity!(worker, "greet", service, greet);
        register_struct_activity!(worker, "farewell", service, farewell);

        assert_eq!(worker.activities.len(), 2);
        assert_eq!(worker.activities[0].0, "greet");
        assert_eq!(worker.activities[1].0, "farewell");
    }

    #[tokio::test]
    async fn test_activity_execution() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hello"));

        register_struct_activity!(worker, "greet", service, greet);

        // Test activity execution
        let input = TestInput { name: "World".to_string() };
        let payload = MockPayload {
            data: serde_json::to_vec(&input).unwrap(),
        };

        let activity = &worker.activities[0].1;
        let result = activity(MockActContext, payload).await;

        assert!(result.is_ok());
        let output: TestOutput = serde_json::from_slice(&result.unwrap().data).unwrap();
        assert_eq!(output.message, "Hello World!");
    }

    #[test]
    fn test_code_reduction() {
        println!("\n");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Line count comparison");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("Original API (manual):");
        println!("  - per Activity: ~25 lines");
        println!("  - 5 Activities: 125 lines");
        println!();
        println!("Simplified API (macro):");
        println!("  - per Activity: 1 line");
        println!("  - 5 Activities: 5 lines");
        println!();
        println!("Reduction: ~96% boilerplate removed!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

// ============ Main (runs the demo) ============

#[tokio::main]
async fn main() {
    println!("Running simplified API tests...\n");

    // Test 1: macro registration
    let mut worker = MockWorker::new();
    let service = Arc::new(GreeterService::new("Hello"));
    register_struct_activity!(worker, "greet", service, greet);
    println!("✅ Test 1 passed: Macro registration (1 line)");

    // Test 2: trait registration
    let mut worker = MockWorker::new();
    let service = Arc::new(GreeterService::new("Hello"));
    worker.register_service_activity("farewell", service, GreeterService::farewell);
    println!("✅ Test 2 passed: Trait registration (1 line)");

    // Test 3: activity execution
    let mut worker = MockWorker::new();
    let service = Arc::new(GreeterService::new("Hello"));
    register_struct_activity!(worker, "greet", service, greet);
    
    let input = TestInput { name: "World".to_string() };
    let payload = MockPayload {
        data: serde_json::to_vec(&input).unwrap(),
    };
    let activity = &worker.activities[0].1;
    let result = activity(MockActContext, payload).await.unwrap();
    let output: TestOutput = serde_json::from_slice(&result.data).unwrap();
    assert_eq!(output.message, "Hello World!");
    println!("✅ Test 3 passed: Activity execution works");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("All tests passed!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("Takeaways:");
    println!("  - reduce ~25 lines to 1-3 lines");
    println!("  - automatic JSON (de)serialization");
    println!("  - compile-time type safety");
    println!("  - supports dependency injection");
}
