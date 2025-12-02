//! Custom operator implementations for feature flag evaluation.
//!
//! This module contains the implementation of custom operators that extend
//! JSON Logic for feature flag use cases. These operators include:
//!
//! - `fractional`: Consistent hashing for A/B testing scenarios
//! - `starts_with`: String prefix matching
//! - `ends_with`: String suffix matching
//! - `sem_ver`: Semantic version comparison
//!
//! Each operator implements the `datalogic_rs::Operator` trait for seamless
//! integration with the DataLogic evaluation engine.

use datalogic_rs::{ContextStack, Error as DataLogicError, Evaluator, Operator};
use serde_json::Value;
use std::cmp::Ordering;

/// Type alias for operator results using datalogic_rs Error type.
type OperatorResult<T> = std::result::Result<T, DataLogicError>;

// ============================================================================
// Helper functions for variable resolution from context
// ============================================================================

/// Resolves a variable path from the context data, or returns the string value directly.
///
/// This helper function handles both direct string values and variable references
/// (like `{"var": "path.to.value"}`) for the custom operators.
fn resolve_string_from_context(
    value: &Value,
    context: &ContextStack,
) -> OperatorResult<String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Object(obj) if obj.contains_key("var") => {
            let var_path = obj
                .get("var")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DataLogicError::InvalidArguments("var reference must be a string".into())
                })?;

            // Get root data and navigate the path
            let root_ref = context.root();
            let data = root_ref.data();
            let mut current = data;
            for part in var_path.split('.') {
                current = current.get(part).ok_or_else(|| {
                    DataLogicError::VariableNotFound(format!(
                        "Variable '{}' not found in data",
                        var_path
                    ))
                })?;
            }

            match current {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) => Ok(n.to_string()),
                Value::Null => Ok(String::new()),
                _ => Err(DataLogicError::TypeError(format!(
                    "Variable '{}' must be a string or number",
                    var_path
                ))),
            }
        }
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        _ => Err(DataLogicError::InvalidArguments(
            "Value must be a string, number, null, or var reference".into(),
        )),
    }
}

// ============================================================================
// Operator trait implementations
// ============================================================================

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
        _evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.len() < 2 {
            return Err(DataLogicError::InvalidArguments(
                "fractional operator requires an array with at least 2 elements".into(),
            ));
        }

        // First argument is the bucket key (can be a value or a var reference)
        let bucket_key = resolve_string_from_context(&args[0], context)?;

        // Second argument is the buckets array
        let buckets = args[1]
            .as_array()
            .ok_or_else(|| {
                DataLogicError::InvalidArguments(
                    "Second argument must be an array of bucket definitions".into(),
                )
            })?
            .as_slice();

        match fractional(&bucket_key, buckets) {
            Ok(bucket_name) => Ok(Value::String(bucket_name)),
            Err(e) => Err(DataLogicError::Custom(e)),
        }
    }
}

/// Custom operator for string prefix matching.
///
/// Checks if a string starts with a given prefix. The comparison is case-sensitive.
pub struct StartsWithOperator;

impl Operator for StartsWithOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        _evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.len() < 2 {
            return Err(DataLogicError::InvalidArguments(
                "starts_with operator requires an array with at least 2 elements".into(),
            ));
        }

        let string_value = resolve_string_from_context(&args[0], context)?;
        let prefix = resolve_string_from_context(&args[1], context)?;

        Ok(Value::Bool(starts_with(&string_value, &prefix)))
    }
}

/// Custom operator for string suffix matching.
///
/// Checks if a string ends with a given suffix. The comparison is case-sensitive.
pub struct EndsWithOperator;

impl Operator for EndsWithOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        _evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.len() < 2 {
            return Err(DataLogicError::InvalidArguments(
                "ends_with operator requires an array with at least 2 elements".into(),
            ));
        }

        let string_value = resolve_string_from_context(&args[0], context)?;
        let suffix = resolve_string_from_context(&args[1], context)?;

        Ok(Value::Bool(ends_with(&string_value, &suffix)))
    }
}

/// Custom operator for semantic version comparison.
///
/// Compares semantic versions according to the semver.org specification.
pub struct SemVerOperator;

impl Operator for SemVerOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        _evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.len() < 3 {
            return Err(DataLogicError::InvalidArguments(
                "sem_ver operator requires an array with at least 3 elements".into(),
            ));
        }

        let version = resolve_string_from_context(&args[0], context)?;
        let operator = args[1].as_str().ok_or_else(|| {
            DataLogicError::InvalidArguments("sem_ver operator must be a string".into())
        })?;
        let target = resolve_string_from_context(&args[2], context)?;

        match sem_ver(&version, operator, &target) {
            Ok(result) => Ok(Value::Bool(result)),
            Err(e) => Err(DataLogicError::Custom(e)),
        }
    }
}

// ============================================================================
// Core implementation functions
// ============================================================================

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

/// Evaluates the starts_with operator for string prefix matching.
///
/// The starts_with operator checks if a string starts with a specific prefix.
/// The comparison is case-sensitive.
///
/// # Arguments
/// * `string_value` - The string to check
/// * `prefix` - The prefix to search for
///
/// # Returns
/// `true` if the string starts with the prefix, `false` otherwise
///
/// # Example
/// ```json
/// {"starts_with": [{"var": "email"}, "admin@"]}
/// ```
/// Returns `true` if email is "admin@example.com"
pub fn starts_with(string_value: &str, prefix: &str) -> bool {
    string_value.starts_with(prefix)
}

/// Evaluates the ends_with operator for string suffix matching.
///
/// The ends_with operator checks if a string ends with a specific suffix.
/// The comparison is case-sensitive.
///
/// # Arguments
/// * `string_value` - The string to check
/// * `suffix` - The suffix to search for
///
/// # Returns
/// `true` if the string ends with the suffix, `false` otherwise
///
/// # Example
/// ```json
/// {"ends_with": [{"var": "filename"}, ".pdf"]}
/// ```
/// Returns `true` if filename is "document.pdf"
pub fn ends_with(string_value: &str, suffix: &str) -> bool {
    string_value.ends_with(suffix)
}

/// Represents a parsed semantic version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
    pub build_metadata: Option<String>,
}

impl SemVer {
    /// Parses a semantic version string.
    ///
    /// Handles versions like:
    /// - "1.2.3"
    /// - "1.2" (treated as "1.2.0")
    /// - "1" (treated as "1.0.0")
    /// - "1.2.3-alpha.1" (with prerelease)
    /// - "1.2.3+build.123" (with build metadata)
    /// - "1.2.3-alpha.1+build.123" (with both)
    pub fn parse(version: &str) -> Result<Self, String> {
        let version = version.trim();
        if version.is_empty() {
            return Err("Version string cannot be empty".to_string());
        }

        // Remove leading 'v' or 'V' if present (common in version tags)
        let version = version
            .strip_prefix('v')
            .or_else(|| version.strip_prefix('V'))
            .unwrap_or(version);

        // Split off build metadata first (after '+')
        let (version_pre, build_metadata) = match version.split_once('+') {
            Some((v, b)) => (v, Some(b.to_string())),
            None => (version, None),
        };

        // Split off prerelease (after '-')
        let (version_core, prerelease) = match version_pre.split_once('-') {
            Some((v, p)) => (v, Some(p.to_string())),
            None => (version_pre, None),
        };

        // Parse the version core (major.minor.patch)
        let parts: Vec<&str> = version_core.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(format!("Invalid version format: {}", version));
        }

        let major = parts[0]
            .parse::<u64>()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;

        let minor = if parts.len() > 1 {
            parts[1]
                .parse::<u64>()
                .map_err(|_| format!("Invalid minor version: {}", parts[1]))?
        } else {
            0
        };

        let patch = if parts.len() > 2 {
            parts[2]
                .parse::<u64>()
                .map_err(|_| format!("Invalid patch version: {}", parts[2]))?
        } else {
            0
        };

        Ok(SemVer {
            major,
            minor,
            patch,
            prerelease,
            build_metadata,
        })
    }

    /// Compares two prerelease strings according to semver spec.
    /// Returns Ordering based on prerelease precedence.
    fn compare_prerelease(a: &Option<String>, b: &Option<String>) -> Ordering {
        match (a, b) {
            // No prerelease has higher precedence than any prerelease
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a_pre), Some(b_pre)) => {
                let a_parts: Vec<&str> = a_pre.split('.').collect();
                let b_parts: Vec<&str> = b_pre.split('.').collect();

                for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
                    let a_num = a_part.parse::<u64>();
                    let b_num = b_part.parse::<u64>();

                    let cmp = match (a_num, b_num) {
                        // Both numeric: compare numerically
                        (Ok(a_n), Ok(b_n)) => a_n.cmp(&b_n),
                        // Numeric has lower precedence than alphanumeric
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        // Both alphanumeric: compare lexically
                        (Err(_), Err(_)) => a_part.cmp(b_part),
                    };

                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }

                // If all compared parts are equal, the one with more parts is greater
                a_parts.len().cmp(&b_parts.len())
            }
        }
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major, minor, patch first
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Compare prerelease (build metadata is ignored for precedence)
        SemVer::compare_prerelease(&self.prerelease, &other.prerelease)
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Evaluates the sem_ver operator for semantic version comparison.
///
/// The sem_ver operator compares semantic versions according to the
/// [semver.org](https://semver.org/) specification.
///
/// # Arguments
/// * `version` - The version string to compare
/// * `operator` - The comparison operator ("=", "!=", "<", "<=", ">", ">=", "^", "~")
/// * `target` - The target version to compare against
///
/// # Returns
/// `true` if the comparison is satisfied, `false` otherwise
///
/// # Supported Operators
/// - `"="` - Equal to
/// - `"!="` - Not equal to
/// - `"<"` - Less than
/// - `"<="` - Less than or equal to
/// - `">"` - Greater than
/// - `">="` - Greater than or equal to
/// - `"^"` - Caret range (compatible with - allows patch and minor updates)
/// - `"~"` - Tilde range (allows patch updates only)
///
/// # Example
/// ```json
/// {"sem_ver": [{"var": "version"}, ">=", "2.0.0"]}
/// ```
/// Returns `true` if version is "2.0.0" or higher
pub fn sem_ver(version: &str, operator: &str, target: &str) -> Result<bool, String> {
    let version = SemVer::parse(version)?;
    let target = SemVer::parse(target)?;

    let result = match operator {
        "=" => version.cmp(&target) == Ordering::Equal,
        "!=" => version.cmp(&target) != Ordering::Equal,
        "<" => version.cmp(&target) == Ordering::Less,
        "<=" => version.cmp(&target) != Ordering::Greater,
        ">" => version.cmp(&target) == Ordering::Greater,
        ">=" => version.cmp(&target) != Ordering::Less,
        "^" => {
            // Caret range: >=target <next-major (or <next-minor if major is 0)
            // ^1.2.3 means >=1.2.3 <2.0.0
            // ^0.2.3 means >=0.2.3 <0.3.0
            // ^0.0.3 means >=0.0.3 <0.0.4
            if version.cmp(&target) == Ordering::Less {
                false
            } else if target.major == 0 {
                if target.minor == 0 {
                    // ^0.0.x allows only patch changes to same patch
                    version.major == 0 && version.minor == 0 && version.patch == target.patch
                } else {
                    // ^0.x.y allows patch changes, same minor
                    version.major == 0 && version.minor == target.minor
                }
            } else {
                // ^x.y.z allows minor and patch changes, same major
                version.major == target.major
            }
        }
        "~" => {
            // Tilde range: allows patch updates only
            // ~1.2.3 means >=1.2.3 <1.3.0
            if version.cmp(&target) == Ordering::Less {
                false
            } else {
                version.major == target.major && version.minor == target.minor
            }
        }
        _ => return Err(format!("Unknown operator: {}", operator)),
    };

    Ok(result)
}

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

    // ============================================================================
    // starts_with tests
    // ============================================================================

    #[test]
    fn test_starts_with_basic() {
        assert!(starts_with("hello world", "hello"));
        assert!(starts_with("admin@example.com", "admin@"));
        assert!(!starts_with("hello world", "world"));
    }

    #[test]
    fn test_starts_with_empty_prefix() {
        // Empty prefix should always return true
        assert!(starts_with("hello", ""));
        assert!(starts_with("", ""));
    }

    #[test]
    fn test_starts_with_empty_string() {
        // Non-empty prefix with empty string should return false
        assert!(!starts_with("", "hello"));
    }

    #[test]
    fn test_starts_with_case_sensitive() {
        assert!(starts_with("/api/users", "/api/"));
        assert!(!starts_with("/API/users", "/api/"));
    }

    #[test]
    fn test_starts_with_exact_match() {
        assert!(starts_with("hello", "hello"));
    }

    #[test]
    fn test_starts_with_prefix_longer_than_string() {
        assert!(!starts_with("hi", "hello"));
    }

    // ============================================================================
    // ends_with tests
    // ============================================================================

    #[test]
    fn test_ends_with_basic() {
        assert!(ends_with("hello world", "world"));
        assert!(ends_with("document.pdf", ".pdf"));
        assert!(!ends_with("hello world", "hello"));
    }

    #[test]
    fn test_ends_with_empty_suffix() {
        // Empty suffix should always return true
        assert!(ends_with("hello", ""));
        assert!(ends_with("", ""));
    }

    #[test]
    fn test_ends_with_empty_string() {
        // Non-empty suffix with empty string should return false
        assert!(!ends_with("", "hello"));
    }

    #[test]
    fn test_ends_with_case_sensitive() {
        assert!(ends_with("https://example.com", ".com"));
        assert!(!ends_with("https://example.COM", ".com"));
    }

    #[test]
    fn test_ends_with_exact_match() {
        assert!(ends_with("hello", "hello"));
    }

    #[test]
    fn test_ends_with_suffix_longer_than_string() {
        assert!(!ends_with("hi", "hello"));
    }

    // ============================================================================
    // SemVer parsing tests
    // ============================================================================

    #[test]
    fn test_semver_parse_basic() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, None);
        assert_eq!(v.build_metadata, None);
    }

    #[test]
    fn test_semver_parse_missing_parts() {
        // Missing patch
        let v = SemVer::parse("1.2").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 0);

        // Missing minor and patch
        let v = SemVer::parse("1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_semver_parse_with_prerelease() {
        let v = SemVer::parse("1.2.3-alpha.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, Some("alpha.1".to_string()));
        assert_eq!(v.build_metadata, None);
    }

    #[test]
    fn test_semver_parse_with_build_metadata() {
        let v = SemVer::parse("1.2.3+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, None);
        assert_eq!(v.build_metadata, Some("build.123".to_string()));
    }

    #[test]
    fn test_semver_parse_with_prerelease_and_build() {
        let v = SemVer::parse("1.2.3-alpha.1+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, Some("alpha.1".to_string()));
        assert_eq!(v.build_metadata, Some("build.123".to_string()));
    }

    #[test]
    fn test_semver_parse_with_v_prefix() {
        let v = SemVer::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_semver_parse_empty() {
        assert!(SemVer::parse("").is_err());
    }

    #[test]
    fn test_semver_parse_invalid() {
        assert!(SemVer::parse("not.a.version").is_err());
        assert!(SemVer::parse("1.2.3.4").is_err());
    }

    // ============================================================================
    // SemVer comparison tests
    // ============================================================================

    #[test]
    fn test_semver_comparison_basic() {
        let v1 = SemVer::parse("1.0.0").unwrap();
        let v2 = SemVer::parse("2.0.0").unwrap();
        assert!(v1 < v2);

        let v1 = SemVer::parse("1.1.0").unwrap();
        let v2 = SemVer::parse("1.2.0").unwrap();
        assert!(v1 < v2);

        let v1 = SemVer::parse("1.0.1").unwrap();
        let v2 = SemVer::parse("1.0.2").unwrap();
        assert!(v1 < v2);
    }

    #[test]
    fn test_semver_comparison_prerelease() {
        // Prerelease has lower precedence than release
        let v1 = SemVer::parse("1.0.0-alpha").unwrap();
        let v2 = SemVer::parse("1.0.0").unwrap();
        assert!(v1 < v2);

        // Compare prereleases
        let v1 = SemVer::parse("1.0.0-alpha").unwrap();
        let v2 = SemVer::parse("1.0.0-alpha.1").unwrap();
        assert!(v1 < v2);

        let v1 = SemVer::parse("1.0.0-alpha.1").unwrap();
        let v2 = SemVer::parse("1.0.0-alpha.2").unwrap();
        assert!(v1 < v2);

        let v1 = SemVer::parse("1.0.0-alpha").unwrap();
        let v2 = SemVer::parse("1.0.0-beta").unwrap();
        assert!(v1 < v2);
    }

    // ============================================================================
    // sem_ver operator tests
    // ============================================================================

    #[test]
    fn test_sem_ver_equal() {
        assert!(sem_ver("1.2.3", "=", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.3", "=", "1.2.4").unwrap());
        assert!(!sem_ver("1.2.3", "=", "2.2.3").unwrap());
    }

    #[test]
    fn test_sem_ver_not_equal() {
        assert!(sem_ver("1.2.3", "!=", "1.2.4").unwrap());
        assert!(!sem_ver("1.2.3", "!=", "1.2.3").unwrap());
    }

    #[test]
    fn test_sem_ver_less_than() {
        assert!(sem_ver("1.2.3", "<", "1.2.4").unwrap());
        assert!(sem_ver("1.2.3", "<", "1.3.0").unwrap());
        assert!(sem_ver("1.2.3", "<", "2.0.0").unwrap());
        assert!(!sem_ver("1.2.3", "<", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.4", "<", "1.2.3").unwrap());
    }

    #[test]
    fn test_sem_ver_less_than_or_equal() {
        assert!(sem_ver("1.2.3", "<=", "1.2.3").unwrap());
        assert!(sem_ver("1.2.3", "<=", "1.2.4").unwrap());
        assert!(!sem_ver("1.2.4", "<=", "1.2.3").unwrap());
    }

    #[test]
    fn test_sem_ver_greater_than() {
        assert!(sem_ver("1.2.4", ">", "1.2.3").unwrap());
        assert!(sem_ver("1.3.0", ">", "1.2.3").unwrap());
        assert!(sem_ver("2.0.0", ">", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.3", ">", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.3", ">", "1.2.4").unwrap());
    }

    #[test]
    fn test_sem_ver_greater_than_or_equal() {
        assert!(sem_ver("1.2.3", ">=", "1.2.3").unwrap());
        assert!(sem_ver("1.2.4", ">=", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.3", ">=", "1.2.4").unwrap());
    }

    #[test]
    fn test_sem_ver_caret_range() {
        // ^1.2.3 means >=1.2.3 <2.0.0
        assert!(sem_ver("1.2.3", "^", "1.2.3").unwrap());
        assert!(sem_ver("1.2.4", "^", "1.2.3").unwrap());
        assert!(sem_ver("1.9.0", "^", "1.2.3").unwrap());
        assert!(!sem_ver("2.0.0", "^", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.2", "^", "1.2.3").unwrap());

        // ^0.2.3 means >=0.2.3 <0.3.0
        assert!(sem_ver("0.2.3", "^", "0.2.3").unwrap());
        assert!(sem_ver("0.2.9", "^", "0.2.3").unwrap());
        assert!(!sem_ver("0.3.0", "^", "0.2.3").unwrap());
        assert!(!sem_ver("1.0.0", "^", "0.2.3").unwrap());

        // ^0.0.3 means >=0.0.3 <0.0.4 (very strict)
        assert!(sem_ver("0.0.3", "^", "0.0.3").unwrap());
        assert!(!sem_ver("0.0.4", "^", "0.0.3").unwrap());
        assert!(!sem_ver("0.1.0", "^", "0.0.3").unwrap());
    }

    #[test]
    fn test_sem_ver_tilde_range() {
        // ~1.2.3 means >=1.2.3 <1.3.0
        assert!(sem_ver("1.2.3", "~", "1.2.3").unwrap());
        assert!(sem_ver("1.2.9", "~", "1.2.3").unwrap());
        assert!(!sem_ver("1.3.0", "~", "1.2.3").unwrap());
        assert!(!sem_ver("1.2.2", "~", "1.2.3").unwrap());
        assert!(!sem_ver("2.0.0", "~", "1.2.3").unwrap());
    }

    #[test]
    fn test_sem_ver_with_prerelease() {
        // Prerelease versions
        assert!(sem_ver("1.0.0-alpha", "<", "1.0.0").unwrap());
        assert!(sem_ver("1.0.0-alpha", "<", "1.0.0-beta").unwrap());
        assert!(sem_ver("1.0.0", ">", "1.0.0-alpha").unwrap());
    }

    #[test]
    fn test_sem_ver_with_missing_parts() {
        // Missing parts should be treated as 0
        assert!(sem_ver("1.2", "=", "1.2.0").unwrap());
        assert!(sem_ver("1", "=", "1.0.0").unwrap());
    }

    #[test]
    fn test_sem_ver_unknown_operator() {
        assert!(sem_ver("1.2.3", "??", "1.2.3").is_err());
    }

    #[test]
    fn test_sem_ver_invalid_version() {
        assert!(sem_ver("not.a.version", "=", "1.2.3").is_err());
        assert!(sem_ver("1.2.3", "=", "not.a.version").is_err());
    }
}

// TODO: Evaluate using an existing MurmurHash3 crate (e.g., `murmur3`, `fasthash`, or `twox-hash`)
// instead of the custom implementation. This would reduce maintenance burden and potentially
// provide better performance or additional features. Consider compatibility with WASM targets
// when selecting a crate.
