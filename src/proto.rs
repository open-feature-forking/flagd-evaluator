//! Protobuf module for binary protocol support.
//!
//! This module provides protobuf-based serialization for evaluation results,
//! offering better performance than JSON serialization.

use crate::types::{ErrorCode, EvaluationResult, ResolutionReason};
use prost::Message;
use serde_json::Value as JsonValue;

// Include the generated protobuf code
pub mod evaluation {
    include!(concat!(env!("OUT_DIR"), "/flagd.evaluation.rs"));
}

use evaluation::{value::Kind, EvaluationResult as ProtoResult, Reason, Value as ProtoValue};

impl From<&ResolutionReason> for i32 {
    fn from(reason: &ResolutionReason) -> Self {
        match reason {
            ResolutionReason::Static => Reason::Static as i32,
            ResolutionReason::Default => Reason::Default as i32,
            ResolutionReason::TargetingMatch => Reason::TargetingMatch as i32,
            ResolutionReason::Disabled => Reason::Disabled as i32,
            ResolutionReason::Error => Reason::Error as i32,
            ResolutionReason::FlagNotFound => Reason::FlagNotFound as i32,
            ResolutionReason::Fallback => Reason::Fallback as i32,
        }
    }
}

impl From<&ErrorCode> for i32 {
    fn from(code: &ErrorCode) -> Self {
        match code {
            ErrorCode::FlagNotFound => evaluation::ErrorCode::FlagNotFound as i32,
            ErrorCode::ParseError => evaluation::ErrorCode::ParseError as i32,
            ErrorCode::TypeMismatch => evaluation::ErrorCode::TypeMismatch as i32,
            ErrorCode::General => evaluation::ErrorCode::General as i32,
        }
    }
}

/// Converts a serde_json::Value to a protobuf Value.
fn json_to_proto_value(value: &JsonValue) -> ProtoValue {
    let kind = match value {
        JsonValue::Null => None,
        JsonValue::Bool(b) => Some(Kind::BoolValue(*b)),
        JsonValue::String(s) => Some(Kind::StringValue(s.clone())),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Kind::IntValue(i))
            } else if let Some(f) = n.as_f64() {
                Some(Kind::DoubleValue(f))
            } else {
                // Fallback to JSON string for unusual numbers
                Some(Kind::JsonValue(value.to_string()))
            }
        }
        JsonValue::Array(_) | JsonValue::Object(_) => {
            // Complex types are serialized as JSON strings
            Some(Kind::JsonValue(value.to_string()))
        }
    };
    ProtoValue { kind }
}

impl EvaluationResult {
    /// Converts the evaluation result to a protobuf message.
    pub fn to_proto(&self) -> ProtoResult {
        ProtoResult {
            value: Some(json_to_proto_value(&self.value)),
            variant: self.variant.clone().unwrap_or_default(),
            reason: i32::from(&self.reason),
            error_code: self
                .error_code
                .as_ref()
                .map(|c| i32::from(c))
                .unwrap_or(0),
            error_message: self.error_message.clone().unwrap_or_default(),
            metadata_json: self
                .flag_metadata
                .as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_default())
                .unwrap_or_default(),
        }
    }

    /// Serializes the evaluation result to protobuf bytes.
    pub fn to_proto_bytes(&self) -> Vec<u8> {
        self.to_proto().encode_to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bool_value_conversion() {
        let result = EvaluationResult::static_result(json!(true), "on".to_string());
        let proto = result.to_proto();

        assert_eq!(proto.variant, "on");
        assert_eq!(proto.reason, Reason::Static as i32);
        assert!(matches!(
            proto.value.unwrap().kind,
            Some(Kind::BoolValue(true))
        ));
    }

    #[test]
    fn test_string_value_conversion() {
        let result = EvaluationResult::targeting_match(json!("hello"), "greeting".to_string());
        let proto = result.to_proto();

        assert_eq!(proto.variant, "greeting");
        assert_eq!(proto.reason, Reason::TargetingMatch as i32);
        assert!(matches!(
            proto.value.unwrap().kind,
            Some(Kind::StringValue(ref s)) if s == "hello"
        ));
    }

    #[test]
    fn test_int_value_conversion() {
        let result = EvaluationResult::static_result(json!(42), "answer".to_string());
        let proto = result.to_proto();

        assert!(matches!(
            proto.value.unwrap().kind,
            Some(Kind::IntValue(42))
        ));
    }

    #[test]
    fn test_double_value_conversion() {
        let result = EvaluationResult::static_result(json!(3.14), "pi".to_string());
        let proto = result.to_proto();

        if let Some(Kind::DoubleValue(v)) = proto.value.unwrap().kind {
            assert!((v - 3.14).abs() < 0.001);
        } else {
            panic!("Expected DoubleValue");
        }
    }

    #[test]
    fn test_object_value_conversion() {
        let result =
            EvaluationResult::static_result(json!({"key": "value"}), "config".to_string());
        let proto = result.to_proto();

        if let Some(Kind::JsonValue(s)) = proto.value.unwrap().kind {
            assert!(s.contains("key"));
            assert!(s.contains("value"));
        } else {
            panic!("Expected JsonValue");
        }
    }

    #[test]
    fn test_error_conversion() {
        let result = EvaluationResult::error(ErrorCode::ParseError, "Something went wrong");
        let proto = result.to_proto();

        assert_eq!(proto.reason, Reason::Error as i32);
        assert_eq!(proto.error_code, evaluation::ErrorCode::ParseError as i32);
        assert_eq!(proto.error_message, "Something went wrong");
    }

    #[test]
    fn test_proto_bytes_roundtrip() {
        let result = EvaluationResult::static_result(json!(true), "on".to_string());
        let bytes = result.to_proto_bytes();

        // Decode the bytes back
        let decoded = ProtoResult::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.variant, "on");
        assert_eq!(decoded.reason, Reason::Static as i32);
    }

    #[test]
    fn test_metadata_conversion() {
        use std::collections::HashMap;

        let mut metadata = HashMap::new();
        metadata.insert("scope".to_string(), json!("test"));

        let result =
            EvaluationResult::static_result(json!(true), "on".to_string()).with_metadata(metadata);
        let proto = result.to_proto();

        assert!(!proto.metadata_json.is_empty());
        assert!(proto.metadata_json.contains("scope"));
    }
}
