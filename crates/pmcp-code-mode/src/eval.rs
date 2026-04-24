//! Shared expression evaluation logic for Code Mode.
//!
//! This module provides core evaluation functions that are used by both
//! synchronous (`PlanExecutor`) and asynchronous (`AsyncScope`) executors.
//! By centralizing this logic, we avoid duplication and ensure consistent
//! behavior across execution modes.
//!
//! ## Design
//!
//! The evaluation functions are generic over a `VariableProvider` trait,
//! allowing them to work with different variable storage mechanisms:
//!
//! ```ignore
//! // Sync executor uses HashMap directly
//! let value = evaluate_expr(&expr, &sync_vars, &local_scope)?;
//!
//! // Async executor uses the same functions
//! let value = evaluate_expr(&expr, &async_vars, &local_scope)?;
//! ```

use crate::types::ExecutionError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::executor::{
    ArrayMethodCall, BinaryOperator, BuiltinFunction, NumberMethodCall, ObjectField, UnaryOperator,
    ValueExpr,
};

/// Trait for providing variable values during evaluation.
///
/// This abstraction allows the evaluation functions to work with
/// different variable storage mechanisms (HashMap, async state, etc.).
pub trait VariableProvider {
    /// Get a variable value by name.
    fn get_variable(&self, name: &str) -> Option<&JsonValue>;
}

/// Simple HashMap-based variable provider.
impl VariableProvider for HashMap<String, JsonValue> {
    fn get_variable(&self, name: &str) -> Option<&JsonValue> {
        self.get(name)
    }
}

/// Evaluate a `ValueExpr` with access to global and local variables.
///
/// This is the core evaluation function used by both sync and async executors.
/// It recursively evaluates expressions, properly handling scope for nested
/// callbacks and block expressions.
///
/// # Arguments
/// * `expr` - The expression to evaluate
/// * `global_vars` - Global variable storage (executor's variables)
/// * `local_vars` - Local scope variables (callback parameters, block bindings)
///
/// # Returns
/// The evaluated JSON value or an execution error.
pub fn evaluate_with_scope<V: VariableProvider>(
    expr: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    match expr {
        ValueExpr::Variable(name) => evaluate_variable_lookup(name, global_vars, local_vars),
        ValueExpr::Literal(value) => Ok(value.clone()),
        ValueExpr::PropertyAccess { object, property } => {
            evaluate_property_access(object, property, global_vars, local_vars)
        },
        ValueExpr::ArrayIndex { array, index } => {
            evaluate_array_index(array, index, global_vars, local_vars)
        },
        ValueExpr::ObjectLiteral { fields } => {
            evaluate_object_literal(fields, global_vars, local_vars)
        },
        ValueExpr::ArrayLiteral { items } => evaluate_array_literal(items, global_vars, local_vars),
        ValueExpr::BinaryOp { left, op, right } => {
            let l = evaluate_with_scope(left, global_vars, local_vars)?;
            let r = evaluate_with_scope(right, global_vars, local_vars)?;
            evaluate_binary_op(&l, *op, &r)
        },
        ValueExpr::UnaryOp { op, operand } => {
            let v = evaluate_with_scope(operand, global_vars, local_vars)?;
            evaluate_unary_op(*op, &v)
        },
        ValueExpr::Ternary {
            condition,
            consequent,
            alternate,
        } => evaluate_ternary(condition, consequent, alternate, global_vars, local_vars),
        ValueExpr::OptionalChain { object, property } => {
            evaluate_optional_chain(object, property, global_vars, local_vars)
        },
        ValueExpr::NullishCoalesce { left, right } => {
            evaluate_nullish_coalesce(left, right, global_vars, local_vars)
        },
        ValueExpr::ArrayMethod { array, method } => {
            evaluate_array_method_dispatch(array, method, global_vars, local_vars)
        },
        ValueExpr::NumberMethod { number, method } => {
            let num_value = evaluate_with_scope(number, global_vars, local_vars)?;
            evaluate_number_method(&num_value, method)
        },
        ValueExpr::Block { bindings, result } => {
            evaluate_block(bindings, result, global_vars, local_vars)
        },
        ValueExpr::BuiltinCall { func, args } => {
            evaluate_builtin_call(func, args, global_vars, local_vars)
        },
        ValueExpr::ApiCall { .. } => Err(executor_only_error(
            "API calls should be handled by executor, not evaluator",
        )),
        ValueExpr::Await { .. } => Err(executor_only_error(
            "Await expressions should be handled by executor",
        )),
        ValueExpr::PromiseAll { .. } => Err(executor_only_error(
            "Promise.all should be handled by executor",
        )),
        #[cfg(feature = "mcp-code-mode")]
        ValueExpr::McpCall { .. } => Err(executor_only_error(
            "MCP calls should be handled by executor, not evaluator",
        )),
        ValueExpr::SdkCall { .. } => Err(executor_only_error(
            "SDK calls should be handled by executor, not evaluator",
        )),
    }
}

/// Build a "should be handled by executor" runtime error.
#[inline]
fn executor_only_error(message: &str) -> ExecutionError {
    ExecutionError::RuntimeError {
        message: message.into(),
    }
}

/// Evaluate a `Variable` reference: local scope, then global, then JS built-ins.
fn evaluate_variable_lookup<V: VariableProvider>(
    name: &str,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    if let Some(value) = local_vars.get(name) {
        return Ok(value.clone());
    }
    if let Some(value) = global_vars.get_variable(name) {
        return Ok(value.clone());
    }
    // JavaScript built-in globals — `undefined` maps to JSON null (no JSON distinction).
    if name == "undefined" {
        return Ok(JsonValue::Null);
    }
    Err(ExecutionError::RuntimeError {
        message: format!("Undefined variable: {}", name),
    })
}

/// Evaluate `obj.property`. Non-objects produce `null`.
fn evaluate_property_access<V: VariableProvider>(
    object: &ValueExpr,
    property: &str,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let obj = evaluate_with_scope(object, global_vars, local_vars)?;
    match obj {
        JsonValue::Object(map) => Ok(map.get(property).cloned().unwrap_or(JsonValue::Null)),
        _ => Ok(JsonValue::Null),
    }
}

/// Evaluate `arr[index]`. Non-array/non-number combinations produce `null`.
fn evaluate_array_index<V: VariableProvider>(
    array: &ValueExpr,
    index: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let arr = evaluate_with_scope(array, global_vars, local_vars)?;
    let idx = evaluate_with_scope(index, global_vars, local_vars)?;
    match (arr, idx) {
        (JsonValue::Array(arr), JsonValue::Number(n)) => {
            let i = n.as_i64().unwrap_or(0) as usize;
            Ok(arr.get(i).cloned().unwrap_or(JsonValue::Null))
        },
        _ => Ok(JsonValue::Null),
    }
}

/// Evaluate `{ key: value, ...spread }`. Non-object spreads are no-ops (JS semantics).
fn evaluate_object_literal<V: VariableProvider>(
    fields: &[ObjectField],
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut map = serde_json::Map::new();
    for field in fields {
        evaluate_object_field_into(field, &mut map, global_vars, local_vars)?;
    }
    Ok(JsonValue::Object(map))
}

/// Evaluate one `ObjectField` (key/value or spread) and merge into `map`.
fn evaluate_object_field_into<V: VariableProvider>(
    field: &ObjectField,
    map: &mut serde_json::Map<String, JsonValue>,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<(), ExecutionError> {
    match field {
        ObjectField::KeyValue {
            key,
            value: value_expr,
        } => {
            let value = evaluate_with_scope(value_expr, global_vars, local_vars)?;
            map.insert(key.clone(), value);
        },
        ObjectField::Spread { expr } => {
            let spread_val = evaluate_with_scope(expr, global_vars, local_vars)?;
            if let JsonValue::Object(spread_map) = spread_val {
                for (k, v) in spread_map {
                    map.insert(k, v);
                }
            }
            // Non-objects spread as no-op (matches JS behavior)
        },
    }
    Ok(())
}

/// Evaluate `[item1, item2, ...]` left-to-right.
fn evaluate_array_literal<V: VariableProvider>(
    items: &[ValueExpr],
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut arr = Vec::with_capacity(items.len());
    for item in items {
        arr.push(evaluate_with_scope(item, global_vars, local_vars)?);
    }
    Ok(JsonValue::Array(arr))
}

/// Evaluate `condition ? consequent : alternate` using JS truthiness on the condition.
fn evaluate_ternary<V: VariableProvider>(
    condition: &ValueExpr,
    consequent: &ValueExpr,
    alternate: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let cond = evaluate_with_scope(condition, global_vars, local_vars)?;
    if is_truthy(&cond) {
        evaluate_with_scope(consequent, global_vars, local_vars)
    } else {
        evaluate_with_scope(alternate, global_vars, local_vars)
    }
}

/// Evaluate `obj?.property`. Null/undefined short-circuits to `null`.
fn evaluate_optional_chain<V: VariableProvider>(
    object: &ValueExpr,
    property: &str,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let obj = evaluate_with_scope(object, global_vars, local_vars)?;
    match obj {
        JsonValue::Null => Ok(JsonValue::Null),
        JsonValue::Object(map) => Ok(map.get(property).cloned().unwrap_or(JsonValue::Null)),
        _ => Ok(JsonValue::Null),
    }
}

/// Evaluate `a ?? b`: take `a` unless it is null/undefined, in which case evaluate `b`.
fn evaluate_nullish_coalesce<V: VariableProvider>(
    left: &ValueExpr,
    right: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let l = evaluate_with_scope(left, global_vars, local_vars)?;
    if is_nullish(&l) {
        evaluate_with_scope(right, global_vars, local_vars)
    } else {
        Ok(l)
    }
}

/// Dispatch to `evaluate_array_method_with_scope` after evaluating the receiver and
/// cloning local scope once (vs N clones per element inside the method body).
fn evaluate_array_method_dispatch<V: VariableProvider>(
    array: &ValueExpr,
    method: &ArrayMethodCall,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let arr_value = evaluate_with_scope(array, global_vars, local_vars)?;
    let mut scope = local_vars.clone();
    evaluate_array_method_with_scope(&arr_value, method, global_vars, &mut scope)
}

/// Evaluate a block expression `{ const x = ...; const y = ...; result }`.
/// Bindings extend a freshly-merged scope in declaration order.
fn evaluate_block<V: VariableProvider>(
    bindings: &[(String, ValueExpr)],
    result: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut merged_vars = local_vars.clone();
    for (name, binding_expr) in bindings {
        let value = evaluate_with_scope(binding_expr, global_vars, &merged_vars)?;
        merged_vars.insert(name.clone(), value);
    }
    evaluate_with_scope(result, global_vars, &merged_vars)
}

/// Evaluate a `BuiltinCall`: evaluate args left-to-right then dispatch to `evaluate_builtin`.
fn evaluate_builtin_call<V: VariableProvider>(
    func: &BuiltinFunction,
    args: &[ValueExpr],
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let evaluated_args: Vec<JsonValue> = args
        .iter()
        .map(|a| evaluate_with_scope(a, global_vars, local_vars))
        .collect::<Result<Vec<_>, _>>()?;
    evaluate_builtin(func, &evaluated_args)
}

/// Evaluate a binary operation.
pub fn evaluate_binary_op(
    left: &JsonValue,
    op: BinaryOperator,
    right: &JsonValue,
) -> Result<JsonValue, ExecutionError> {
    match op {
        BinaryOperator::Add => add_values(left, right),
        BinaryOperator::Sub => numeric_op(left, right, |a, b| a - b),
        BinaryOperator::Mul => numeric_op(left, right, |a, b| a * b),
        BinaryOperator::Div => {
            numeric_op(left, right, |a, b| if b != 0.0 { a / b } else { f64::NAN })
        },
        BinaryOperator::Mod => {
            numeric_op(left, right, |a, b| if b != 0.0 { a % b } else { f64::NAN })
        },
        BinaryOperator::BitwiseOr => {
            numeric_op(left, right, |a, b| ((a as i32) | (b as i32)) as f64)
        },
        BinaryOperator::Eq => Ok(JsonValue::Bool(json_equals(left, right))),
        BinaryOperator::NotEq => Ok(JsonValue::Bool(!json_equals(left, right))),
        BinaryOperator::StrictEq => Ok(JsonValue::Bool(json_strict_equals(left, right))),
        BinaryOperator::StrictNotEq => Ok(JsonValue::Bool(!json_strict_equals(left, right))),
        BinaryOperator::Lt => compare_values(left, right, |a, b| a < b),
        BinaryOperator::Lte => compare_values(left, right, |a, b| a <= b),
        BinaryOperator::Gt => compare_values(left, right, |a, b| a > b),
        BinaryOperator::Gte => compare_values(left, right, |a, b| a >= b),
        BinaryOperator::And => Ok(if is_truthy(left) {
            right.clone()
        } else {
            left.clone()
        }),
        BinaryOperator::Or => Ok(if is_truthy(left) {
            left.clone()
        } else {
            right.clone()
        }),
        BinaryOperator::Concat => {
            let l_str = json_to_string(left);
            let r_str = json_to_string(right);
            Ok(JsonValue::String(format!("{}{}", l_str, r_str)))
        },
    }
}

/// Evaluate a unary operation.
pub fn evaluate_unary_op(
    op: UnaryOperator,
    value: &JsonValue,
) -> Result<JsonValue, ExecutionError> {
    match op {
        UnaryOperator::Not => Ok(JsonValue::Bool(!is_truthy(value))),
        UnaryOperator::Plus => {
            let n = to_number(value);
            Ok(serde_json::Number::from_f64(n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null))
        },
        UnaryOperator::Neg => {
            let n = to_number(value);
            Ok(JsonValue::Number(
                serde_json::Number::from_f64(-n).unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        },
        UnaryOperator::TypeOf => {
            let type_str = match value {
                JsonValue::Null => "object", // JavaScript quirk
                JsonValue::Bool(_) => "boolean",
                JsonValue::Number(_) => "number",
                JsonValue::String(_) => "string",
                JsonValue::Array(_) => "object",
                JsonValue::Object(_) => "object",
            };
            Ok(JsonValue::String(type_str.to_string()))
        },
    }
}

/// Check if a JSON value is truthy (JavaScript semantics).
pub fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        JsonValue::String(s) => !s.is_empty(),
        JsonValue::Array(_) => true,
        JsonValue::Object(_) => true,
    }
}

/// Check if a JSON value is nullish (null or undefined).
pub fn is_nullish(value: &JsonValue) -> bool {
    matches!(value, JsonValue::Null)
}

/// Convert a JSON value to a number (JavaScript semantics).
pub fn to_number(value: &JsonValue) -> f64 {
    match value {
        JsonValue::Null => 0.0,
        JsonValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        },
        JsonValue::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        JsonValue::String(s) => s.parse().unwrap_or(f64::NAN),
        JsonValue::Array(_) | JsonValue::Object(_) => f64::NAN,
    }
}

/// Add two JSON values (handles both numeric and string concatenation).
fn add_values(left: &JsonValue, right: &JsonValue) -> Result<JsonValue, ExecutionError> {
    // String concatenation takes precedence
    if matches!(left, JsonValue::String(_)) || matches!(right, JsonValue::String(_)) {
        let l_str = json_to_string(left);
        let r_str = json_to_string(right);
        return Ok(JsonValue::String(format!("{}{}", l_str, r_str)));
    }

    // Numeric addition
    let l = to_number(left);
    let r = to_number(right);
    Ok(JsonValue::Number(
        serde_json::Number::from_f64(l + r).unwrap_or_else(|| serde_json::Number::from(0)),
    ))
}

/// Perform a numeric operation.
fn numeric_op<F>(left: &JsonValue, right: &JsonValue, op: F) -> Result<JsonValue, ExecutionError>
where
    F: Fn(f64, f64) -> f64,
{
    let l = to_number(left);
    let r = to_number(right);
    let result = op(l, r);
    Ok(JsonValue::Number(
        serde_json::Number::from_f64(result).unwrap_or_else(|| serde_json::Number::from(0)),
    ))
}

/// Compare two JSON values.
fn compare_values<F>(
    left: &JsonValue,
    right: &JsonValue,
    op: F,
) -> Result<JsonValue, ExecutionError>
where
    F: Fn(f64, f64) -> bool,
{
    let l = to_number(left);
    let r = to_number(right);
    Ok(JsonValue::Bool(op(l, r)))
}

/// Check equality (loose, ==).
fn json_equals(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            a.as_f64().unwrap_or(f64::NAN) == b.as_f64().unwrap_or(f64::NAN)
        },
        (JsonValue::String(a), JsonValue::String(b)) => a == b,
        // Loose equality type coercion
        (JsonValue::Number(n), JsonValue::String(s))
        | (JsonValue::String(s), JsonValue::Number(n)) => {
            if let Ok(parsed) = s.parse::<f64>() {
                n.as_f64().unwrap_or(f64::NAN) == parsed
            } else {
                false
            }
        },
        _ => false,
    }
}

/// Check strict equality (===).
fn json_strict_equals(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            a.as_f64().unwrap_or(f64::NAN) == b.as_f64().unwrap_or(f64::NAN)
        },
        (JsonValue::String(a), JsonValue::String(b)) => a == b,
        (JsonValue::Array(a), JsonValue::Array(b)) => std::ptr::eq(a, b), // Reference equality
        (JsonValue::Object(a), JsonValue::Object(b)) => std::ptr::eq(a, b), // Reference equality
        _ => false,
    }
}

/// Controls how non-primitive JSON values are rendered to string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonStringMode {
    /// JavaScript-compatible: objects render as "[object Object]", arrays as JSON.
    JavaScript,
    /// JSON-compatible: all values render via `serde_json::to_string`.
    Json,
}

/// Convert JSON value to string with configurable object rendering.
pub fn json_to_string_with_mode(value: &JsonValue, mode: JsonStringMode) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(_) => match mode {
            JsonStringMode::JavaScript => value.to_string(),
            JsonStringMode::Json => serde_json::to_string(value).unwrap_or_default(),
        },
        JsonValue::Object(_) => match mode {
            JsonStringMode::JavaScript => "[object Object]".to_string(),
            JsonStringMode::Json => serde_json::to_string(value).unwrap_or_default(),
        },
    }
}

/// Convert JSON value to string (JavaScript-compatible mode).
///
/// Objects render as `"[object Object]"` matching JavaScript's `String()` behavior.
pub fn json_to_string(value: &JsonValue) -> String {
    json_to_string_with_mode(value, JsonStringMode::JavaScript)
}

/// Restore a scope variable to its previous value, or remove it if it didn't exist before.
#[inline]
fn restore_scope_var(
    scope: &mut HashMap<String, JsonValue>,
    key: &str,
    previous: Option<JsonValue>,
) {
    match previous {
        Some(prev) => {
            scope.insert(key.to_string(), prev);
        },
        None => {
            scope.remove(key);
        },
    }
}

/// Evaluate an array method with scope.
///
/// Supports dual-dispatch: methods like `.length`, `.includes()`, `.indexOf()`,
/// `.slice()`, and `.concat()` work on both arrays and strings (matching JS behavior).
/// Array-only methods (`.map()`, `.filter()`, etc.) produce a clear error on strings.
pub fn evaluate_array_method_with_scope<V: VariableProvider>(
    arr_value: &JsonValue,
    method: &ArrayMethodCall,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    // String-compatible methods — dispatch before array handling.
    if let JsonValue::String(s) = arr_value {
        return evaluate_string_method(s, method, global_vars, local_vars);
    }

    let arr = match arr_value {
        JsonValue::Array(a) => a.clone(),
        _ => {
            return Err(ExecutionError::RuntimeError {
                message: "Method called on non-array and non-string".into(),
            })
        },
    };

    match method {
        ArrayMethodCall::Length => Ok(JsonValue::Number((arr.len() as i64).into())),
        ArrayMethodCall::Map { item_var, body } => {
            eval_array_map(arr, item_var, body, global_vars, local_vars)
        },
        ArrayMethodCall::Filter {
            item_var,
            predicate,
        } => eval_array_filter(arr, item_var, predicate, global_vars, local_vars),
        ArrayMethodCall::Find {
            item_var,
            predicate,
        } => eval_array_find(arr, item_var, predicate, global_vars, local_vars),
        ArrayMethodCall::Some {
            item_var,
            predicate,
        } => eval_array_some(arr, item_var, predicate, global_vars, local_vars),
        ArrayMethodCall::Every {
            item_var,
            predicate,
        } => eval_array_every(arr, item_var, predicate, global_vars, local_vars),
        ArrayMethodCall::FlatMap { item_var, body } => {
            eval_array_flat_map(arr, item_var, body, global_vars, local_vars)
        },
        ArrayMethodCall::Reduce {
            acc_var,
            item_var,
            body,
            initial,
        } => eval_array_reduce(
            arr,
            acc_var,
            item_var,
            body,
            initial,
            global_vars,
            local_vars,
        ),
        ArrayMethodCall::Slice { start, end } => Ok(eval_array_slice(arr, *start, *end)),
        ArrayMethodCall::Concat { other } => {
            eval_array_concat(arr, other, global_vars, local_vars)
        },
        ArrayMethodCall::Push { item } => eval_array_push(arr, item, global_vars, local_vars),
        ArrayMethodCall::Join { separator } => Ok(eval_array_join(&arr, separator.as_deref())),
        ArrayMethodCall::Reverse => {
            let mut reversed = arr;
            reversed.reverse();
            Ok(JsonValue::Array(reversed))
        },
        ArrayMethodCall::Sort { comparator } => {
            eval_array_sort(arr, comparator.as_ref(), global_vars, local_vars)
        },
        ArrayMethodCall::Flat => Ok(JsonValue::Array(flatten_array(arr, 1))),
        ArrayMethodCall::Includes { item } => {
            eval_array_includes(&arr, item, global_vars, local_vars)
        },
        ArrayMethodCall::IndexOf { item } => {
            eval_array_index_of(&arr, item, global_vars, local_vars)
        },
        ArrayMethodCall::First => Ok(arr.into_iter().next().unwrap_or(JsonValue::Null)),
        ArrayMethodCall::Last => Ok(arr.into_iter().last().unwrap_or(JsonValue::Null)),
        ArrayMethodCall::ToString => Ok(JsonValue::String(
            serde_json::to_string(&JsonValue::Array(arr)).unwrap_or_default(),
        )),

        // String-only methods — error when called on arrays.
        ArrayMethodCall::ToLowerCase
        | ArrayMethodCall::ToUpperCase
        | ArrayMethodCall::StartsWith { .. }
        | ArrayMethodCall::EndsWith { .. }
        | ArrayMethodCall::Trim
        | ArrayMethodCall::Replace { .. }
        | ArrayMethodCall::Split { .. }
        | ArrayMethodCall::Substring { .. } => Err(ExecutionError::RuntimeError {
            message: "This method is only available on strings, not arrays".into(),
        }),
    }
}

/// `arr.map(item => body)` — collect each evaluated body into a result vec.
fn eval_array_map<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    body: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut results = Vec::with_capacity(arr.len());
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item);
        let value = evaluate_with_scope(body, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        results.push(value?);
    }
    Ok(JsonValue::Array(results))
}

/// `arr.filter(item => predicate)` — keep items whose predicate is truthy.
fn eval_array_filter<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    predicate: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut results = Vec::new();
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item.clone());
        let keep = evaluate_with_scope(predicate, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        if is_truthy(&keep?) {
            results.push(item);
        }
    }
    Ok(JsonValue::Array(results))
}

/// `arr.find(item => predicate)` — first item whose predicate is truthy, or null.
fn eval_array_find<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    predicate: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item.clone());
        let found = evaluate_with_scope(predicate, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        if is_truthy(&found?) {
            return Ok(item);
        }
    }
    Ok(JsonValue::Null)
}

/// `arr.some(item => predicate)` — true iff any item's predicate is truthy.
fn eval_array_some<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    predicate: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item);
        let found = evaluate_with_scope(predicate, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        if is_truthy(&found?) {
            return Ok(JsonValue::Bool(true));
        }
    }
    Ok(JsonValue::Bool(false))
}

/// `arr.every(item => predicate)` — true iff every item's predicate is truthy.
fn eval_array_every<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    predicate: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item);
        let found = evaluate_with_scope(predicate, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        if !is_truthy(&found?) {
            return Ok(JsonValue::Bool(false));
        }
    }
    Ok(JsonValue::Bool(true))
}

/// `arr.flatMap(item => body)` — map then flatten one level (non-array bodies kept as-is).
fn eval_array_flat_map<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item_var: &str,
    body: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut results = Vec::new();
    for item in arr {
        let old = local_vars.insert(item_var.to_string(), item);
        let value = evaluate_with_scope(body, global_vars, local_vars);
        restore_scope_var(local_vars, item_var, old);
        match value? {
            JsonValue::Array(items) => results.extend(items),
            other => results.push(other),
        }
    }
    Ok(JsonValue::Array(results))
}

/// `arr.reduce((acc, item) => body, initial)` — left-fold with both vars in scope.
fn eval_array_reduce<V: VariableProvider>(
    arr: Vec<JsonValue>,
    acc_var: &str,
    item_var: &str,
    body: &ValueExpr,
    initial: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut acc = evaluate_with_scope(initial, global_vars, local_vars)?;
    for item in arr {
        let old_acc = local_vars.insert(acc_var.to_string(), acc.clone());
        let old_item = local_vars.insert(item_var.to_string(), item);
        let result = evaluate_with_scope(body, global_vars, local_vars);
        restore_scope_var(local_vars, acc_var, old_acc);
        restore_scope_var(local_vars, item_var, old_item);
        acc = result?;
    }
    Ok(acc)
}

/// `arr.slice(start, end)` — half-open interval, `end` defaults to `len`.
fn eval_array_slice(arr: Vec<JsonValue>, start: usize, end: Option<usize>) -> JsonValue {
    let len = arr.len();
    let end_idx = end.unwrap_or(len).min(len);
    let sliced: Vec<JsonValue> = arr
        .into_iter()
        .skip(start)
        .take(end_idx.saturating_sub(start))
        .collect();
    JsonValue::Array(sliced)
}

/// `arr.concat(other)` — array → extend in place; non-array → push as single element.
fn eval_array_concat<V: VariableProvider>(
    arr: Vec<JsonValue>,
    other: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut result = arr;
    let other_val = evaluate_with_scope(other, global_vars, local_vars)?;
    if let JsonValue::Array(other_arr) = other_val {
        result.extend(other_arr);
    } else {
        result.push(other_val);
    }
    Ok(JsonValue::Array(result))
}

/// `arr.push(item)` — append, returning a new array (no in-place mutation of source).
fn eval_array_push<V: VariableProvider>(
    arr: Vec<JsonValue>,
    item: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut result = arr;
    let item_val = evaluate_with_scope(item, global_vars, local_vars)?;
    result.push(item_val);
    Ok(JsonValue::Array(result))
}

/// `arr.join(separator)` — JS-compatible string join, `","` if separator is `None`.
fn eval_array_join(arr: &[JsonValue], separator: Option<&str>) -> JsonValue {
    let sep = separator.unwrap_or(",");
    let joined: String = arr
        .iter()
        .map(json_to_string)
        .collect::<Vec<_>>()
        .join(sep);
    JsonValue::String(joined)
}

/// `arr.sort()` (default lexicographic) or `arr.sort((a, b) => expr)` (custom comparator).
/// Custom-comparator errors are captured on the first failing pair and bubbled out.
fn eval_array_sort<V: VariableProvider>(
    arr: Vec<JsonValue>,
    comparator: Option<&(String, String, Box<ValueExpr>)>,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let mut sorted = arr;
    match comparator {
        None => sorted.sort_by_key(json_to_string),
        Some((a_var, b_var, body)) => {
            sort_with_comparator(&mut sorted, a_var, b_var, body, global_vars, local_vars)?;
        },
    }
    Ok(JsonValue::Array(sorted))
}

/// Apply a JavaScript-style comparator callback to a `Vec<JsonValue>` in place.
/// Captures the first comparator error and propagates it after the sort completes.
fn sort_with_comparator<V: VariableProvider>(
    sorted: &mut [JsonValue],
    a_var: &str,
    b_var: &str,
    body: &ValueExpr,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<(), ExecutionError> {
    let mut sort_error: Option<ExecutionError> = None;
    sorted.sort_by(|a, b| {
        if sort_error.is_some() {
            return std::cmp::Ordering::Equal;
        }
        let mut merged = local_vars.clone();
        merged.insert(a_var.to_string(), a.clone());
        merged.insert(b_var.to_string(), b.clone());
        match evaluate_with_scope(body, global_vars, &merged) {
            Ok(result) => comparator_result_to_ordering(&result),
            Err(e) => {
                sort_error = Some(e);
                std::cmp::Ordering::Equal
            },
        }
    });
    match sort_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// JS comparator convention: negative → Less, positive → Greater, zero/NaN → Equal.
#[inline]
fn comparator_result_to_ordering(result: &JsonValue) -> std::cmp::Ordering {
    let n = to_number(result);
    if n < 0.0 {
        std::cmp::Ordering::Less
    } else if n > 0.0 {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

/// `arr.includes(item)` — true iff any element loose-equals the search value.
fn eval_array_includes<V: VariableProvider>(
    arr: &[JsonValue],
    item: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let search_val = evaluate_with_scope(item, global_vars, local_vars)?;
    let found = arr.iter().any(|elem| json_equals(elem, &search_val));
    Ok(JsonValue::Bool(found))
}

/// `arr.indexOf(item)` — first matching index, or -1.
fn eval_array_index_of<V: VariableProvider>(
    arr: &[JsonValue],
    item: &ValueExpr,
    global_vars: &V,
    local_vars: &mut HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    let search_val = evaluate_with_scope(item, global_vars, local_vars)?;
    for (i, arr_item) in arr.iter().enumerate() {
        if json_equals(arr_item, &search_val) {
            return Ok(JsonValue::Number((i as i64).into()));
        }
    }
    Ok(JsonValue::Number((-1_i64).into()))
}

/// Evaluate a method call on a string value.
///
/// These mirror JavaScript's string methods for the subset that overlaps
/// with array methods (`.length`, `.includes()`, `.indexOf()`, `.slice()`, `.concat()`).
fn evaluate_string_method<V: VariableProvider>(
    s: &str,
    method: &ArrayMethodCall,
    global_vars: &V,
    local_vars: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    match method {
        ArrayMethodCall::Length => Ok(JsonValue::Number((s.chars().count() as i64).into())),

        ArrayMethodCall::Includes { item } => {
            let search_val = evaluate_with_scope(item, global_vars, local_vars)?;
            match search_val {
                JsonValue::String(sub) => Ok(JsonValue::Bool(s.contains(sub.as_str()))),
                _ => Ok(JsonValue::Bool(false)),
            }
        },

        ArrayMethodCall::IndexOf { item } => {
            let search_val = evaluate_with_scope(item, global_vars, local_vars)?;
            match search_val {
                JsonValue::String(sub) => {
                    // Use char-based index for safety with multi-byte characters
                    let idx = s
                        .char_indices()
                        .zip(0i64..)
                        .find_map(|((byte_pos, _), char_idx)| {
                            if s[byte_pos..].starts_with(sub.as_str()) {
                                Some(char_idx)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(-1);
                    Ok(JsonValue::Number(idx.into()))
                },
                _ => Ok(JsonValue::Number((-1_i64).into())),
            }
        },

        ArrayMethodCall::Slice { start, end } => {
            let char_count = s.chars().count();
            let end_idx = end.unwrap_or(char_count).min(char_count);
            let start_idx = (*start).min(char_count);
            let sliced: String = s
                .chars()
                .skip(start_idx)
                .take(end_idx.saturating_sub(start_idx))
                .collect();
            Ok(JsonValue::String(sliced))
        },

        ArrayMethodCall::Concat { other } => {
            let other_val = evaluate_with_scope(other, global_vars, local_vars)?;
            Ok(JsonValue::String(format!(
                "{}{}",
                s,
                json_to_string(&other_val)
            )))
        },

        ArrayMethodCall::ToLowerCase => Ok(JsonValue::String(s.to_lowercase())),

        ArrayMethodCall::ToUpperCase => Ok(JsonValue::String(s.to_uppercase())),

        ArrayMethodCall::StartsWith { search } => {
            let search_val = evaluate_with_scope(search, global_vars, local_vars)?;
            match search_val {
                JsonValue::String(sub) => Ok(JsonValue::Bool(s.starts_with(sub.as_str()))),
                _ => Ok(JsonValue::Bool(false)),
            }
        },

        ArrayMethodCall::EndsWith { search } => {
            let search_val = evaluate_with_scope(search, global_vars, local_vars)?;
            match search_val {
                JsonValue::String(sub) => Ok(JsonValue::Bool(s.ends_with(sub.as_str()))),
                _ => Ok(JsonValue::Bool(false)),
            }
        },

        ArrayMethodCall::Trim => Ok(JsonValue::String(s.trim().to_string())),

        ArrayMethodCall::Replace {
            search,
            replacement,
        } => {
            let search_val = evaluate_with_scope(search, global_vars, local_vars)?;
            let repl_val = evaluate_with_scope(replacement, global_vars, local_vars)?;
            match (search_val, repl_val) {
                (JsonValue::String(needle), JsonValue::String(repl)) => {
                    // JS .replace() only replaces the first occurrence
                    Ok(JsonValue::String(s.replacen(
                        needle.as_str(),
                        repl.as_str(),
                        1,
                    )))
                },
                _ => Ok(JsonValue::String(s.to_string())),
            }
        },

        ArrayMethodCall::Split { separator } => {
            let sep_val = evaluate_with_scope(separator, global_vars, local_vars)?;
            match sep_val {
                JsonValue::String(sep) if sep.is_empty() => {
                    // JS "ab".split("") => ["a", "b"] (split into chars)
                    // Safety cap to prevent unbounded memory from large strings
                    let parts: Vec<JsonValue> = s
                        .chars()
                        .take(10_000)
                        .map(|c| JsonValue::String(c.to_string()))
                        .collect();
                    Ok(JsonValue::Array(parts))
                },
                JsonValue::String(sep) => {
                    let parts: Vec<JsonValue> = s
                        .split(sep.as_str())
                        .map(|p| JsonValue::String(p.to_string()))
                        .collect();
                    Ok(JsonValue::Array(parts))
                },
                _ => Ok(JsonValue::Array(vec![JsonValue::String(s.to_string())])),
            }
        },

        ArrayMethodCall::Substring { start, end } => {
            let start_val = evaluate_with_scope(start, global_vars, local_vars)?;
            let start_idx = match start_val {
                JsonValue::Number(n) => n.as_u64().unwrap_or(0) as usize,
                _ => 0,
            };
            let end_idx = if let Some(end_expr) = end {
                let end_val = evaluate_with_scope(end_expr, global_vars, local_vars)?;
                match end_val {
                    JsonValue::Number(n) => n.as_u64().unwrap_or(0) as usize,
                    _ => usize::MAX,
                }
            } else {
                usize::MAX
            };
            // JS substring() swaps if start > end
            let (lo, hi) = if start_idx > end_idx {
                (end_idx, start_idx)
            } else {
                (start_idx, end_idx)
            };
            // Single-pass: skip to lo, take (hi - lo) chars
            let result: String = s.chars().skip(lo).take(hi.saturating_sub(lo)).collect();
            Ok(JsonValue::String(result))
        },

        ArrayMethodCall::ToString => Ok(JsonValue::String(s.to_string())),

        // Array-only methods — produce a helpful error
        _ => {
            let method_name = match method {
                ArrayMethodCall::Map { .. } => ".map()",
                ArrayMethodCall::Filter { .. } => ".filter()",
                ArrayMethodCall::Find { .. } => ".find()",
                ArrayMethodCall::Some { .. } => ".some()",
                ArrayMethodCall::Every { .. } => ".every()",
                ArrayMethodCall::Reduce { .. } => ".reduce()",
                ArrayMethodCall::FlatMap { .. } => ".flatMap()",
                ArrayMethodCall::Push { .. } => ".push()",
                ArrayMethodCall::Join { .. } => ".join()",
                ArrayMethodCall::Reverse => ".reverse()",
                ArrayMethodCall::Sort { .. } => ".sort()",
                ArrayMethodCall::Flat => ".flat()",
                ArrayMethodCall::First => ".first()",
                ArrayMethodCall::Last => ".last()",
                _ => "this method",
            };
            Err(ExecutionError::RuntimeError {
                message: format!(
                    "String does not support {} — use it only on arrays",
                    method_name
                ),
            })
        },
    }
}

/// Evaluate a built-in function call (parseFloat, Math.abs, Object.keys, etc.).
fn evaluate_builtin(
    func: &BuiltinFunction,
    args: &[JsonValue],
) -> Result<JsonValue, ExecutionError> {
    /// Helper to convert an f64 to JsonValue, returning Null for NaN.
    fn number_or_null(n: f64) -> JsonValue {
        serde_json::Number::from_f64(n)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null)
    }

    match func {
        BuiltinFunction::ParseFloat | BuiltinFunction::NumberCast => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            Ok(number_or_null(to_number(val)))
        },
        BuiltinFunction::ParseInt => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            let n = to_number(val);
            if n.is_finite() {
                Ok(JsonValue::Number((n as i64).into()))
            } else {
                Ok(JsonValue::Null) // NaN
            }
        },
        BuiltinFunction::MathAbs => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            Ok(number_or_null(to_number(val).abs()))
        },
        BuiltinFunction::MathRound => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            Ok(number_or_null(to_number(val).round()))
        },
        BuiltinFunction::MathFloor => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            Ok(number_or_null(to_number(val).floor()))
        },
        BuiltinFunction::MathCeil => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            Ok(number_or_null(to_number(val).ceil()))
        },
        BuiltinFunction::MathMax => {
            if args.is_empty() {
                return Ok(number_or_null(f64::NEG_INFINITY));
            }
            let mut result = f64::NEG_INFINITY;
            for arg in args {
                result = result.max(to_number(arg));
            }
            Ok(number_or_null(result))
        },
        BuiltinFunction::MathMin => {
            if args.is_empty() {
                return Ok(number_or_null(f64::INFINITY));
            }
            let mut result = f64::INFINITY;
            for arg in args {
                result = result.min(to_number(arg));
            }
            Ok(number_or_null(result))
        },
        BuiltinFunction::ObjectKeys => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            match val {
                JsonValue::Object(map) => {
                    let keys: Vec<JsonValue> =
                        map.keys().map(|k| JsonValue::String(k.clone())).collect();
                    Ok(JsonValue::Array(keys))
                },
                _ => Ok(JsonValue::Array(vec![])),
            }
        },
        BuiltinFunction::ObjectValues => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            match val {
                JsonValue::Object(map) => {
                    let values: Vec<JsonValue> = map.values().cloned().collect();
                    Ok(JsonValue::Array(values))
                },
                _ => Ok(JsonValue::Array(vec![])),
            }
        },
        BuiltinFunction::ObjectEntries => {
            let val = args.first().unwrap_or(&JsonValue::Null);
            match val {
                JsonValue::Object(map) => {
                    let entries: Vec<JsonValue> = map
                        .iter()
                        .map(|(k, v)| {
                            JsonValue::Array(vec![JsonValue::String(k.clone()), v.clone()])
                        })
                        .collect();
                    Ok(JsonValue::Array(entries))
                },
                _ => Ok(JsonValue::Array(vec![])),
            }
        },
    }
}

/// Flatten an array to one level.
fn flatten_array(arr: Vec<JsonValue>, depth: usize) -> Vec<JsonValue> {
    if depth == 0 {
        return arr;
    }

    let mut result = Vec::new();
    for item in arr {
        if let JsonValue::Array(inner) = item {
            result.extend(flatten_array(inner, depth - 1));
        } else {
            result.push(item);
        }
    }
    result
}

/// Evaluate a number method.
pub fn evaluate_number_method(
    num_value: &JsonValue,
    method: &NumberMethodCall,
) -> Result<JsonValue, ExecutionError> {
    let num = match num_value {
        JsonValue::Number(n) => n.as_f64().unwrap_or(0.0),
        JsonValue::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => {
            return Err(ExecutionError::RuntimeError {
                message: format!("Number method called on non-number: {:?}", num_value),
            })
        },
    };

    match method {
        NumberMethodCall::ToFixed { digits } => {
            let formatted = format!("{:.prec$}", num, prec = *digits);
            Ok(JsonValue::String(formatted))
        },
        NumberMethodCall::ToString => Ok(JsonValue::String(num.to_string())),
    }
}

/// Evaluate an expression with just a variable map (no local scope).
/// This is a convenience wrapper for the common case.
pub fn evaluate(
    expr: &ValueExpr,
    variables: &HashMap<String, JsonValue>,
) -> Result<JsonValue, ExecutionError> {
    evaluate_with_scope(expr, variables, &HashMap::new())
}

/// Evaluate an expression with an extra variable binding.
/// Creates a merged scope with the new binding and evaluates.
pub fn evaluate_with_binding(
    expr: &ValueExpr,
    variables: &HashMap<String, JsonValue>,
    var: &str,
    value: &JsonValue,
) -> Result<JsonValue, ExecutionError> {
    let mut local_vars = HashMap::new();
    local_vars.insert(var.to_string(), value.clone());
    evaluate_with_scope(expr, variables, &local_vars)
}

/// Evaluate an expression with two extra variable bindings.
/// Used for reduce operations with accumulator and item variables.
pub fn evaluate_with_two_bindings(
    expr: &ValueExpr,
    variables: &HashMap<String, JsonValue>,
    var1: &str,
    value1: &JsonValue,
    var2: &str,
    value2: &JsonValue,
) -> Result<JsonValue, ExecutionError> {
    let mut local_vars = HashMap::new();
    local_vars.insert(var1.to_string(), value1.clone());
    local_vars.insert(var2.to_string(), value2.clone());
    evaluate_with_scope(expr, variables, &local_vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undefined_global() {
        let vars: HashMap<String, JsonValue> = HashMap::new();
        let expr = ValueExpr::Variable("undefined".to_string());
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_undefined_variable_error() {
        let vars: HashMap<String, JsonValue> = HashMap::new();
        let expr = ValueExpr::Variable("nonexistent".to_string());
        let result = evaluate(&expr, &vars);
        assert!(result.is_err());
        match result {
            Err(ExecutionError::RuntimeError { message }) => {
                assert!(message.contains("Undefined variable"));
            },
            _ => panic!("Expected RuntimeError"),
        }
    }

    #[test]
    fn test_comparison_with_undefined() {
        let mut vars: HashMap<String, JsonValue> = HashMap::new();
        vars.insert("x".to_string(), JsonValue::Null);

        // x !== undefined should be false when x is null
        let expr = ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("x".to_string())),
            op: BinaryOperator::StrictNotEq,
            right: Box::new(ValueExpr::Variable("undefined".to_string())),
        };
        let result = evaluate(&expr, &vars).unwrap();
        // null !== null is false (they're strictly equal in our model)
        assert_eq!(result, JsonValue::Bool(false));
    }

    // =========================================================================
    // String method tests
    // =========================================================================

    /// Helper to evaluate an ArrayMethod on a string variable.
    fn eval_string_method(s: &str, method: ArrayMethodCall) -> Result<JsonValue, ExecutionError> {
        let mut vars = HashMap::new();
        vars.insert("s".to_string(), JsonValue::String(s.to_string()));
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("s".to_string())),
            method,
        };
        evaluate(&expr, &vars)
    }

    #[test]
    fn test_string_length() {
        let result = eval_string_method("hello", ArrayMethodCall::Length).unwrap();
        assert_eq!(result, JsonValue::Number(5.into()));
    }

    #[test]
    fn test_string_length_empty() {
        let result = eval_string_method("", ArrayMethodCall::Length).unwrap();
        assert_eq!(result, JsonValue::Number(0.into()));
    }

    #[test]
    fn test_string_length_multibyte() {
        // Emoji is 1 char, not multiple bytes
        let result = eval_string_method("hi\u{1F600}", ArrayMethodCall::Length).unwrap();
        assert_eq!(result, JsonValue::Number(3.into()));
    }

    #[test]
    fn test_string_includes_hit() {
        let result = eval_string_method(
            "hello world",
            ArrayMethodCall::Includes {
                item: Box::new(ValueExpr::Literal(JsonValue::String("world".into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Bool(true));
    }

    #[test]
    fn test_string_includes_miss() {
        let result = eval_string_method(
            "hello",
            ArrayMethodCall::Includes {
                item: Box::new(ValueExpr::Literal(JsonValue::String("xyz".into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Bool(false));
    }

    #[test]
    fn test_string_includes_non_string_arg() {
        // Searching for a number in a string returns false (no coercion)
        let result = eval_string_method(
            "abc 42 def",
            ArrayMethodCall::Includes {
                item: Box::new(ValueExpr::Literal(JsonValue::Number(42.into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Bool(false));
    }

    #[test]
    fn test_string_index_of_found() {
        let result = eval_string_method(
            "abcdef",
            ArrayMethodCall::IndexOf {
                item: Box::new(ValueExpr::Literal(JsonValue::String("cd".into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Number(2.into()));
    }

    #[test]
    fn test_string_index_of_miss() {
        let result = eval_string_method(
            "abc",
            ArrayMethodCall::IndexOf {
                item: Box::new(ValueExpr::Literal(JsonValue::String("xyz".into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Number((-1_i64).into()));
    }

    #[test]
    fn test_string_index_of_non_string_arg() {
        let result = eval_string_method(
            "abc",
            ArrayMethodCall::IndexOf {
                item: Box::new(ValueExpr::Literal(JsonValue::Number(1.into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::Number((-1_i64).into()));
    }

    #[test]
    fn test_string_slice() {
        let result = eval_string_method(
            "hello world",
            ArrayMethodCall::Slice {
                start: 0,
                end: Some(5),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::String("hello".into()));
    }

    #[test]
    fn test_string_slice_no_end() {
        let result = eval_string_method(
            "hello world",
            ArrayMethodCall::Slice {
                start: 6,
                end: None,
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::String("world".into()));
    }

    #[test]
    fn test_string_slice_past_end() {
        let result = eval_string_method(
            "hi",
            ArrayMethodCall::Slice {
                start: 0,
                end: Some(100),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::String("hi".into()));
    }

    #[test]
    fn test_string_concat() {
        let result = eval_string_method(
            "hello",
            ArrayMethodCall::Concat {
                other: Box::new(ValueExpr::Literal(JsonValue::String(" world".into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::String("hello world".into()));
    }

    #[test]
    fn test_string_concat_with_number() {
        let result = eval_string_method(
            "count: ",
            ArrayMethodCall::Concat {
                other: Box::new(ValueExpr::Literal(JsonValue::Number(42.into()))),
            },
        )
        .unwrap();
        assert_eq!(result, JsonValue::String("count: 42".into()));
    }

    #[test]
    fn test_string_map_errors() {
        let result = eval_string_method(
            "hello",
            ArrayMethodCall::Map {
                item_var: "x".into(),
                body: Box::new(ValueExpr::Variable("x".into())),
            },
        );
        assert!(result.is_err());
        match result {
            Err(ExecutionError::RuntimeError { message }) => {
                assert!(message.contains("String does not support .map()"));
            },
            _ => panic!("Expected RuntimeError"),
        }
    }

    #[test]
    fn test_string_filter_errors() {
        let result = eval_string_method(
            "hello",
            ArrayMethodCall::Filter {
                item_var: "x".into(),
                predicate: Box::new(ValueExpr::Literal(JsonValue::Bool(true))),
            },
        );
        assert!(result.is_err());
        match result {
            Err(ExecutionError::RuntimeError { message }) => {
                assert!(message.contains("String does not support .filter()"));
            },
            _ => panic!("Expected RuntimeError"),
        }
    }

    #[test]
    fn test_array_methods_still_work_after_string_dispatch() {
        // Regression: ensure array .includes() still works
        let mut vars = HashMap::new();
        vars.insert(
            "arr".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(1.into()),
                JsonValue::Number(2.into()),
                JsonValue::Number(3.into()),
            ]),
        );
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("arr".to_string())),
            method: ArrayMethodCall::Includes {
                item: Box::new(ValueExpr::Literal(JsonValue::Number(2.into()))),
            },
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Bool(true));
    }

    #[test]
    fn test_array_length_still_works() {
        let mut vars = HashMap::new();
        vars.insert(
            "arr".to_string(),
            JsonValue::Array(vec![JsonValue::Null; 4]),
        );
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("arr".to_string())),
            method: ArrayMethodCall::Length,
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Number(4.into()));
    }

    // =========================================================================
    // Built-in function tests
    // =========================================================================

    #[test]
    fn test_parse_float() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ParseFloat,
            args: vec![ValueExpr::Literal(JsonValue::String("3.14".into()))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        // Why: test fixture uses 3.14 as a representative non-integer parse target,
        // not the mathematical PI constant — clippy::approx_constant is a false positive here.
        #[allow(clippy::approx_constant)]
        let expected = serde_json::json!(3.14);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_float_integer() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ParseFloat,
            args: vec![ValueExpr::Literal(JsonValue::String("42".into()))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(42.0));
    }

    #[test]
    fn test_parse_float_nan_returns_null() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ParseFloat,
            args: vec![ValueExpr::Literal(JsonValue::String("not-a-number".into()))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_parse_int() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ParseInt,
            args: vec![ValueExpr::Literal(JsonValue::String("42.9".into()))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    #[test]
    fn test_parse_int_nan_returns_null() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ParseInt,
            args: vec![ValueExpr::Literal(JsonValue::String("abc".into()))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_math_abs() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathAbs,
            args: vec![ValueExpr::Literal(serde_json::json!(-5.0))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(5.0));
    }

    #[test]
    fn test_math_max() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathMax,
            args: vec![
                ValueExpr::Literal(serde_json::json!(1)),
                ValueExpr::Literal(serde_json::json!(5)),
                ValueExpr::Literal(serde_json::json!(3)),
            ],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(5.0));
    }

    #[test]
    fn test_math_min() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathMin,
            args: vec![
                ValueExpr::Literal(serde_json::json!(10)),
                ValueExpr::Literal(serde_json::json!(2)),
                ValueExpr::Literal(serde_json::json!(7)),
            ],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(2.0));
    }

    #[test]
    fn test_math_round() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathRound,
            args: vec![ValueExpr::Literal(serde_json::json!(3.7))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(4.0));
    }

    #[test]
    fn test_math_floor() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathFloor,
            args: vec![ValueExpr::Literal(serde_json::json!(3.7))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(3.0));
    }

    #[test]
    fn test_math_ceil() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::MathCeil,
            args: vec![ValueExpr::Literal(serde_json::json!(3.2))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(4.0));
    }

    #[test]
    fn test_object_keys() {
        let mut vars = HashMap::new();
        vars.insert("obj".to_string(), serde_json::json!({"a": 1, "b": 2}));
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ObjectKeys,
            args: vec![ValueExpr::Variable("obj".to_string())],
        };
        let result = evaluate(&expr, &vars).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr.contains(&JsonValue::String("a".into())));
        assert!(arr.contains(&JsonValue::String("b".into())));
    }

    #[test]
    fn test_object_values() {
        let mut vars = HashMap::new();
        vars.insert("obj".to_string(), serde_json::json!({"x": 10, "y": 20}));
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ObjectValues,
            args: vec![ValueExpr::Variable("obj".to_string())],
        };
        let result = evaluate(&expr, &vars).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr.contains(&serde_json::json!(10)));
        assert!(arr.contains(&serde_json::json!(20)));
    }

    #[test]
    fn test_object_entries() {
        let mut vars = HashMap::new();
        vars.insert("obj".to_string(), serde_json::json!({"key": "val"}));
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ObjectEntries,
            args: vec![ValueExpr::Variable("obj".to_string())],
        };
        let result = evaluate(&expr, &vars).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], serde_json::json!(["key", "val"]));
    }

    #[test]
    fn test_object_keys_non_object() {
        let vars = HashMap::new();
        let expr = ValueExpr::BuiltinCall {
            func: BuiltinFunction::ObjectKeys,
            args: vec![ValueExpr::Literal(serde_json::json!(42))],
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Array(vec![]));
    }

    // =========================================================================
    // Unary plus tests
    // =========================================================================

    #[test]
    fn test_unary_plus_string_to_number() {
        let vars = HashMap::new();
        let expr = ValueExpr::UnaryOp {
            op: UnaryOperator::Plus,
            operand: Box::new(ValueExpr::Literal(JsonValue::String("42".into()))),
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(42.0));
    }

    #[test]
    fn test_unary_plus_nan_returns_null() {
        let vars = HashMap::new();
        let expr = ValueExpr::UnaryOp {
            op: UnaryOperator::Plus,
            operand: Box::new(ValueExpr::Literal(JsonValue::String("abc".into()))),
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    // =========================================================================
    // Sort with comparator tests
    // =========================================================================

    #[test]
    fn test_sort_ascending_comparator() {
        let mut vars = HashMap::new();
        vars.insert("arr".to_string(), serde_json::json!([3, 1, 4, 1, 5]));
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("arr".to_string())),
            method: ArrayMethodCall::Sort {
                comparator: Some((
                    "a".to_string(),
                    "b".to_string(),
                    Box::new(ValueExpr::BinaryOp {
                        left: Box::new(ValueExpr::Variable("a".to_string())),
                        op: BinaryOperator::Sub,
                        right: Box::new(ValueExpr::Variable("b".to_string())),
                    }),
                )),
            },
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!([1, 1, 3, 4, 5]));
    }

    #[test]
    fn test_sort_descending_comparator() {
        let mut vars = HashMap::new();
        vars.insert("arr".to_string(), serde_json::json!([3, 1, 4, 1, 5]));
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("arr".to_string())),
            method: ArrayMethodCall::Sort {
                comparator: Some((
                    "a".to_string(),
                    "b".to_string(),
                    Box::new(ValueExpr::BinaryOp {
                        left: Box::new(ValueExpr::Variable("b".to_string())),
                        op: BinaryOperator::Sub,
                        right: Box::new(ValueExpr::Variable("a".to_string())),
                    }),
                )),
            },
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!([5, 4, 3, 1, 1]));
    }

    #[test]
    fn test_sort_default_string_sort() {
        let mut vars = HashMap::new();
        vars.insert(
            "arr".to_string(),
            serde_json::json!(["banana", "apple", "cherry"]),
        );
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("arr".to_string())),
            method: ArrayMethodCall::Sort { comparator: None },
        };
        let result = evaluate(&expr, &vars).unwrap();
        assert_eq!(result, serde_json::json!(["apple", "banana", "cherry"]));
    }

    // =========================================================================
    // Scope-chain push/pop optimization tests
    // =========================================================================

    #[test]
    fn test_array_map_large_scope_performance() {
        // Regression test: verifies push/pop optimization handles large arrays
        // with many scope variables without excessive allocation.
        // Before optimization: 1000 HashMap clones (one per element).
        // After optimization: 1 HashMap clone (at array method entry).
        use std::time::Instant;

        let mut vars = HashMap::new();
        // Pre-populate scope with 20 variables to make clone cost measurable
        for i in 0..20 {
            vars.insert(
                format!("var_{i}"),
                JsonValue::Number(serde_json::Number::from(i)),
            );
        }

        // Build a 1000-element array
        let arr: Vec<JsonValue> = (0..1000)
            .map(|i| JsonValue::Number(serde_json::Number::from(i)))
            .collect();
        vars.insert("data".to_string(), JsonValue::Array(arr));

        // Simple map: x => x (identity)
        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("data".to_string())),
            method: ArrayMethodCall::Map {
                item_var: "x".to_string(),
                body: Box::new(ValueExpr::Variable("x".to_string())),
            },
        };

        let start = Instant::now();
        let result = evaluate(&expr, &vars).unwrap();
        let elapsed = start.elapsed();

        let arr_result = result.as_array().unwrap();
        assert_eq!(arr_result.len(), 1000);
        // Verify first and last element values
        assert_eq!(arr_result[0], JsonValue::Number(0.into()));
        assert_eq!(arr_result[999], JsonValue::Number(999.into()));

        // Sanity check: should complete in under 100ms even on slow CI
        assert!(
            elapsed.as_millis() < 100,
            "Array map took {}ms, expected < 100ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_array_filter_preserves_outer_scope() {
        // Verifies that push/pop correctly restores scope after filter.
        // The outer variable "x" should be unchanged after the filter runs.
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), JsonValue::String("outer".into()));
        vars.insert(
            "arr".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(1.into()),
                JsonValue::Number(2.into()),
                JsonValue::Number(3.into()),
            ]),
        );

        // Block: { filter using x as item_var, then return outer x }
        let expr = ValueExpr::Block {
            bindings: vec![(
                "filtered".to_string(),
                ValueExpr::ArrayMethod {
                    array: Box::new(ValueExpr::Variable("arr".to_string())),
                    method: ArrayMethodCall::Filter {
                        item_var: "x".to_string(),
                        predicate: Box::new(ValueExpr::BinaryOp {
                            left: Box::new(ValueExpr::Variable("x".to_string())),
                            op: BinaryOperator::Gt,
                            right: Box::new(ValueExpr::Literal(JsonValue::Number(1.into()))),
                        }),
                    },
                },
            )],
            result: Box::new(ValueExpr::Variable("x".to_string())),
        };

        let result = evaluate(&expr, &vars).unwrap();
        // The outer "x" should still be "outer", not overwritten by the filter
        assert_eq!(result, JsonValue::String("outer".into()));
    }

    #[test]
    fn test_array_reduce_large_scope_performance() {
        // Regression test for reduce with push/pop optimization at scale.
        let mut vars = HashMap::new();
        for i in 0..20 {
            vars.insert(
                format!("var_{i}"),
                JsonValue::Number(serde_json::Number::from(i)),
            );
        }

        // Sum 1000 numbers using reduce
        let arr: Vec<JsonValue> = (0..1000)
            .map(|i| JsonValue::Number(serde_json::Number::from(i)))
            .collect();
        vars.insert("data".to_string(), JsonValue::Array(arr));

        let expr = ValueExpr::ArrayMethod {
            array: Box::new(ValueExpr::Variable("data".to_string())),
            method: ArrayMethodCall::Reduce {
                acc_var: "acc".to_string(),
                item_var: "x".to_string(),
                body: Box::new(ValueExpr::BinaryOp {
                    left: Box::new(ValueExpr::Variable("acc".to_string())),
                    op: BinaryOperator::Add,
                    right: Box::new(ValueExpr::Variable("x".to_string())),
                }),
                initial: Box::new(ValueExpr::Literal(JsonValue::Number(0.into()))),
            },
        };

        let result = evaluate(&expr, &vars).unwrap();
        // Sum of 0..1000 = 999 * 1000 / 2 = 499500
        // Note: the evaluator's Add operation converts to f64, so the result is 499500.0
        assert_eq!(result, serde_json::json!(499_500.0));
    }
}
