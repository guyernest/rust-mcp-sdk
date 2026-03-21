//! State extractor for MCP tools.
//!
//! Provides shared state injection for standalone `#[mcp_tool]` functions,
//! similar to Axum's `State<T>` extractor.

use std::ops::Deref;
use std::sync::Arc;

/// State extractor for `#[mcp_tool]` functions.
///
/// Wraps shared state in `Arc<T>` and auto-derefs to `&T`, eliminating
/// the manual `Arc::clone()` + `move` closure ceremony.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::State;
///
/// #[mcp_tool(description = "Query database")]
/// async fn query(args: QueryArgs, db: State<Database>) -> Result<Value> {
///     db.execute(&args.sql).await  // auto-deref to &Database
/// }
///
/// // At registration:
/// server_builder.tool("query", query().with_state(my_db))
/// ```
#[derive(Debug)]
pub struct State<T>(pub Arc<T>);

// Manual Clone impl avoids requiring T: Clone (Arc<T> is always Clone).
impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        State(Arc::clone(&self.0))
    }
}

impl<T> Deref for State<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> From<Arc<T>> for State<T> {
    fn from(arc: Arc<T>) -> Self {
        State(arc)
    }
}

impl<T> From<T> for State<T> {
    fn from(val: T) -> Self {
        State(Arc::new(val))
    }
}

impl<T> AsRef<T> for State<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestDb {
        name: String,
    }

    impl TestDb {
        fn get_name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_deref_works() {
        let db = TestDb {
            name: "test_db".to_string(),
        };
        let state = State(Arc::new(db));
        assert_eq!(state.get_name(), "test_db");
    }

    #[test]
    fn test_from_arc() {
        let arc = Arc::new(TestDb {
            name: "from_arc".to_string(),
        });
        let state: State<TestDb> = State::from(arc);
        assert_eq!(state.get_name(), "from_arc");
    }

    #[test]
    fn test_from_value() {
        let db = TestDb {
            name: "from_value".to_string(),
        };
        let state: State<TestDb> = State::from(db);
        assert_eq!(state.get_name(), "from_value");
    }

    #[test]
    fn test_clone_arc_semantics() {
        let state1 = State(Arc::new(TestDb {
            name: "shared".to_string(),
        }));
        let state2 = state1.clone();
        // Both point to the same allocation
        assert!(Arc::ptr_eq(&state1.0, &state2.0));
        assert_eq!(state2.get_name(), "shared");
    }

    #[test]
    fn test_as_ref() {
        let state = State(Arc::new(TestDb {
            name: "as_ref".to_string(),
        }));
        let db_ref: &TestDb = state.as_ref();
        assert_eq!(db_ref.get_name(), "as_ref");
    }
}
