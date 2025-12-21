//! Fractional operator for percentage-based bucket assignment.
//!
//! The fractional operator uses consistent hashing to assign users to buckets
//! for A/B testing scenarios.

use datalogic_rs::{ContextStack, Error as DataLogicError, Evaluator, Operator};
use serde_json::Value;

use super::common::OperatorResult;

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

        // Check if first arg is a bucket definition (array) or a bucket key
        let evaluated_first = evaluator.evaluate(&args[0], context)?;
        let (bucket_key, start_index) = if evaluated_first.is_array() {
            // Shorthand format: [["bucket1"], ["bucket2", weight]]
            // Use targetingKey from context
            let root_ref = context.root();
            let data = root_ref.data();
            let targeting_key = data
                .get("targetingKey")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (targeting_key.to_string(), 0)
        } else {
            // Explicit key format: [key, ["bucket1", 50], ["bucket2", 50]]
            let key = match evaluated_first {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Null => String::new(),
                _ => return Err(DataLogicError::TypeError(
                    "Bucket key must evaluate to a string or number".into(),
                )),
            };
            (key, 1)
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
                    return Err(DataLogicError::InvalidArguments(
                        format!("Bucket definition must be an array, got: {:?}", evaluated),
                    ));
                }
            }
        }

        match fractional(&bucket_key, &bucket_values) {
            Ok(bucket_name) => Ok(Value::String(bucket_name)),
            Err(e) => Err(DataLogicError::Custom(e)),
        }
    }
}

/// MurmurHash3 32-bit implementation for consistent hashing.
///
/// This is a simplified implementation of MurmurHash3 that provides
/// good distribution for our use case. It's used by the fractional
/// operator to consistently assign users to buckets.
///
/// # Arguments
/// * `key` - The byte slice to hash
/// * `seed` - Seed value for the hash
///
/// # Returns
/// A 32-bit hash value
fn murmurhash3_32(key: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e2d51;
    const C2: u32 = 0x1b873593;
    const R1: u32 = 15;
    const R2: u32 = 13;
    const M: u32 = 5;
    const N: u32 = 0xe6546b64;

    let mut hash = seed;
    let len = key.len();
    let n_blocks = len / 4;

    // Process 4-byte chunks
    for i in 0..n_blocks {
        let mut k =
            u32::from_le_bytes([key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3]]);

        k = k.wrapping_mul(C1);
        k = k.rotate_left(R1);
        k = k.wrapping_mul(C2);

        hash ^= k;
        hash = hash.rotate_left(R2);
        hash = hash.wrapping_mul(M).wrapping_add(N);
    }

    // Process remaining bytes
    let tail = &key[n_blocks * 4..];
    let mut k1: u32 = 0;

    if tail.len() >= 3 {
        k1 ^= (tail[2] as u32) << 16;
    }
    if tail.len() >= 2 {
        k1 ^= (tail[1] as u32) << 8;
    }
    if !tail.is_empty() {
        k1 ^= tail[0] as u32;
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(R1);
        k1 = k1.wrapping_mul(C2);
        hash ^= k1;
    }

    // Finalization
    hash ^= len as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85ebca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash ^= hash >> 16;

    hash
}

/// Evaluates the fractional operator for consistent bucket assignment.
///
/// The fractional operator takes a bucket key (typically a user ID) and
/// a list of bucket definitions with percentages. It uses consistent hashing
/// to always assign the same bucket key to the same bucket.
///
/// # Arguments
/// * `bucket_key` - The key to use for bucket assignment (e.g., user ID)
/// * `buckets` - Array of [name, percentage, name, percentage, ...] values
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
    let hash = murmurhash3_32(bucket_key.as_bytes(), 0);

    // Convert to signed int32 to match reference implementation
    let hash_i32 = hash as i32;

    // Calculate hash ratio: abs(hash) / MaxInt32 to get value in [0.0, 1.0]
    let hash_ratio = (hash_i32.abs() as f64) / (i32::MAX as f64);

    // Map to bucket value in range [0, 100]
    let bucket_value = (hash_ratio * 100.0).floor() as u32;

    // Find which bucket this value falls into by accumulating weights
    let mut cumulative_weight: u32 = 0;
    for (name, weight) in &bucket_defs {
        cumulative_weight += weight;
        if bucket_value < cumulative_weight {
            return Ok(name.clone());
        }
    }

    // If we didn't find a bucket (e.g., total_weight < 100), return the last one
    Ok(bucket_defs.last().map(|(name, _)| name.clone()).unwrap_or_else(|| "".to_string()))
}

// TODO: Evaluate using an existing MurmurHash3 crate (e.g., `murmur3`, `fasthash`, or `twox-hash`)
// instead of the custom implementation. This would reduce maintenance burden and potentially
// provide better performance or additional features. Consider compatibility with WASM targets
// when selecting a crate.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_murmurhash3_consistency() {
        // Same input should always produce same output
        let hash1 = murmurhash3_32(b"test-key", 0);
        let hash2 = murmurhash3_32(b"test-key", 0);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_murmurhash3_different_inputs() {
        let hash1 = murmurhash3_32(b"key1", 0);
        let hash2 = murmurhash3_32(b"key2", 0);
        assert_ne!(hash1, hash2);
    }

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
