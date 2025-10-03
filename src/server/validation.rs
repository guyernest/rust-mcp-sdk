//! Validation helpers for typed tools
//!
//! Provides ergonomic validation functions that return consistent Error::Validation errors
//! with optional machine-readable hints for client elicitation support.

use crate::{Error, Result};
use regex::Regex;
use serde_json::json;

/// Validation error with elicitation support
#[derive(Debug)]
pub struct ValidationError {
    /// Human-readable error message
    pub message: String,
    /// Machine-readable error code for elicitation
    pub code: Option<String>,
    /// Field that failed validation
    pub field: Option<String>,
    /// Expected format or value
    pub expected: Option<String>,
}

impl ValidationError {
    /// Create a validation error with elicitation hints
    pub fn elicit(
        code: impl Into<String>,
        field: impl Into<String>,
        expected: impl Into<String>,
    ) -> Error {
        let field_str = field.into();
        Error::Validation(
            json!({
                "message": format!("Validation failed for field '{}'", &field_str),
                "code": code.into(),
                "field": field_str,
                "expected": expected.into(),
                "elicit": true
            })
            .to_string(),
        )
    }

    /// Create a simple validation error
    pub fn simple(message: impl Into<String>) -> Error {
        Error::Validation(message.into())
    }
}

/// Validate that a value is within a range
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_range;
///
/// let age = 25;
/// validate_range("age", &age, &18, &120)?;
/// # Ok(())
/// # }
/// ```
pub fn validate_range<T>(field: &str, value: &T, min: &T, max: &T) -> Result<()>
where
    T: PartialOrd + std::fmt::Display,
{
    if value < min || value > max {
        return Err(ValidationError::elicit(
            "out_of_range",
            field,
            format!("Value must be between {} and {}", min, max),
        ));
    }
    Ok(())
}

/// Validate that a value is one of an allowed set
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_one_of;
///
/// let currency = "USD";
/// validate_one_of("currency", &currency, &["USD", "EUR", "GBP"])?;
/// # Ok(())
/// # }
/// ```
pub fn validate_one_of<T>(field: &str, value: &T, allowed: &[T]) -> Result<()>
where
    T: PartialEq + std::fmt::Display,
{
    if !allowed.contains(value) {
        let allowed_str = allowed
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ValidationError::elicit(
            "invalid_choice",
            field,
            format!("Must be one of: {}", allowed_str),
        ));
    }
    Ok(())
}

/// Validate that a string matches a regex pattern
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_regex;
///
/// let email = "user@example.com";
/// validate_regex("email", email, r"^[\w\.-]+@[\w\.-]+\.\w+$")?;
/// # Ok(())
/// # }
/// ```
pub fn validate_regex(field: &str, value: &str, pattern: &str) -> Result<()> {
    let regex = Regex::new(pattern)
        .map_err(|e| Error::Internal(format!("Invalid regex pattern '{}': {}", pattern, e)))?;

    if !regex.is_match(value) {
        return Err(ValidationError::elicit(
            "pattern_mismatch",
            field,
            format!("Must match pattern: {}", pattern),
        ));
    }
    Ok(())
}

/// Validate string length
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_length;
///
/// let name = "John Doe";
/// validate_length("name", name, Some(2), Some(100))?;
/// # Ok(())
/// # }
/// ```
pub fn validate_length(
    field: &str,
    value: &str,
    min: Option<usize>,
    max: Option<usize>,
) -> Result<()> {
    let len = value.len();

    if let Some(min_len) = min {
        if len < min_len {
            return Err(ValidationError::elicit(
                "too_short",
                field,
                format!("Minimum length is {}", min_len),
            ));
        }
    }

    if let Some(max_len) = max {
        if len > max_len {
            return Err(ValidationError::elicit(
                "too_long",
                field,
                format!("Maximum length is {}", max_len),
            ));
        }
    }

    Ok(())
}

/// Validate email format
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_email;
///
/// validate_email("email", "user@example.com")?;
/// # Ok(())
/// # }
/// ```
pub fn validate_email(field: &str, value: &str) -> Result<()> {
    // Basic email validation
    if !value.contains('@') || !value.contains('.') || value.len() < 5 {
        return Err(ValidationError::elicit(
            "invalid_email",
            field,
            "Valid email address (e.g., user@example.com)",
        ));
    }

    // More detailed validation
    let parts: Vec<&str> = value.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ValidationError::elicit(
            "invalid_email",
            field,
            "Valid email address (e.g., user@example.com)",
        ));
    }

    Ok(())
}

/// Validate URL format
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_url;
///
/// validate_url("website", "https://example.com")?;
/// # Ok(())
/// # }
/// ```
pub fn validate_url(field: &str, value: &str) -> Result<()> {
    if !value.starts_with("http://") && !value.starts_with("https://") {
        return Err(ValidationError::elicit(
            "invalid_url",
            field,
            "Valid URL starting with http:// or https://",
        ));
    }

    // Basic URL structure check
    if value.len() < 10 || !value[8..].contains('.') {
        return Err(ValidationError::elicit(
            "invalid_url",
            field,
            "Valid URL (e.g., https://example.com)",
        ));
    }

    Ok(())
}

/// Validate that a path is safe (no traversal)
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_safe_path;
///
/// validate_safe_path("filepath", "/tmp/myfile.txt", Some("/tmp/"))?;
/// # Ok(())
/// # }
/// ```
pub fn validate_safe_path(field: &str, path: &str, allowed_prefix: Option<&str>) -> Result<()> {
    // Check for path traversal
    if path.contains("..") {
        return Err(ValidationError::elicit(
            "path_traversal",
            field,
            "Path must not contain '..'",
        ));
    }

    // Check for null bytes
    if path.contains('\0') {
        return Err(ValidationError::elicit(
            "invalid_path",
            field,
            "Path must not contain null bytes",
        ));
    }

    // Check allowed prefix
    if let Some(prefix) = allowed_prefix {
        if !path.starts_with(prefix) {
            return Err(ValidationError::elicit(
                "path_not_allowed",
                field,
                format!("Path must start with '{}'", prefix),
            ));
        }
    }

    Ok(())
}

/// Validate that a value is not empty
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_required;
///
/// validate_required("username", "john_doe")?;
/// # Ok(())
/// # }
/// ```
pub fn validate_required(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(ValidationError::elicit(
            "required_field",
            field,
            "This field is required",
        ));
    }
    Ok(())
}

/// Validate array/vec size
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_array_size;
///
/// let items = vec!["a", "b", "c"];
/// validate_array_size("items", &items, Some(1), Some(10))?;
/// # Ok(())
/// # }
/// ```
pub fn validate_array_size<T>(
    field: &str,
    items: &[T],
    min: Option<usize>,
    max: Option<usize>,
) -> Result<()> {
    let len = items.len();

    if let Some(min_len) = min {
        if len < min_len {
            return Err(ValidationError::elicit(
                "too_few_items",
                field,
                format!("Minimum {} items required", min_len),
            ));
        }
    }

    if let Some(max_len) = max {
        if len > max_len {
            return Err(ValidationError::elicit(
                "too_many_items",
                field,
                format!("Maximum {} items allowed", max_len),
            ));
        }
    }

    Ok(())
}

/// Validate numeric percentage (0-100)
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::validate_percentage;
///
/// validate_percentage("discount", 25.5)?;
/// # Ok(())
/// # }
/// ```
pub fn validate_percentage(field: &str, value: f64) -> Result<()> {
    if !(0.0..=100.0).contains(&value) {
        return Err(ValidationError::elicit(
            "invalid_percentage",
            field,
            "Value must be between 0 and 100",
        ));
    }
    Ok(())
}

/// Builder for complex validations
///
/// # Example
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use pmcp::server::validation::Validator;
///
/// let mut v = Validator::new();
/// v.field("age", 25).range(&18, &120);
/// v.field("email", "user@example.com").email();
/// v.field("country", "US").one_of(&["US", "UK", "CA"]);
/// v.validate()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Validator {
    errors: Vec<Error>,
}

impl Validator {
    /// Create a new validator
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Start validating a field
    pub fn field<'a, T>(&'a mut self, name: &'a str, value: T) -> FieldValidator<'a, T> {
        FieldValidator {
            validator: self,
            name,
            value,
        }
    }

    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get validation result
    pub fn validate(self) -> Result<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            // Return first error for simplicity
            Err(self.errors.into_iter().next().unwrap())
        }
    }

    /// Get all validation errors
    pub fn errors(&self) -> &[Error] {
        &self.errors
    }

    fn add_error(&mut self, error: Error) {
        self.errors.push(error);
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

/// Field validator for chaining validations
#[derive(Debug)]
pub struct FieldValidator<'a, T> {
    validator: &'a mut Validator,
    name: &'a str,
    value: T,
}

impl<'a, T> FieldValidator<'a, T>
where
    T: PartialOrd + std::fmt::Display,
{
    /// Validate range
    pub fn range(self, min: &T, max: &T) -> &'a mut Validator {
        if let Err(e) = validate_range(self.name, &self.value, min, max) {
            self.validator.add_error(e);
        }
        self.validator
    }
}

impl<'a> FieldValidator<'a, &str> {
    /// Validate required
    pub fn required(self) -> &'a mut Validator {
        if let Err(e) = validate_required(self.name, self.value) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate email
    pub fn email(self) -> &'a mut Validator {
        if let Err(e) = validate_email(self.name, self.value) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate URL
    pub fn url(self) -> &'a mut Validator {
        if let Err(e) = validate_url(self.name, self.value) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate regex pattern
    pub fn regex(self, pattern: &str) -> &'a mut Validator {
        if let Err(e) = validate_regex(self.name, self.value, pattern) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate length
    pub fn length(self, min: Option<usize>, max: Option<usize>) -> &'a mut Validator {
        if let Err(e) = validate_length(self.name, self.value, min, max) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate one of
    pub fn one_of(self, allowed: &[&str]) -> &'a mut Validator {
        if let Err(e) = validate_one_of(self.name, &self.value, allowed) {
            self.validator.add_error(e);
        }
        self.validator
    }

    /// Validate safe path
    pub fn safe_path(self, allowed_prefix: Option<&str>) -> &'a mut Validator {
        if let Err(e) = validate_safe_path(self.name, self.value, allowed_prefix) {
            self.validator.add_error(e);
        }
        self.validator
    }
}

impl<'a, T> FieldValidator<'a, &[T]> {
    /// Validate array size
    pub fn size(self, min: Option<usize>, max: Option<usize>) -> &'a mut Validator {
        if let Err(e) = validate_array_size(self.name, self.value, min, max) {
            self.validator.add_error(e);
        }
        self.validator
    }
}

impl<'a> FieldValidator<'a, f64> {
    /// Validate percentage
    pub fn percentage(self) -> &'a mut Validator {
        if let Err(e) = validate_percentage(self.name, self.value) {
            self.validator.add_error(e);
        }
        self.validator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_range() {
        assert!(validate_range("age", &25, &18, &65).is_ok());
        assert!(validate_range("age", &10, &18, &65).is_err());
        assert!(validate_range("age", &70, &18, &65).is_err());
    }

    #[test]
    fn test_validate_one_of() {
        assert!(validate_one_of("currency", &"USD", &["USD", "EUR", "GBP"]).is_ok());
        assert!(validate_one_of("currency", &"JPY", &["USD", "EUR", "GBP"]).is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("email", "user@example.com").is_ok());
        assert!(validate_email("email", "invalid").is_err());
        assert!(validate_email("email", "@example.com").is_err());
        assert!(validate_email("email", "user@").is_err());
    }

    #[test]
    fn test_validate_safe_path() {
        assert!(validate_safe_path("path", "/tmp/file.txt", Some("/tmp/")).is_ok());
        assert!(validate_safe_path("path", "/tmp/../etc/passwd", None).is_err());
        assert!(validate_safe_path("path", "/etc/passwd", Some("/tmp/")).is_err());
    }

    #[test]
    fn test_validator_builder() {
        let mut v = Validator::new();
        v.field("age", 25).range(&18, &65);
        v.field("email", "user@example.com").email();
        let result = v.validate();

        assert!(result.is_ok());

        let mut v2 = Validator::new();
        v2.field("age", 10).range(&18, &65);
        let result = v2.validate();

        assert!(result.is_err());
    }
}
