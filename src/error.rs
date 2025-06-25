//! Defines a more readable error type
use core::error::Error;

/// Custom error type
pub type BoxedError = Box<dyn Error + Send + Sync>;
