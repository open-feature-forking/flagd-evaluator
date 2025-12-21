//! Integration tests for the flagd-evaluator library.
//!
//! These tests verify the complete evaluation flow including memory management,
//! JSON parsing, custom operators, and error handling.

use flagd_evaluator::{alloc, dealloc, pack_ptr_len, unpack_ptr_len};


// ============================================================================
// Memory Management
// ============================================================================

#[test]
fn test_alloc_dealloc() {
    let ptr = alloc(100);
    assert!(!ptr.is_null());
    dealloc(ptr, 100);
}

#[test]
fn test_alloc_zero_bytes() {
    let ptr = alloc(0);
    assert!(ptr.is_null());
}

#[test]
fn test_multiple_allocations() {
    let mut pointers = Vec::new();

    for size in [10, 100, 1000, 10000] {
        let ptr = alloc(size);
        assert!(!ptr.is_null());
        pointers.push((ptr, size));
    }

    for (ptr, size) in pointers {
        dealloc(ptr, size);
    }
}

#[test]
fn test_pack_unpack_ptr_len() {
    let original_ptr = 0x12345678 as *const u8;
    let original_len = 999u32;

    let packed = pack_ptr_len(original_ptr, original_len);
    let (unpacked_ptr, unpacked_len) = unpack_ptr_len(packed);

    assert_eq!(unpacked_ptr, original_ptr);
    assert_eq!(unpacked_len, original_len);
}


// ============================================================================
// update_state integration tests
// ============================================================================

#[test]
fn test_update_state_success() {
    let config = r#"{
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                }
            }
        }
    }"#;

    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(response.success);

    // Verify the state was actually stored
    let state = flagd_evaluator::storage::get_flag_state();
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state.flags.len(), 1);
    assert!(state.flags.contains_key("testFlag"));
}

#[test]
fn test_update_state_invalid_json() {
    let config = "not valid json";
    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(!response.success);
    let err = response.error.unwrap();
    // Error should be JSON format with validation errors
    assert!(err.contains("Invalid JSON") || err.contains("\"valid\":false"));
}

#[test]
fn test_update_state_missing_flags_field() {
    let config = r#"{"other": "data"}"#;
    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(!response.success);
    let err = response.error.unwrap();
    // Error should indicate missing required field or invalid schema
    assert!(err.contains("\"valid\":false") || err.contains("required"));
}

#[test]
fn test_update_state_replaces_existing_state() {
    // First configuration
    let config1 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            }
        }
    }"#;
    let response = flagd_evaluator::storage::update_flag_state(config1).unwrap();
    assert!(response.success);

    // Verify first state
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    assert!(state.flags.contains_key("flag1"));

    // Second configuration should replace the first
    let config2 = r#"{
        "flags": {
            "flag2": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"off": false}
            }
        }
    }"#;
    let response = flagd_evaluator::storage::update_flag_state(config2).unwrap();
    assert!(response.success);

    // Verify state was replaced
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    assert!(!state.flags.contains_key("flag1"));
    assert!(state.flags.contains_key("flag2"));
    assert_eq!(state.flags.len(), 1);
}

#[test]
fn test_update_state_with_targeting() {
    let config = r#"{
        "flags": {
            "complexFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {
                    "on": true,
                    "off": false
                },
                "targeting": {
                    "if": [
                        {">=": [{"var": "age"}, 18]},
                        "on",
                        "off"
                    ]
                }
            }
        }
    }"#;

    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(response.success);

    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("complexFlag").unwrap();
    assert!(flag.targeting.is_some());
}

#[test]
fn test_update_state_with_metadata() {
    let config = r#"{
        "$schema": "https://flagd.dev/schema/v0/flags.json",
        "$evaluators": {
            "emailWithFaas": {
                "in": ["@faas.com", {"var": ["email"]}]
            }
        },
        "flags": {
            "myFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            }
        }
    }"#;

    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(response.success);

    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    assert!(state.flag_set_metadata.contains_key("$schema"));
    assert!(state.flag_set_metadata.contains_key("$evaluators"));
}

#[test]
fn test_update_state_empty_flags() {
    let config = r#"{"flags": {}}"#;
    let result = flagd_evaluator::storage::update_flag_state(config);
    assert!(result.is_ok());

    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    assert_eq!(state.flags.len(), 0);
}

#[test]
fn test_update_state_multiple_flags() {
    let config = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true, "off": false}
            },
            "flag2": {
                "state": "DISABLED",
                "defaultVariant": "red",
                "variants": {"red": "red", "blue": "blue"}
            },
            "flag3": {
                "state": "ENABLED",
                "defaultVariant": "default",
                "variants": {"default": {"key": "value"}}
            }
        }
    }"#;

    let result = flagd_evaluator::storage::update_flag_state(config);
    assert!(result.is_ok());

    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    assert_eq!(state.flags.len(), 3);
    assert!(state.flags.contains_key("flag1"));
    assert!(state.flags.contains_key("flag2"));
    assert!(state.flags.contains_key("flag3"));
}

#[test]
fn test_update_state_invalid_flag_structure() {
    let config = r#"{
        "flags": {
            "badFlag": {
                "state": "ENABLED"
            }
        }
    }"#;
    let response = flagd_evaluator::storage::update_flag_state(config).unwrap();
    assert!(!response.success);
    let err = response.error.unwrap();
    // Error should indicate validation failure due to missing required fields
    assert!(err.contains("\"valid\":false") || err.contains("required"));
}

// ============================================================================
// Tests for $evaluators and $ref resolution
// ============================================================================

#[test]
fn test_evaluators_simple_ref_evaluation() {
    use flagd_evaluator::evaluation::evaluate_flag;
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};
    use serde_json::json;

    clear_flag_state();

    let config = r#"{
        "$evaluators": {
            "isAdmin": {
                "in": ["admin@", {"var": "email"}]
            }
        },
        "flags": {
            "adminFeature": {
                "state": "ENABLED",
                "variants": {
                    "on": true,
                    "off": false
                },
                "defaultVariant": "off",
                "targeting": {
                    "if": [
                        {"$ref": "isAdmin"},
                        "on",
                        "off"
                    ]
                }
            }
        }
    }"#;

    // Update state
    let result = update_flag_state(config);
    assert!(result.is_ok(), "Failed to update state: {:?}", result);

    // Get the flag
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("adminFeature").unwrap();

    // Test with admin email - should return true
    let context = json!({"email": "admin@example.com"});
    let eval_result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(eval_result.value, json!(true));
    assert_eq!(eval_result.variant, Some("on".to_string()));

    // Test with non-admin email - should return false
    let context = json!({"email": "user@example.com"});
    let eval_result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(eval_result.value, json!(false));
    assert_eq!(eval_result.variant, Some("off".to_string()));
}

#[test]
fn test_evaluators_nested_ref_evaluation() {
    use flagd_evaluator::evaluation::evaluate_flag;
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};
    use serde_json::json;

    clear_flag_state();

    let config = r#"{
        "$evaluators": {
            "isAdmin": {
                "starts_with": [{"var": "email"}, "admin@"]
            },
            "isActive": {
                "==": [{"var": "status"}, "active"]
            },
            "isActiveAdmin": {
                "and": [
                    {"$ref": "isAdmin"},
                    {"$ref": "isActive"}
                ]
            }
        },
        "flags": {
            "premiumFeature": {
                "state": "ENABLED",
                "variants": {
                    "enabled": "premium",
                    "disabled": "free"
                },
                "defaultVariant": "disabled",
                "targeting": {
                    "if": [
                        {"$ref": "isActiveAdmin"},
                        "enabled",
                        "disabled"
                    ]
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("premiumFeature").unwrap();

    // Test with active admin - should return premium
    let context = json!({"email": "admin@company.com", "status": "active"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("premium"));
    assert_eq!(result.variant, Some("enabled".to_string()));

    // Test with non-admin - should return free
    let context = json!({"email": "user@company.com", "status": "active"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("free"));
    assert_eq!(result.variant, Some("disabled".to_string()));

    // Test with admin but inactive - should return free
    let context = json!({"email": "admin@company.com", "status": "inactive"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("free"));
    assert_eq!(result.variant, Some("disabled".to_string()));
}

#[test]
fn test_evaluators_with_fractional_operator() {
    use flagd_evaluator::evaluation::evaluate_flag;
    use flagd_evaluator::storage::{
        clear_flag_state, set_validation_mode, update_flag_state, ValidationMode,
    };
    use serde_json::json;

    clear_flag_state();
    // Use permissive mode since bare $ref at top level doesn't validate
    set_validation_mode(ValidationMode::Permissive);

    let config = r#"{
        "$evaluators": {
            "abTestSplit": {
                "fractional": [
                    {"var": "userId"},
                    ["control", 50],
                    ["treatment", 50]
                ]
            }
        },
        "flags": {
            "experimentFlag": {
                "state": "ENABLED",
                "variants": {
                    "control": "control-experience",
                    "treatment": "treatment-experience"
                },
                "defaultVariant": "control",
                "targeting": {
                    "$ref": "abTestSplit"
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    set_validation_mode(ValidationMode::Strict); // Reset to strict for other tests
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("experimentFlag").unwrap();

    // Test with specific user ID - should consistently return same variant
    let context = json!({"userId": "user-123"});
    let result1 = evaluate_flag(flag, &context, &state.flag_set_metadata);
    let result2 = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result1.value, result2.value);
    assert!(
        result1.value == json!("control-experience")
            || result1.value == json!("treatment-experience")
    );
}

#[test]
fn test_evaluators_complex_targeting() {
    use flagd_evaluator::evaluation::evaluate_flag;
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};
    use serde_json::json;

    clear_flag_state();

    let config = r#"{
        "$evaluators": {
            "isPremiumUser": {
                "==": [{"var": "tier"}, "premium"]
            },
            "isHighValue": {
                ">=": [{"var": "lifetime_value"}, 1000]
            },
            "isVIPUser": {
                "or": [
                    {"$ref": "isPremiumUser"},
                    {"$ref": "isHighValue"}
                ]
            }
        },
        "flags": {
            "vipFeatures": {
                "state": "ENABLED",
                "variants": {
                    "vip": {"features": ["advanced", "priority_support", "custom_reports"]},
                    "standard": {"features": ["basic"]}
                },
                "defaultVariant": "standard",
                "targeting": {
                    "if": [
                        {
                            "and": [
                                {"$ref": "isVIPUser"},
                                {"==": [{"var": "active"}, true]}
                            ]
                        },
                        "vip",
                        "standard"
                    ]
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("vipFeatures").unwrap();

    // Premium + active - should get VIP
    let context = json!({"tier": "premium", "lifetime_value": 500, "active": true});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.variant, Some("vip".to_string()));

    // High value + active - should get VIP
    let context = json!({"tier": "basic", "lifetime_value": 1500, "active": true});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.variant, Some("vip".to_string()));

    // Premium but inactive - should get standard
    let context = json!({"tier": "premium", "lifetime_value": 500, "active": false});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.variant, Some("standard".to_string()));

    // Neither premium nor high value - should get standard
    let context = json!({"tier": "basic", "lifetime_value": 100, "active": true});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.variant, Some("standard".to_string()));
}

#[test]
fn test_evaluators_missing_ref_in_storage() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    let config = r#"{
        "$evaluators": {
            "validRule": {
                "==": [{"var": "x"}, 1]
            }
        },
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "variants": {"on": true, "off": false},
                "defaultVariant": "off",
                "targeting": {
                    "$ref": "nonExistentRule"
                }
            }
        }
    }"#;

    let result = update_flag_state(config);
    let response = result.unwrap();
    assert!(!response.success);
    let err = response.error.unwrap();
    assert!(err.contains("nonExistentRule"));
}

#[test]
fn test_evaluators_multiple_refs_in_single_flag() {
    use flagd_evaluator::evaluation::evaluate_flag;
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};
    use serde_json::json;

    clear_flag_state();

    let config = r#"{
        "$evaluators": {
            "isAdmin": {
                "starts_with": [{"var": "email"}, "admin@"]
            },
            "isManager": {
                "starts_with": [{"var": "email"}, "manager@"]
            }
        },
        "flags": {
            "accessFlag": {
                "state": "ENABLED",
                "variants": {
                    "full": "full-access",
                    "limited": "limited-access",
                    "none": "no-access"
                },
                "defaultVariant": "none",
                "targeting": {
                    "if": [
                        {"$ref": "isAdmin"},
                        "full",
                        {
                            "if": [
                                {"$ref": "isManager"},
                                "limited",
                                "none"
                            ]
                        }
                    ]
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("accessFlag").unwrap();

    // Admin gets full access
    let context = json!({"email": "admin@company.com"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("full-access"));

    // Manager gets limited access
    let context = json!({"email": "manager@company.com"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("limited-access"));

    // Regular user gets no access
    let context = json!({"email": "user@company.com"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);
    assert_eq!(result.value, json!("no-access"));
}

// ============================================================================
// Tests for changed flags detection in update_state
// ============================================================================

#[test]
fn test_update_state_changed_flags_on_first_update() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    let config = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            },
            "flag2": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"off": false}
            }
        }
    }"#;

    let response = update_flag_state(config).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&"flag1".to_string()));
    assert!(changed.contains(&"flag2".to_string()));
}

#[test]
fn test_update_state_changed_flags_partial_update() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    // Initial config
    let config1 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            },
            "flag2": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"off": false}
            }
        }
    }"#;
    update_flag_state(config1).unwrap();

    // Update - modify flag1, keep flag2 same
    let config2 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"on": true}
            },
            "flag2": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"off": false}
            }
        }
    }"#;

    let response = update_flag_state(config2).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed.contains(&"flag1".to_string()));
}

#[test]
fn test_update_state_changed_flags_targeting_change() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    // Initial config
    let config1 = r#"{
        "flags": {
            "featureFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"on": true, "off": false},
                "targeting": {
                    "if": [
                        {"==": [{"var": "tier"}, "premium"]},
                        "on",
                        "off"
                    ]
                }
            }
        }
    }"#;
    update_flag_state(config1).unwrap();

    // Update with different targeting rule
    let config2 = r#"{
        "flags": {
            "featureFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"on": true, "off": false},
                "targeting": {
                    "if": [
                        {"==": [{"var": "tier"}, "enterprise"]},
                        "on",
                        "off"
                    ]
                }
            }
        }
    }"#;

    let response = update_flag_state(config2).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed.contains(&"featureFlag".to_string()));
}

#[test]
fn test_update_state_changed_flags_metadata_change() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    // Initial config
    let config1 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true},
                "metadata": {
                    "description": "Original"
                }
            }
        }
    }"#;
    update_flag_state(config1).unwrap();

    // Update with different metadata
    let config2 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true},
                "metadata": {
                    "description": "Updated"
                }
            }
        }
    }"#;

    let response = update_flag_state(config2).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed.contains(&"flag1".to_string()));
}

#[test]
fn test_update_state_changed_flags_no_changes() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    let config = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            }
        }
    }"#;

    // First update
    update_flag_state(config).unwrap();

    // Second update with same config
    let response = update_flag_state(config).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 0);
}

#[test]
fn test_update_state_changed_flags_add_and_remove() {
    use flagd_evaluator::storage::{clear_flag_state, update_flag_state};

    clear_flag_state();

    // Initial config
    let config1 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            },
            "flag2": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": {"off": false}
            }
        }
    }"#;
    update_flag_state(config1).unwrap();

    // Remove flag2, add flag3
    let config2 = r#"{
        "flags": {
            "flag1": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {"on": true}
            },
            "flag3": {
                "state": "ENABLED",
                "defaultVariant": "red",
                "variants": {"red": "red"}
            }
        }
    }"#;

    let response = update_flag_state(config2).unwrap();
    assert!(response.success);
    let changed = response.changed_flags.unwrap();
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&"flag2".to_string())); // Removed
    assert!(changed.contains(&"flag3".to_string())); // Added
    assert!(!changed.contains(&"flag1".to_string())); // Unchanged
}
