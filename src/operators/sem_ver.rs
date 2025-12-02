//! Semantic version comparison operator.
//!
//! The sem_ver operator compares semantic versions according to the semver.org specification.

use datalogic_rs::{ContextStack, Error as DataLogicError, Evaluator, Operator};
use serde_json::Value;
use std::cmp::Ordering;

use super::common::{resolve_string_from_context, OperatorResult};

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
