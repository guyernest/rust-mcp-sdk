//! Standard error codes for validation with client elicitation support
//!
//! Provides a consistent set of error codes that clients can use to
//! automate UI elicitation and provide better user experiences.

use serde_json::{json, Value};

/// Standard validation error codes for MCP tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorCode {
    /// Required field is missing
    MissingField,
    /// Field value has invalid format (e.g., email, URL)
    InvalidFormat,
    /// Numeric value is out of allowed range
    OutOfRange,
    /// Value is not in the allowed set
    NotAllowed,
    /// String is too short
    TooShort,
    /// String is too long
    TooLong,
    /// Array/collection has too few items
    TooFewItems,
    /// Array/collection has too many items
    TooManyItems,
    /// Pattern/regex mismatch
    PatternMismatch,
    /// Path traversal or security violation
    SecurityViolation,
    /// Type mismatch (expected different type)
    TypeMismatch,
    /// Custom validation failed
    CustomValidation,
}

impl ValidationErrorCode {
    /// Get the string code for this error
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::InvalidFormat => "invalid_format",
            Self::OutOfRange => "out_of_range",
            Self::NotAllowed => "not_allowed",
            Self::TooShort => "too_short",
            Self::TooLong => "too_long",
            Self::TooFewItems => "too_few_items",
            Self::TooManyItems => "too_many_items",
            Self::PatternMismatch => "pattern_mismatch",
            Self::SecurityViolation => "security_violation",
            Self::TypeMismatch => "type_mismatch",
            Self::CustomValidation => "custom_validation",
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::MissingField => "Required field is missing",
            Self::InvalidFormat => "Field value has invalid format",
            Self::OutOfRange => "Value is outside the allowed range",
            Self::NotAllowed => "Value is not in the allowed set",
            Self::TooShort => "Value is too short",
            Self::TooLong => "Value is too long",
            Self::TooFewItems => "Collection has too few items",
            Self::TooManyItems => "Collection has too many items",
            Self::PatternMismatch => "Value does not match the required pattern",
            Self::SecurityViolation => "Security constraint violated",
            Self::TypeMismatch => "Value has incorrect type",
            Self::CustomValidation => "Custom validation failed",
        }
    }
}

impl std::fmt::Display for ValidationErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured validation error with elicitation support
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The error code
    pub code: ValidationErrorCode,
    /// The field that failed validation
    pub field: String,
    /// Human-readable error message
    pub message: String,
    /// Expected format/value description
    pub expected: Option<String>,
    /// The actual value that failed (for debugging)
    pub actual: Option<Value>,
    /// Additional context for the error
    pub context: Option<Value>,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(code: ValidationErrorCode, field: impl Into<String>) -> Self {
        let field = field.into();
        let message = format!("{} for field '{}'", code.description(), field);
        Self {
            code,
            field,
            message,
            expected: None,
            actual: None,
            context: None,
        }
    }

    /// Set the expected value/format description
    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Set the actual value that failed validation
    pub fn actual(mut self, actual: impl Into<Value>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    /// Set additional context
    pub fn context(mut self, context: impl Into<Value>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set a custom message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Convert to a JSON value for elicitation
    pub fn to_json(&self) -> Value {
        let mut obj = json!({
            "code": self.code.as_str(),
            "field": self.field,
            "message": self.message,
            "elicit": true,
        });

        if let Some(expected) = &self.expected {
            obj["expected"] = json!(expected);
        }

        if let Some(actual) = &self.actual {
            obj["actual"] = actual.clone();
        }

        if let Some(context) = &self.context {
            obj["context"] = context.clone();
        }

        obj
    }

    /// Convert to an MCP Error
    pub fn to_error(&self) -> crate::Error {
        crate::Error::Validation(self.to_json().to_string())
    }
}

/// Helper trait for creating validation errors
pub trait IntoValidationError {
    /// Convert to a validation error
    fn into_validation_error(self, code: ValidationErrorCode, field: &str) -> crate::Error;
}

impl<T, E> IntoValidationError for Result<T, E>
where
    E: std::fmt::Display,
{
    fn into_validation_error(self, code: ValidationErrorCode, field: &str) -> crate::Error {
        match self {
            Ok(_) => panic!("Cannot convert Ok result to validation error"),
            Err(e) => ValidationError::new(code, field)
                .message(format!("{}: {}", code.description(), e))
                .to_error(),
        }
    }
}

// Convenience functions for common validations

/// Create a missing field error
pub fn missing_field(field: &str) -> crate::Error {
    ValidationError::new(ValidationErrorCode::MissingField, field)
        .expected("This field is required")
        .to_error()
}

/// Create an invalid format error
pub fn invalid_format(field: &str, expected: &str) -> crate::Error {
    ValidationError::new(ValidationErrorCode::InvalidFormat, field)
        .expected(expected)
        .to_error()
}

/// Create an out of range error
pub fn out_of_range<T: std::fmt::Display>(
    field: &str,
    value: T,
    min: Option<T>,
    max: Option<T>,
) -> crate::Error {
    let expected = match (min, max) {
        (Some(min), Some(max)) => format!("Value between {} and {}", min, max),
        (Some(min), None) => format!("Value >= {}", min),
        (None, Some(max)) => format!("Value <= {}", max),
        (None, None) => "Value within valid range".to_string(),
    };

    ValidationError::new(ValidationErrorCode::OutOfRange, field)
        .expected(expected)
        .actual(json!(value.to_string()))
        .to_error()
}

/// Create a not allowed error
pub fn not_allowed<T: std::fmt::Display>(field: &str, value: T, allowed: &[T]) -> crate::Error {
    let allowed_str = allowed
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    ValidationError::new(ValidationErrorCode::NotAllowed, field)
        .expected(format!("One of: {}", allowed_str))
        .actual(json!(value.to_string()))
        .to_error()
}

/// Create a pattern mismatch error
pub fn pattern_mismatch(field: &str, pattern: &str) -> crate::Error {
    ValidationError::new(ValidationErrorCode::PatternMismatch, field)
        .expected(format!("Match pattern: {}", pattern))
        .to_error()
}

/// Create a security violation error
pub fn security_violation(field: &str, reason: &str) -> crate::Error {
    ValidationError::new(ValidationErrorCode::SecurityViolation, field)
        .message(format!("Security violation: {}", reason))
        .to_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_json() {
        let error = ValidationError::new(ValidationErrorCode::OutOfRange, "age")
            .expected("18-120")
            .actual(json!(150))
            .message("Age must be between 18 and 120");

        let json = error.to_json();
        assert_eq!(json["code"], "out_of_range");
        assert_eq!(json["field"], "age");
        assert_eq!(json["expected"], "18-120");
        assert_eq!(json["actual"], 150);
        assert_eq!(json["elicit"], true);
    }

    #[test]
    fn test_convenience_functions() {
        let error = missing_field("email");
        assert!(error.to_string().contains("missing_field"));

        let error = invalid_format("email", "user@example.com");
        assert!(error.to_string().contains("invalid_format"));

        let error = out_of_range("age", 200, Some(18), Some(120));
        assert!(error.to_string().contains("out_of_range"));
    }
}
