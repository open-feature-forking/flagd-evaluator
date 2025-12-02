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
//! - `StartsWithOperator`: String prefix matching
//! - `EndsWithOperator`: String suffix matching
//! - `SemVerOperator`: Semantic version comparison
//!
//! ## Module Organization
//!
//! Each operator is implemented in its own file for easier maintenance:
//! - `common.rs`: Shared utilities and helper functions
//! - `fractional.rs`: Fractional/percentage-based bucket assignment
//! - `starts_with.rs`: String prefix matching
//! - `ends_with.rs`: String suffix matching
//! - `sem_ver.rs`: Semantic version comparison

mod common;
mod ends_with;
mod fractional;
mod sem_ver;
mod starts_with;

pub use ends_with::{ends_with, EndsWithOperator};
pub use fractional::{fractional, FractionalOperator};
pub use sem_ver::{sem_ver, SemVer, SemVerOperator};
pub use starts_with::{starts_with, StartsWithOperator};

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
/// - `starts_with`: For string prefix matching
/// - `ends_with`: For string suffix matching
/// - `sem_ver`: For semantic version comparison
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
    logic.add_operator("starts_with".to_string(), Box::new(StartsWithOperator));
    logic.add_operator("ends_with".to_string(), Box::new(EndsWithOperator));
    logic.add_operator("sem_ver".to_string(), Box::new(SemVerOperator));

    logic
}
