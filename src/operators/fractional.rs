//! Fractional operator for percentage-based bucket assignment.
//!
//! The fractional operator uses consistent hashing to assign users to buckets
//! for A/B testing scenarios.

use datalogic_rs::{CustomOperator, DataArena, DataValue, EvalContext, LogicError};
use datalogic_rs::logic::Result as DataLogicResult;

use super::common::resolve_string_from_datavalue;

/// Custom operator for fractional/percentage-based bucket assignment.
///
/// The fractional operator uses consistent hashing to assign users to buckets
/// for A/B testing scenarios.
#[derive(Debug)]
pub struct FractionalOperator;

impl CustomOperator for FractionalOperator {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        _context: &EvalContext<'a>,
        arena: &'a DataArena,
    ) -> DataLogicResult<&'a DataValue<'a>> {
        if args.len() < 2 {
            return Err(LogicError::Custom(
                "fractional operator requires an array with at least 2 elements".to_string(),
            ));
        }

        // First argument is the bucket key
        let bucket_key = resolve_string_from_datavalue(&args[0])
            .map_err(|e| LogicError::Custom(format!("Failed to resolve bucket key: {}", e)))?;

        // Second argument is the buckets array
        let buckets = args[1]
            .as_array()
            .ok_or_else(|| {
                LogicError::Custom(
                    "Second argument must be an array of bucket definitions".to_string(),
                )
            })?;

        match fractional_with_datavalue(&bucket_key, buckets) {
            Ok(bucket_name) => {
                let s_arena = arena.alloc_str(&bucket_name);
                Ok(arena.alloc(DataValue::String(s_arena)))
            }
            Err(e) => Err(LogicError::Custom(e)),
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

/// Evaluates the fractional operator for consistent bucket assignment with DataValue types.
///
/// The fractional operator takes a bucket key (typically a user ID) and
/// a list of bucket definitions with percentages. It uses consistent hashing
/// to always assign the same bucket key to the same bucket.
///
/// # Arguments
/// * `bucket_key` - The key to use for bucket assignment (e.g., user ID)
/// * `buckets` - Array of [name, percentage, name, percentage, ...] values as DataValue
///
/// # Returns
/// The name of the selected bucket, or an error if the input is invalid
fn fractional_with_datavalue<'a>(bucket_key: &str, buckets: &[DataValue<'a>]) -> std::result::Result<String, String> {
    if buckets.is_empty() {
        return Err("Fractional operator requires at least one bucket".to_string());
    }

    // Parse bucket definitions: [name1, weight1, name2, weight2, ...]
    let mut bucket_defs: Vec<(String, u32)> = Vec::new();
    let mut total_weight: u32 = 0;

    let mut i = 0;
    while i < buckets.len() {
        // Get bucket name
        let name = match buckets[i] {
            DataValue::String(s) => s.to_string(),
            _ => return Err(format!("Bucket name at index {} must be a string", i)),
        };

        i += 1;

        // Get bucket weight
        if i >= buckets.len() {
            return Err(format!("Missing weight for bucket '{}'", name));
        }

        let weight = match buckets[i].as_f64() {
            Some(n) if n >= 0.0 => n as u32,
            _ => return Err(format!("Weight for bucket '{}' must be a positive number", name)),
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

    // Map hash to range [0, total_weight)
    let bucket_value = (hash as u64 * total_weight as u64 / u32::MAX as u64) as u32;

    // Find which bucket this value falls into
    let mut cumulative_weight: u32 = 0;
    for (name, weight) in bucket_defs {
        cumulative_weight += weight;
        if bucket_value < cumulative_weight {
            return Ok(name);
        }
    }

    // Should never reach here, but return last bucket as fallback
    Err("Failed to select bucket".to_string())
}

/// Evaluates the fractional operator for consistent bucket assignment.
///
/// This version works with serde_json::Value for backward compatibility with tests.
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
pub fn fractional(bucket_key: &str, buckets: &[serde_json::Value]) -> std::result::Result<String, String> {
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
            serde_json::Value::String(s) => s.clone(),
            _ => return Err(format!("Bucket name at index {} must be a string", i)),
        };

        i += 1;

        // Get bucket weight
        if i >= buckets.len() {
            return Err(format!("Missing weight for bucket '{}'", name));
        }

        let weight = match &buckets[i] {
            serde_json::Value::Number(n) => n
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

    // Map hash to range [0, total_weight)
    let bucket_value = (hash as u64 * total_weight as u64 / u32::MAX as u64) as u32;

    // Find which bucket this value falls into
    let mut cumulative_weight: u32 = 0;
    for (name, weight) in bucket_defs {
        cumulative_weight += weight;
        if bucket_value < cumulative_weight {
            return Ok(name);
        }
    }

    // Should never reach here, but return last bucket as fallback
    Err("Failed to select bucket".to_string())
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
        let buckets: Vec<serde_json::Value> = vec![];
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
