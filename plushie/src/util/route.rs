//! Navigation stack with parameters.
//!
//! A simple client-side routing stack. Each entry has a path
//! string and optional parameters. Push to navigate forward,
//! pop to go back.

use std::collections::HashMap;

/// A single entry in the navigation stack.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub path: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// A navigation stack tracking the current route and history.
///
/// The root route (first push) cannot be popped.
#[derive(Debug, Clone)]
pub struct Route {
    stack: Vec<RouteEntry>,
}

impl Route {
    /// Create a new route starting at the given path with no parameters.
    pub fn new(path: &str) -> Self {
        Self {
            stack: vec![RouteEntry {
                path: path.to_string(),
                params: HashMap::new(),
            }],
        }
    }

    /// Navigate to a new path, pushing it onto the stack.
    pub fn push(&mut self, path: &str) {
        self.push_with_params(path, HashMap::new());
    }

    /// Navigate to a new path with parameters.
    pub fn push_with_params(&mut self, path: &str, params: HashMap<String, serde_json::Value>) {
        self.stack.push(RouteEntry {
            path: path.to_string(),
            params,
        });
    }

    /// Go back one level. Returns `false` if already at the root.
    pub fn pop(&mut self) -> bool {
        if self.stack.len() <= 1 {
            return false;
        }
        self.stack.pop();
        true
    }

    /// Replace the current (top) route without changing history depth.
    pub fn replace_top(&mut self, path: &str) {
        if let Some(top) = self.stack.last_mut() {
            top.path = path.to_string();
            top.params.clear();
        }
    }

    /// The current path (top of the stack).
    pub fn current(&self) -> &str {
        &self.stack.last().expect("route stack is never empty").path
    }

    /// Parameters of the current route.
    pub fn params(&self) -> &HashMap<String, serde_json::Value> {
        &self.stack.last().expect("route stack is never empty").params
    }

    /// Whether there is a previous route to go back to.
    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// How many entries are in the stack (including root).
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}
