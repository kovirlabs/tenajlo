//! Running long operations (fetch, pull, push) and their cancellation tokens.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

type Map = Arc<Mutex<HashMap<String, CancellationToken>>>;

/// Registry of cancellable operations by id.
#[derive(Default)]
pub struct Operations {
    running: Map,
}

/// Keeps an operation registered; removes it when dropped.
pub struct OperationGuard {
    id: String,
    running: Map,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
    }
}

impl Operations {
    /// Registers `id` and returns its cancellation token.
    pub fn start(&self, id: &str) -> (CancellationToken, OperationGuard) {
        let token = CancellationToken::new();
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.to_owned(), token.clone());
        (
            token,
            OperationGuard {
                id: id.to_owned(),
                running: self.running.clone(),
            },
        )
    }

    /// Cancels a running operation. Unknown ids (already finished) are ignored.
    pub fn cancel(&self, id: &str) {
        if let Some(token) = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
        {
            token.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_and_cleanup() {
        let ops = Operations::default();
        let (token, guard) = ops.start("a");
        ops.cancel("missing");
        assert!(!token.is_cancelled());
        ops.cancel("a");
        assert!(token.is_cancelled());
        drop(guard);
        assert!(ops.running.lock().unwrap().is_empty());
    }
}
