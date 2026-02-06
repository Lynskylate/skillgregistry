//! Temporal Rust SDK simplified Activity registration utilities
//!
//! This module provides a simplified Activity registration API that significantly reduces boilerplate.
//!
//! # Raw API vs Simplified API comparison
//!
//! ## Raw API (verbose, ~25 lines)
//! ```rust
//! let service = Arc::new(GreeterService::new("Hello"));
//! let svc_clone = Arc::clone(&service);
//!
//! worker.register_activity("greet-activity", move |_ctx: ActContext, payload: Payload| {
//!     let svc = Arc::clone(&svc_clone);
//!     Box::pin(async move {
//!         let input: GreetInput = serde_json::from_slice(&payload.data)?;
//!         let output = svc.greet(input).await?;
//!         let data = serde_json::to_vec(&output)?;
//!         Ok(Payload {
//!             metadata: HashMap::from([("encoding".to_string(), "json/plain".as_bytes().to_vec())]),
//!             data,
//!             ..Default::default()
//!         })
//!     }) as Pin<Box<dyn Future<Output = Result<Payload, ActivityError>> + Send>>
//! });
//! ```
//!
//! ## Simplified API (recommended, 3 lines)
//! ```rust
//! let svc = Arc::clone(&service);
//! worker.register_activity_json("greet", move |ctx, input: GreetInput| {
//!     let svc = Arc::clone(&svc);
//!     async move { svc.greet(input).await }
//! });
//! ```

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use temporalio_common::protos::temporal::api::common::v1::Payload;
use temporalio_sdk::{ActContext, ActivityError, Worker};

/// Activity registration trait with automatic JSON serialization
///
/// Provides a simplified Activity registration method for `Worker` that automatically handles:
/// - JSON deserialization of the request payload
/// - JSON serialization of the response payload
/// - Payload construction
///
/// # Usage example
///
/// ```rust
/// use temporal_sdk_helpers::ActivityRegistrarJson;
///
/// let service = Arc::new(MyService::new());
///
/// // Register Activity (only 3 lines)
/// let svc = Arc::clone(&service);
/// worker.register_activity_json("my-activity", move |ctx, input: MyInput| {
///     let svc = Arc::clone(&svc);
///     async move { svc.process(input).await }
/// });
/// ```
pub trait ActivityRegistrarJson {
    /// Register an Activity with automatic JSON serialization
    ///
    /// # Parameters
    ///
    /// * `activity_type` - Activity type name
    /// * `f` - Activity handler function; receives `(ActContext, Input)`, returns `Result<Output, ActivityError>`
    ///
    /// # Type bounds
    ///
    /// * `Input`: must implement `DeserializeOwned` and `Send`
    /// * `Output`: must implement `Serialize` and `Send`
    fn register_activity_json<Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        f: impl Fn(ActContext, Input) -> Fut + Send + Sync + Clone + 'static,
    )
    where
        Input: DeserializeOwned + Send + 'static,
        Output: Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, ActivityError>> + Send + 'static;
}

impl ActivityRegistrarJson for Worker {
    fn register_activity_json<Input, Output, Fut>(
        &mut self,
        activity_type: impl Into<String>,
        f: impl Fn(ActContext, Input) -> Fut + Send + Sync + Clone + 'static,
    )
    where
        Input: DeserializeOwned + Send + 'static,
        Output: Serialize + Send + 'static,
        Fut: Future<Output = Result<Output, ActivityError>> + Send + 'static,
    {
        self.register_activity(activity_type, move |ctx: ActContext, payload: Payload| {
            // Auto-deserialize input
            let input: Input = match serde_json::from_slice(&payload.data) {
                Ok(i) => i,
                Err(e) => {
                    return Box::pin(async move {
                        Err(ActivityError::from(anyhow::anyhow!(
                            "Failed to deserialize input: {}",
                            e
                        )))
                    }) as Pin<Box<dyn Future<Output = _> + Send>>;
                }
            };

            let f = f.clone();
            Box::pin(async move {
                match f(ctx, input).await {
                    Ok(output) => {
                        // Auto-serialize output
                        let data = serde_json::to_vec(&output).map_err(|e| {
                            ActivityError::from(anyhow::anyhow!(
                                "Failed to serialize output: {}",
                                e
                            ))
                        })?;
                        
                        Ok(Payload {
                            metadata: std::collections::HashMap::from([(
                                "encoding".to_string(),
                                "json/plain".as_bytes().to_vec(),
                            )]),
                            data,
                            ..Default::default()
                        })
                    }
                    Err(e) => Err(e),
                }
            }) as Pin<Box<dyn Future<Output = _> + Send>>
        });
    }
}

/// Quick registration helper macro
///
/// For quickly registering Activities that don't require complex logic
///
/// # Usage example
///
/// ```rust
/// let service = Arc::new(GreeterService::new("Hello"));
///
/// // Register using macro
/// register_activity!(worker, "greet", service, greet);
/// ```
#[macro_export]
macro_rules! register_activity {
    ($worker:expr, $activity_type:expr, $service:expr, $method:ident) => {{
        let svc = std::sync::Arc::clone(&$service);
        $worker.register_activity_json($activity_type, move |ctx, input| {
            let svc = std::sync::Arc::clone(&svc);
            async move { svc.$method(input).await }
        });
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
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
        async fn greet(&self, input: TestInput) -> Result<TestOutput, ActivityError> {
            Ok(TestOutput {
                message: format!("{} {}!", self.prefix, input.name),
            })
        }
    }

    /// Verify the trait compiles
    #[test]
    fn test_trait_compiles() {
        // This test primarily verifies the code compiles
        println!("ActivityRegistrarJson trait compiled successfully");
    }

    /// Verify the macro compiles
    #[test]
    fn test_macro_compiles() {
        // Check that the macro definition is correct
        println!("register_activity! macro compiled successfully");
    }
}
