//! Models for flagd feature flag configuration parsing.
//!
//! This module provides data structures for working with flagd feature flag configurations
//! according to the [flagd specification](https://flagd.dev/reference/flag-definitions/).

mod feature_flag;

pub use feature_flag::{FeatureFlag, ParsingResult};

use crate::types::EvaluationResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from updating flag state indicating which flags have changed.
///
/// This is used for PROVIDER_CONFIGURATION_CHANGED events per the provider spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStateResponse {
    /// Whether the update was successful
    pub success: bool,

    /// Error message if the update failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// List of flag keys that were changed (added, removed, or mutated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_flags: Option<Vec<String>>,

    /// Pre-evaluated results for static and disabled flags.
    ///
    /// These flags don't require targeting evaluation, so their results are
    /// computed during `update_state()` to allow host-side caching and avoid
    /// WASM boundary overhead on every evaluation call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_evaluated: Option<HashMap<String, EvaluationResult>>,
}
