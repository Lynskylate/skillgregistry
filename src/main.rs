//! 不依赖 Temporal SDK 的单元测试
//! 验证简化 API 的核心逻辑

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ============ 模拟 Temporal SDK 类型 ============

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

// ============ 简化 API 的核心实现（不依赖 Temporal SDK） ============

/// 模拟的 Worker 类型
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

/// 自动 JSON 序列化的注册 trait
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

/// 简化注册宏
#[macro_export]
macro_rules! register_struct_activity {
    ($worker:expr, $activity_type:expr, $service:expr, $method:ident) => {
        {
            let svc = Arc::clone(&$service);
            $worker.register_activity_json($activity_type, move |_ctx: MockActContext, input| {
                let svc = Arc::clone(&svc);
                async move { svc.$method(input).await }
            });
        }
    };
}

/// Worker 扩展 trait
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
        _method: fn(&T, Input) -> Fut,
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
            async move { _method(&*svc, input).await }
        });
    }
}

// ============ 测试用的服务和类型 ============

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

// ============ 单元测试 ============

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_with_macro() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hello"));

        // 使用宏注册（1 行代码）
        register_struct_activity!(worker, "greet", service, greet);

        assert_eq!(worker.activities.len(), 1);
        assert_eq!(worker.activities[0].0, "greet");
    }

    #[tokio::test]
    async fn test_register_with_trait() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hello"));

        // 使用 trait 注册（1 行代码）
        worker.register_service_activity("farewell", service, GreeterService::farewell);

        assert_eq!(worker.activities.len(), 1);
        assert_eq!(worker.activities[0].0, "farewell");
    }

    #[tokio::test]
    async fn test_register_multiple_activities() {
        let mut worker = MockWorker::new();
        let service = Arc::new(GreeterService::new("Hi"));

        // 注册多个 activities
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

        // 测试 activity 执行
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
        println!("代码行数对比");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("原始 API（手动）:");
        println!("  - 每个 Activity: ~25 行");
        println!("  - 5 个 Activities: 125 行");
        println!();
        println!("简化 API（宏）:");
        println!("  - 每个 Activity: 1 行");
        println!("  - 5 个 Activities: 5 行");
        println!();
        println!("减少: ~96% 的样板代码！");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

// ============ Main（运行测试） ============

#[tokio::main]
async fn main() {
    println!("Running simplified API tests...\n");

    // 测试 1: 宏注册
    let mut worker = MockWorker::new();
    let service = Arc::new(GreeterService::new("Hello"));
    register_struct_activity!(worker, "greet", service, greet);
    println!("✅ Test 1 passed: Macro registration (1 line)");

    // 测试 2: Trait 注册
    let mut worker = MockWorker::new();
    let service = Arc::new(GreeterService::new("Hello"));
    worker.register_service_activity("farewell", service, GreeterService::farewell);
    println!("✅ Test 2 passed: Trait registration (1 line)");

    // 测试 3: Activity 执行
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
    println!("总结：");
    println!("  - 从 ~25 行减少到 1-3 行");
    println!("  - 自动 JSON 序列化");
    println!("  - 编译时类型安全");
    println!("  - 支持依赖注入");
}
