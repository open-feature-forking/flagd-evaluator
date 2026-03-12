//! Fractional operator for percentage-based bucket assignment.
//!
//! The fractional operator uses consistent hashing to assign users to buckets
//! for A/B testing scenarios.

use super::common::OperatorResult;
use datalogic_rs::{ContextStack, Error as DataLogicError, Evaluator, Operator};
use murmurhash3::murmurhash3_x86_32;
use serde_json::Value;

/// Custom operator for fractional/percentage-based bucket assignment.
///
/// The fractional operator uses consistent hashing to assign users to buckets
/// for A/B testing scenarios.
pub struct FractionalOperator;

impl Operator for FractionalOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.is_empty() {
            return Err(DataLogicError::InvalidArguments(
                "fractional operator requires at least one bucket definition".into(),
            ));
        }

        // Evaluate the first argument to determine bucketing key logic
        let evaluated_first = evaluator.evaluate(&args[0], context)?;
        let (bucket_key, start_index) = if let Value::String(s) = &evaluated_first {
            // Explicit bucketing key provided
            (s.clone(), 1)
        } else {
            // Fallback: use flagKey + targetingKey from context data
            let data = context.root().data().clone();
            let targeting_key = data
                .get("targetingKey")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let flag_key = data
                .get("$flagd")
                .and_then(|v| v.get("flagKey"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (format!("{}{}", flag_key, targeting_key), 0)
        };

        // Parse bucket definitions from remaining arguments
        let mut bucket_values: Vec<Value> = Vec::new();

        if start_index == 1 && args.len() == 2 {
            // Single array format: ["key", ["bucket1", 50, "bucket2", 50]]
            let evaluated_buckets = evaluator.evaluate(&args[1], context)?;
            if let Some(arr) = evaluated_buckets.as_array() {
                bucket_values.extend_from_slice(arr);
            } else {
                return Err(DataLogicError::InvalidArguments(
                    "Second argument must be an array of bucket definitions".into(),
                ));
            }
        } else {
            // Multiple array format: ["key", ["bucket1", 50], ["bucket2", 50]]
            // or shorthand: [["bucket1"], ["bucket2", weight]]
            for arg in &args[start_index..] {
                let evaluated = evaluator.evaluate(arg, context)?;
                if let Some(bucket_def) = evaluated.as_array() {
                    // Each bucket is [name, weight] or [name] (weight=1)
                    if bucket_def.len() >= 2 {
                        bucket_values.push(bucket_def[0].clone());
                        bucket_values.push(bucket_def[1].clone());
                    } else if bucket_def.len() == 1 {
                        // Shorthand: [name] implies weight of 1
                        bucket_values.push(bucket_def[0].clone());
                        bucket_values.push(Value::Number(1.into()));
                    }
                } else {
                    return Err(DataLogicError::InvalidArguments(format!(
                        "Bucket definition must be an array, got: {:?}",
                        evaluated
                    )));
                }
            }
        }

        match fractional(&bucket_key, &bucket_values) {
            Ok(bucket_name) => Ok(Value::String(bucket_name)),
            Err(e) => Err(DataLogicError::Custom(e)),
        }
    }
}

/// Evaluates the fractional operator for consistent bucket assignment.
///
/// The fractional operator takes a bucket key (typically a user ID) and
/// a list of bucket definitions with integer weights. It uses consistent hashing
/// to always assign the same bucket key to the same bucket.
///
/// # Algorithm
///
/// Uses high-resolution integer arithmetic instead of float-based percentage
/// bucketing. The MurmurHash3 value is mapped uniformly into `[0, totalWeight)`
/// using a single multiply-and-shift:
///
/// ```text
/// bucket = (u64(hash) * u64(totalWeight)) >> 32
/// ```
///
/// Weights must be non-negative integers summing to at most `i32::MAX`
/// (2,147,483,647).
///
/// # Arguments
/// * `bucket_key` - The key to use for bucket assignment (e.g., user ID)
/// * `buckets` - Array of [name, weight, name, weight, ...] values
///
/// # Returns
/// The name of the selected bucket, or an error if the input is invalid
///
/// # Example
/// ```json
/// {"fractional": ["user123", ["control", 50, "treatment", 50]]}
/// ```
/// This will consistently assign "user123" to either "control" or "treatment"
/// based on its hash value.
pub fn fractional(bucket_key: &str, buckets: &[Value]) -> Result<String, String> {
    if buckets.is_empty() {
        return Err("Fractional operator requires at least one bucket".to_string());
    }

    // Parse bucket definitions: [name1, weight1, name2, weight2, ...]
    let mut bucket_defs: Vec<(String, u64)> = Vec::new();
    let mut total_weight: u64 = 0;

    let mut i = 0;
    while i < buckets.len() {
        // Get bucket name
        let name = match &buckets[i] {
            Value::String(s) => s.clone(),
            _ => return Err(format!("Bucket name at index {} must be a string", i)),
        };

        i += 1;

        // Get bucket weight
        if i >= buckets.len() {
            return Err(format!("Missing weight for bucket '{}'", name));
        }

        let weight = match &buckets[i] {
            Value::Number(n) => n.as_u64().ok_or_else(|| {
                format!("Weight for bucket '{}' must be a positive integer", name)
            })?,
            _ => return Err(format!("Weight for bucket '{}' must be a number", name)),
        };

        total_weight = total_weight
            .checked_add(weight)
            .ok_or_else(|| "Total weight overflow".to_string())?;

        bucket_defs.push((name, weight));
        i += 1;
    }

    if bucket_defs.is_empty() {
        return Err("No valid bucket definitions found".to_string());
    }

    if total_weight == 0 {
        return Err("Total weight must be greater than zero".to_string());
    }

    // Weights must not exceed MaxInt32 to ensure safe integer arithmetic
    if total_weight > i32::MAX as u64 {
        return Err(format!(
            "Total weight {} exceeds maximum allowed value of {}",
            total_weight,
            i32::MAX
        ));
    }

    // Hash the bucket key using MurmurHash3 (seed=0, matches Apache Commons MurmurHash3.hash32x86)
    let hash: u32 = murmurhash3_x86_32(bucket_key.as_bytes(), 0);

    // Map the 32-bit hash uniformly into [0, totalWeight) using integer arithmetic.
    // This replaces the previous float-based approach (abs(hash)/i32::MAX * 100)
    // with higher resolution and no floating-point imprecision.
    let bucket_value: u64 = (hash as u64 * total_weight) >> 32;

    // Find which bucket this value falls into by accumulating weights
    let mut cumulative_weight: u64 = 0;
    for (name, weight) in &bucket_defs {
        cumulative_weight += weight;
        if bucket_value < cumulative_weight {
            return Ok(name.clone());
        }
    }

    // Unreachable for valid inputs: bucket_value < total_weight is always true
    // since (hash * total_weight) >> 32 < total_weight. Fall back defensively.
    Ok(bucket_defs.last().unwrap().0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_fractional_50_50_consistent() {
        let buckets = vec![json!("control"), json!(50), json!("treatment"), json!(50)];

        // Same key must always yield the same bucket
        let result1 = fractional("user-123", &buckets).unwrap();
        let result2 = fractional("user-123", &buckets).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_fractional_50_50_both_reachable() {
        let buckets = vec![json!("control"), json!(50), json!("treatment"), json!(50)];

        let mut seen_control = false;
        let mut seen_treatment = false;
        for i in 0..100 {
            match fractional(&format!("user-{}", i), &buckets)
                .unwrap()
                .as_str()
            {
                "control" => seen_control = true,
                "treatment" => seen_treatment = true,
                other => panic!("Unexpected bucket: {}", other),
            }
        }
        assert!(seen_control, "control bucket must be reachable");
        assert!(seen_treatment, "treatment bucket must be reachable");
    }

    #[test]
    fn test_fractional_unequal_weights() {
        let buckets = vec![json!("small"), json!(10), json!("large"), json!(90)];

        let mut small_count = 0u32;
        let mut large_count = 0u32;
        for i in 0..1000 {
            match fractional(&format!("user-{}", i), &buckets)
                .unwrap()
                .as_str()
            {
                "small" => small_count += 1,
                "large" => large_count += 1,
                other => panic!("Unexpected bucket: {}", other),
            }
        }
        // 90/10 split — large should dominate
        assert!(
            large_count > small_count * 3,
            "large ({}) should dominate small ({})",
            large_count,
            small_count
        );
    }

    #[test]
    fn test_fractional_high_resolution_weights() {
        // Weights well above 100 — impossible with old float/percentage approach
        let buckets = vec![
            json!("a"),
            json!(1000),
            json!("b"),
            json!(1000),
            json!("c"),
            json!(1000),
        ];
        let mut counts = std::collections::HashMap::new();
        for i in 0..3000 {
            let r = fractional(&format!("u-{}", i), &buckets).unwrap();
            *counts.entry(r).or_insert(0u32) += 1;
        }
        for bucket in ["a", "b", "c"] {
            let c = counts.get(bucket).copied().unwrap_or(0);
            // Each should get roughly 1/3; allow generous tolerance
            assert!(
                c > 500 && c < 1500,
                "bucket '{}' got {} assignments (expected ~1000)",
                bucket,
                c
            );
        }
    }

    #[test]
    fn test_fractional_max_int32_total_weight() {
        // Total weight exactly at the MaxInt32 boundary must succeed
        let half = i32::MAX as u64 / 2;
        let remainder = i32::MAX as u64 - half * 2;
        let buckets = vec![
            json!("a"),
            Value::Number(serde_json::Number::from(half)),
            json!("b"),
            Value::Number(serde_json::Number::from(half + remainder)),
        ];
        let result = fractional("any-key", &buckets);
        assert!(result.is_ok(), "MaxInt32 total weight must be accepted");
    }

    #[test]
    fn test_fractional_exceeds_max_int32_rejected() {
        let over: u64 = i32::MAX as u64 + 1;
        let buckets = vec![json!("a"), Value::Number(serde_json::Number::from(over))];
        let result = fractional("any-key", &buckets);
        assert!(result.is_err(), "Total weight > MaxInt32 must be rejected");
        assert!(
            result.unwrap_err().contains("exceeds maximum"),
            "Error message should mention maximum"
        );
    }

    #[test]
    fn test_fractional_empty_buckets() {
        assert!(fractional("user-123", &[]).is_err());
    }

    #[test]
    fn test_fractional_missing_weight() {
        let buckets = vec![json!("only-name")];
        assert!(fractional("user-123", &buckets).is_err());
    }

    #[test]
    fn test_fractional_invalid_name_type() {
        let buckets = vec![json!(123), json!(50)];
        assert!(fractional("user-123", &buckets).is_err());
    }

    #[test]
    fn test_fractional_invalid_weight_type() {
        let buckets = vec![json!("bucket"), json!("not-a-number")];
        assert!(fractional("user-123", &buckets).is_err());
    }

    #[test]
    fn test_fractional_single_bucket() {
        let buckets = vec![json!("only"), json!(100)];
        assert_eq!(fractional("any-key", &buckets).unwrap(), "only");
        assert_eq!(fractional("another-key", &buckets).unwrap(), "only");
    }

    #[test]
    fn test_fractional_integer_arithmetic_matches_spec() {
        // Verify the formula: bucket_value = (hash as u64 * totalWeight) >> 32
        // for a known hash, so the algorithm is pinned against regression.
        let key = "test-key";
        let hash = murmurhash3_x86_32(key.as_bytes(), 0);
        let total_weight: u64 = 100;
        let expected_bucket_value = (hash as u64 * total_weight) >> 32;

        let buckets = vec![json!("low"), json!(50u64), json!("high"), json!(50u64)];
        let result = fractional(key, &buckets).unwrap();

        let expected = if expected_bucket_value < 50 {
            "low"
        } else {
            "high"
        };
        assert_eq!(
            result, expected,
            "bucket assignment must match the spec formula"
        );
    }
}
