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

/// Evaluates the fractional operator for consistent bucket assignment (internal use).
///
/// The fractional operator takes a bucket key (typically a user ID) and
/// a list of bucket definitions with percentages. It uses consistent hashing
/// to always assign the same bucket key to the same bucket.
///
/// Note: This is an internal helper. Use the `fractional` operator in JSON Logic rules instead.
pub(crate) fn fractional(bucket_key: &str, buckets: &[Value]) -> Result<String, String> {
    if buckets.is_empty() {
        return Err("Fractional operator requires at least one bucket".to_string());
    }

    // Parse bucket definitions: [name1, weight1, name2, weight2, ...]
    let mut bucket_defs: Vec<(String, u32)> = Vec::new();
    let mut total_weight: u32 = 0;

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
            Value::Number(n) => n
                .as_u64()
                .ok_or_else(|| format!("Weight for bucket '{}' must be a positive integer", name))?
                as u32,
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

    // Hash the bucket key to get a consistent value
    // Using murmurhash3_x86_32 to match Apache Commons MurmurHash3.hash32x86
    // Java code: Math.abs(mmrHash) * 1.0f / Integer.MAX_VALUE * 100
    let hash: u32 = murmurhash3_x86_32(bucket_key.as_bytes(), 0);
    let hash_i32 = hash as i32; // Cast to signed integer (may be negative)
    let abs_hash = hash_i32.abs(); // Take absolute value like Java does
    let bucket_value = (abs_hash as f64 / i32::MAX as f64) * 100.0;

    // Find which bucket this value falls into by accumulating weights
    let mut cumulative_weight: f64 = 0.;
    for (name, weight) in &bucket_defs {
        cumulative_weight += (weight * 100) as f64 / total_weight as f64;
        if bucket_value < cumulative_weight {
            return Ok(name.clone());
        }
    }

    // If we didn't find a bucket (e.g., total_weight < 100), return the last one
    Ok(bucket_defs
        .last()
        .map(|(name, _)| name.clone())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_fractional_50_50() {
        let buckets = vec![json!("control"), json!(50), json!("treatment"), json!(50)];

        // Test consistency - same key should always return same bucket
        let result1 = fractional("user-123", &buckets).unwrap();
        let result2 = fractional("user-123", &buckets).unwrap();
        assert_eq!(result1, result2);

        // Test that both buckets are reachable with different keys
        let mut seen_control = false;
        let mut seen_treatment = false;

        for i in 0..100 {
            let key = format!("test-user-{}", i);
            let result = fractional(&key, &buckets).unwrap();
            match result.as_str() {
                "control" => seen_control = true,
                "treatment" => seen_treatment = true,
                _ => panic!("Unexpected bucket: {}", result),
            }
        }

        assert!(seen_control, "control bucket should be reachable");
        assert!(seen_treatment, "treatment bucket should be reachable");
    }

    #[test]
    fn test_fractional_unequal_weights() {
        let buckets = vec![json!("small"), json!(10), json!("large"), json!(90)];

        let mut small_count = 0;
        let mut large_count = 0;

        // Run many iterations to check distribution
        for i in 0..1000 {
            let key = format!("user-{}", i);
            let result = fractional(&key, &buckets).unwrap();
            match result.as_str() {
                "small" => small_count += 1,
                "large" => large_count += 1,
                _ => panic!("Unexpected bucket"),
            }
        }

        // Large bucket should have significantly more assignments
        assert!(
            large_count > small_count * 3,
            "Large bucket should dominate"
        );
    }

    #[test]
    fn test_fractional_empty_buckets() {
        let buckets: Vec<Value> = vec![];
        let result = fractional("user-123", &buckets);
        assert!(result.is_err());
    }

    #[test]
    fn test_fractional_missing_weight() {
        let buckets = vec![json!("only-name")];
        let result = fractional("user-123", &buckets);
        assert!(result.is_err());
    }

    #[test]
    fn test_fractional_invalid_name_type() {
        let buckets = vec![json!(123), json!(50)];
        let result = fractional("user-123", &buckets);
        assert!(result.is_err());
    }

    #[test]
    fn test_fractional_invalid_weight_type() {
        let buckets = vec![json!("bucket"), json!("not-a-number")];
        let result = fractional("user-123", &buckets);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod hash_debug {
    use super::*;

    #[test]
    fn debug_hash_calculations() {
        let test_keys = vec![
            "fractional-flag-shorthandjon@company.com",
            "fractional-flag-shorthandjane@company.com",
        ];

        for key in test_keys {
            let hash_u32 = murmurhash3_x86_32(key.as_bytes(), 0);
            let hash_i32 = hash_u32 as i32;
            let abs_hash = hash_i32.abs();

            // Current method
            let bucket_current = (hash_u32 as f64 / u32::MAX as f64) * 100.0;

            // Java-style method
            let bucket_java = (abs_hash as f64 / i32::MAX as f64) * 100.0;

            println!("\nKey: {}", key);
            println!("  Hash (u32): {}", hash_u32);
            println!("  Hash (i32): {}", hash_i32);
            println!("  Abs: {}", abs_hash);
            println!("  Bucket (current u32/MAX): {:.6}", bucket_current);
            println!("  Bucket (Java abs/i32MAX): {:.6}", bucket_java);
        }
    }
}

#[test]
fn debug_shorthand_keys() {
    let flag_key = "fractional-flag-shorthand";
    let test_cases = vec![
        ("jon@company.com", "heads"),  // Expected
        ("jane@company.com", "tails"), // Expected
    ];

    for (targeting_key, expected) in test_cases {
        let bucket_key = format!("{}{}", flag_key, targeting_key);
        let hash: u32 = murmurhash3_x86_32(bucket_key.as_bytes(), 0);
        let hash_i32 = hash as i32;
        let abs_hash = hash_i32.abs();
        let bucket_value = (abs_hash as f64 / i32::MAX as f64) * 100.0;

        println!("\nTargeting Key: {}", targeting_key);
        println!("  Bucket Key: {}", bucket_key);
        println!("  Hash (u32): {}", hash);
        println!("  Hash (i32): {}", hash_i32);
        println!("  Abs: {}", abs_hash);
        println!("  Bucket Value: {:.6}", bucket_value);
        println!("  Expected: {}", expected);

        // Buckets: [("heads", 1), ("tails", 1)] total=2
        // cumulative: heads=50%, tails=100%
        let result = if bucket_value < 50.0 {
            "heads"
        } else {
            "tails"
        };
        println!("  Result: {}", result);
        println!("  Match: {}", result == expected);
    }
}
