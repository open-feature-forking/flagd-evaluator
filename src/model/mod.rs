//! Models for flagd feature flag configuration parsing.
//!
//! This module provides data structures for working with flagd feature flag configurations
//! according to the [flagd specification](https://flagd.dev/reference/flag-definitions/).

mod feature_flag;

pub use feature_flag::{FeatureFlag, ParsingResult};
