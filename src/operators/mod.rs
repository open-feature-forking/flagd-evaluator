//! Custom operators for JSON Logic evaluation.
//!
//! This module provides custom operators that extend the base JSON Logic
//! functionality, specifically for feature flag evaluation use cases.
//!
//! ## Operator Trait Implementation
//!
//! All custom operators implement the `datalogic_rs::Operator` trait, allowing
//! them to be registered with the DataLogic engine for seamless evaluation.
//!
//! ## Available Operators
//!
//! - `FractionalOperator`: Percentage-based bucket assignment for A/B testing
//! - `SemVerOperator`: Semantic version comparison
//!
//! ## Module Organization
//!
//! Each operator is implemented in its own file for easier maintenance:
//! - `common.rs`: Shared utilities and helper functions
//! - `fractional.rs`: Fractional/percentage-based bucket assignment
//! - `sem_ver.rs`: Semantic version comparison

mod common;
mod fractional;
mod sem_ver;

pub use fractional::{FractionalOperator};
pub use sem_ver::{SemVer, SemVerOperator};

use datalogic_rs::DataLogic;

/// Creates a new DataLogic instance with all custom operators registered.
///
/// This function initializes the DataLogic engine and registers all flagd-specific
/// custom operators. Use this instead of `DataLogic::new()` when you need access
/// to the custom operators.
///
/// # Returns
///
/// A configured DataLogic instance with the following operators registered:
/// - `fractional`: For A/B testing bucket assignment
/// - `sem_ver`: For semantic version comparison
///
/// Note: The `starts_with` and `ends_with` operators are provided by datalogic-rs
/// and are available by default without custom registration.
///
/// # Example
///
/// ```rust
/// use flagd_evaluator::operators::create_evaluator;
///
/// let engine = create_evaluator();
/// // Now you can use custom operators in your rules
/// ```
pub fn create_evaluator() -> DataLogic {
    let mut logic = DataLogic::new();

    logic.add_operator("fractional".to_string(), Box::new(FractionalOperator));
    logic.add_operator("sem_ver".to_string(), Box::new(SemVerOperator));

    logic
}
