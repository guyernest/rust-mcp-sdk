//! Semantic regression baseline for `pmcp-code-mode`'s evaluator hotspots
//! (Phase 75 Wave 0 Task 2).
//!
//! Pins the EXACT `JsonValue` output of representative `ValueExpr` and
//! `ArrayMethodCall` programs against `evaluate_with_scope` and
//! `evaluate_array_method_with_scope`. This file is the regression contract
//! for Wave 3's mandatory refactor of the two highest-cog functions in the
//! whole repo:
//!
//! - `evaluate_with_scope` (eval.rs:59, current cog 123, must drop ≤25 per D-10-B)
//! - `evaluate_array_method_with_scope` (eval.rs:506, current cog 117, must drop ≤25 per D-10-B)
//!
//! The existing `tests/property_tests.rs` only asserts "doesn't panic" — it
//! cannot detect semantic drift where the refactor returns the wrong value.
//! This file fills that gap.
//!
//! Coverage minimum (per the plan):
//! - ≥12 per-variant tests for `evaluate_with_scope` (one per major
//!   `ValueExpr` variant)
//! - ≥6 per-method tests for `evaluate_array_method_with_scope` (one per
//!   common `ArrayMethodCall` variant)
//! - 20-30 corpus programs (real `PlanCompiler::compile_code` → evaluator
//!   round-trip, exercising the parser↔evaluator interaction the property
//!   tests can't catch)
//!
//! Wave 3's refactor must keep every `assert_eq!` payload byte-identical. If
//! Wave 3 finds a semantic bug (test produces unexpected value PRE-refactor),
//! record the discrepancy in `75-W0-SPIKE-RESULTS.md` rather than "fixing"
//! the test — the test pins CURRENT behavior, not desired behavior.

#![cfg(feature = "js-runtime")]

use pmcp_code_mode::eval::{evaluate_array_method_with_scope, evaluate_with_scope};
use pmcp_code_mode::executor::{
    ArrayMethodCall, BinaryOperator, ObjectField, PlanCompiler, PlanStep, UnaryOperator, ValueExpr,
};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

// ============================================================================
// Helpers
// ============================================================================

/// Empty global variable provider for tests that don't need globals.
fn empty_globals() -> HashMap<String, JsonValue> {
    HashMap::new()
}

/// Empty local scope for tests that don't have nested callbacks.
fn empty_locals() -> HashMap<String, JsonValue> {
    HashMap::new()
}

/// Convenience: evaluate an expression with no global / local context.
fn eval_isolated(expr: &ValueExpr) -> JsonValue {
    evaluate_with_scope(expr, &empty_globals(), &empty_locals())
        .expect("expression should evaluate without error")
}

/// Compile a JavaScript fragment of the form `return <expr>;` and extract the
/// returned `ValueExpr`. Used by the corpus tests to round-trip parser ↔
/// evaluator with the same source the production runtime sees.
fn compile_return_expr(js_source: &str) -> ValueExpr {
    let wrapped = format!("return {};", js_source);
    let mut compiler = PlanCompiler::new();
    let plan = compiler
        .compile_code(&wrapped)
        .unwrap_or_else(|err| panic!("compile failed for `{}`: {:?}", js_source, err));
    for step in plan.steps {
        if let PlanStep::Return { value } = step {
            return value;
        }
    }
    panic!("no Return step found for `{}`", js_source);
}

// ============================================================================
// Per-variant `evaluate_with_scope` tests (≥12 minimum per the plan)
// ============================================================================

#[test]
fn variant_literal_passthrough() {
    let expr = ValueExpr::Literal(json!(42));
    assert_eq!(eval_isolated(&expr), json!(42));
}

#[test]
fn variant_literal_string() {
    let expr = ValueExpr::Literal(json!("hello"));
    assert_eq!(eval_isolated(&expr), json!("hello"));
}

#[test]
fn variant_variable_local_scope_wins_over_global() {
    let mut globals = empty_globals();
    globals.insert("x".into(), json!(1));
    let mut locals = empty_locals();
    locals.insert("x".into(), json!(99));
    let expr = ValueExpr::Variable("x".into());
    let result =
        evaluate_with_scope(&expr, &globals, &locals).expect("variable lookup should succeed");
    assert_eq!(result, json!(99));
}

#[test]
fn variant_variable_global_fallback() {
    let mut globals = empty_globals();
    globals.insert("answer".into(), json!(42));
    let expr = ValueExpr::Variable("answer".into());
    let result = evaluate_with_scope(&expr, &globals, &empty_locals())
        .expect("global variable lookup should succeed");
    assert_eq!(result, json!(42));
}

#[test]
fn variant_variable_undefined_builtin_returns_null() {
    let expr = ValueExpr::Variable("undefined".into());
    assert_eq!(eval_isolated(&expr), JsonValue::Null);
}

#[test]
fn variant_property_access_returns_value() {
    let expr = ValueExpr::PropertyAccess {
        object: Box::new(ValueExpr::Literal(json!({"a": 1, "b": 2}))),
        property: "b".into(),
    };
    assert_eq!(eval_isolated(&expr), json!(2));
}

#[test]
fn variant_property_access_missing_returns_null() {
    let expr = ValueExpr::PropertyAccess {
        object: Box::new(ValueExpr::Literal(json!({"a": 1}))),
        property: "missing".into(),
    };
    assert_eq!(eval_isolated(&expr), JsonValue::Null);
}

#[test]
fn variant_array_index_in_bounds() {
    let expr = ValueExpr::ArrayIndex {
        array: Box::new(ValueExpr::Literal(json!([10, 20, 30]))),
        index: Box::new(ValueExpr::Literal(json!(1))),
    };
    assert_eq!(eval_isolated(&expr), json!(20));
}

#[test]
fn variant_object_literal_evaluates_fields() {
    let expr = ValueExpr::ObjectLiteral {
        fields: vec![
            ObjectField::KeyValue {
                key: "n".into(),
                value: ValueExpr::Literal(json!(5)),
            },
            ObjectField::KeyValue {
                key: "msg".into(),
                value: ValueExpr::Literal(json!("hi")),
            },
        ],
    };
    assert_eq!(eval_isolated(&expr), json!({"n": 5, "msg": "hi"}));
}

#[test]
fn variant_array_literal_evaluates_each_item() {
    let expr = ValueExpr::ArrayLiteral {
        items: vec![
            ValueExpr::Literal(json!(1)),
            ValueExpr::Literal(json!(2)),
            ValueExpr::Literal(json!(3)),
        ],
    };
    assert_eq!(eval_isolated(&expr), json!([1, 2, 3]));
}

#[test]
fn variant_binop_addition_numeric() {
    // The evaluator promotes integer arithmetic results to f64, so the JSON
    // payload is `Number(5.0)` not `Number(5)`. Pinning to `5.0` matches
    // current behavior — a Wave 3 refactor that switches numeric promotion
    // would have to update this test (intentional break, not silent drift).
    let expr = ValueExpr::BinaryOp {
        left: Box::new(ValueExpr::Literal(json!(2))),
        op: BinaryOperator::Add,
        right: Box::new(ValueExpr::Literal(json!(3))),
    };
    assert_eq!(eval_isolated(&expr), json!(5.0));
}

#[test]
fn variant_binop_multiplication() {
    let expr = ValueExpr::BinaryOp {
        left: Box::new(ValueExpr::Literal(json!(4))),
        op: BinaryOperator::Mul,
        right: Box::new(ValueExpr::Literal(json!(7))),
    };
    assert_eq!(eval_isolated(&expr), json!(28.0));
}

#[test]
fn variant_binop_strict_equality_true() {
    let expr = ValueExpr::BinaryOp {
        left: Box::new(ValueExpr::Literal(json!(5))),
        op: BinaryOperator::StrictEq,
        right: Box::new(ValueExpr::Literal(json!(5))),
    };
    assert_eq!(eval_isolated(&expr), json!(true));
}

#[test]
fn variant_unary_not() {
    let expr = ValueExpr::UnaryOp {
        op: UnaryOperator::Not,
        operand: Box::new(ValueExpr::Literal(json!(false))),
    };
    assert_eq!(eval_isolated(&expr), json!(true));
}

#[test]
fn variant_unary_neg() {
    let expr = ValueExpr::UnaryOp {
        op: UnaryOperator::Neg,
        operand: Box::new(ValueExpr::Literal(json!(7))),
    };
    assert_eq!(eval_isolated(&expr), json!(-7.0));
}

#[test]
fn variant_ternary_truthy_branch() {
    let expr = ValueExpr::Ternary {
        condition: Box::new(ValueExpr::Literal(json!(true))),
        consequent: Box::new(ValueExpr::Literal(json!("yes"))),
        alternate: Box::new(ValueExpr::Literal(json!("no"))),
    };
    assert_eq!(eval_isolated(&expr), json!("yes"));
}

#[test]
fn variant_ternary_falsy_branch() {
    let expr = ValueExpr::Ternary {
        condition: Box::new(ValueExpr::Literal(json!(0))),
        consequent: Box::new(ValueExpr::Literal(json!("yes"))),
        alternate: Box::new(ValueExpr::Literal(json!("no"))),
    };
    assert_eq!(eval_isolated(&expr), json!("no"));
}

#[test]
fn variant_optional_chain_on_object_returns_value() {
    let expr = ValueExpr::OptionalChain {
        object: Box::new(ValueExpr::Literal(json!({"a": 5}))),
        property: "a".into(),
    };
    assert_eq!(eval_isolated(&expr), json!(5));
}

#[test]
fn variant_optional_chain_on_null_returns_null() {
    let expr = ValueExpr::OptionalChain {
        object: Box::new(ValueExpr::Literal(JsonValue::Null)),
        property: "anything".into(),
    };
    assert_eq!(eval_isolated(&expr), JsonValue::Null);
}

#[test]
fn variant_nullish_coalesce_left_is_null() {
    let expr = ValueExpr::NullishCoalesce {
        left: Box::new(ValueExpr::Literal(JsonValue::Null)),
        right: Box::new(ValueExpr::Literal(json!(42))),
    };
    assert_eq!(eval_isolated(&expr), json!(42));
}

#[test]
fn variant_nullish_coalesce_left_is_zero_keeps_zero() {
    // Zero is NOT nullish in JS (?? checks null/undefined only)
    let expr = ValueExpr::NullishCoalesce {
        left: Box::new(ValueExpr::Literal(json!(0))),
        right: Box::new(ValueExpr::Literal(json!(99))),
    };
    assert_eq!(eval_isolated(&expr), json!(0));
}

#[test]
fn variant_block_with_local_bindings() {
    let expr = ValueExpr::Block {
        bindings: vec![
            ("a".into(), ValueExpr::Literal(json!(2))),
            ("b".into(), ValueExpr::Literal(json!(3))),
        ],
        result: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("a".into())),
            op: BinaryOperator::Add,
            right: Box::new(ValueExpr::Variable("b".into())),
        }),
    };
    assert_eq!(eval_isolated(&expr), json!(5.0));
}

// ============================================================================
// Per-method `evaluate_array_method_with_scope` tests (≥6 minimum per the plan)
// ============================================================================

fn eval_method_isolated(arr: JsonValue, method: ArrayMethodCall) -> JsonValue {
    let globals = empty_globals();
    let mut locals = empty_locals();
    evaluate_array_method_with_scope(&arr, &method, &globals, &mut locals)
        .expect("array method should evaluate without error")
}

#[test]
fn array_method_length() {
    let result = eval_method_isolated(json!([1, 2, 3, 4]), ArrayMethodCall::Length);
    assert_eq!(result, json!(4));
}

#[test]
fn array_method_map_doubles_each_element() {
    let method = ArrayMethodCall::Map {
        item_var: "x".into(),
        body: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("x".into())),
            op: BinaryOperator::Mul,
            right: Box::new(ValueExpr::Literal(json!(2))),
        }),
    };
    let result = eval_method_isolated(json!([1, 2, 3]), method);
    assert_eq!(result, json!([2.0, 4.0, 6.0]));
}

#[test]
fn array_method_filter_keeps_evens() {
    let method = ArrayMethodCall::Filter {
        item_var: "n".into(),
        predicate: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::BinaryOp {
                left: Box::new(ValueExpr::Variable("n".into())),
                op: BinaryOperator::Mod,
                right: Box::new(ValueExpr::Literal(json!(2))),
            }),
            op: BinaryOperator::StrictEq,
            right: Box::new(ValueExpr::Literal(json!(0))),
        }),
    };
    let result = eval_method_isolated(json!([1, 2, 3, 4, 5]), method);
    assert_eq!(result, json!([2, 4]));
}

#[test]
fn array_method_find_returns_first_match() {
    let method = ArrayMethodCall::Find {
        item_var: "x".into(),
        predicate: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("x".into())),
            op: BinaryOperator::Gt,
            right: Box::new(ValueExpr::Literal(json!(2))),
        }),
    };
    let result = eval_method_isolated(json!([1, 2, 3, 4]), method);
    assert_eq!(result, json!(3));
}

#[test]
fn array_method_some_true_when_any_match() {
    let method = ArrayMethodCall::Some {
        item_var: "x".into(),
        predicate: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("x".into())),
            op: BinaryOperator::Gt,
            right: Box::new(ValueExpr::Literal(json!(10))),
        }),
    };
    let result = eval_method_isolated(json!([1, 2, 11]), method);
    assert_eq!(result, json!(true));
}

#[test]
fn array_method_every_false_when_one_fails() {
    let method = ArrayMethodCall::Every {
        item_var: "x".into(),
        predicate: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("x".into())),
            op: BinaryOperator::Gt,
            right: Box::new(ValueExpr::Literal(json!(0))),
        }),
    };
    let result = eval_method_isolated(json!([1, 2, -1, 3]), method);
    assert_eq!(result, json!(false));
}

#[test]
fn array_method_reduce_sums_elements() {
    let method = ArrayMethodCall::Reduce {
        acc_var: "acc".into(),
        item_var: "x".into(),
        body: Box::new(ValueExpr::BinaryOp {
            left: Box::new(ValueExpr::Variable("acc".into())),
            op: BinaryOperator::Add,
            right: Box::new(ValueExpr::Variable("x".into())),
        }),
        initial: Box::new(ValueExpr::Literal(json!(0))),
    };
    let result = eval_method_isolated(json!([1, 2, 3, 4]), method);
    assert_eq!(result, json!(10.0));
}

#[test]
fn array_method_includes_returns_true() {
    let method = ArrayMethodCall::Includes {
        item: Box::new(ValueExpr::Literal(json!(3))),
    };
    let result = eval_method_isolated(json!([1, 2, 3]), method);
    assert_eq!(result, json!(true));
}

#[test]
fn array_method_join_default_separator() {
    let method = ArrayMethodCall::Join { separator: None };
    let result = eval_method_isolated(json!(["a", "b", "c"]), method);
    assert_eq!(result, json!("a,b,c"));
}

#[test]
fn array_method_concat_appends_other() {
    let method = ArrayMethodCall::Concat {
        other: Box::new(ValueExpr::Literal(json!([4, 5]))),
    };
    let result = eval_method_isolated(json!([1, 2, 3]), method);
    assert_eq!(result, json!([1, 2, 3, 4, 5]));
}

// ============================================================================
// Real-expression corpus (post-review revision — Codex Concern #9)
//
// Catches parser↔evaluator interaction bugs that pure AST construction
// tests cannot. Each entry is `(JS source returning a value, expected JSON)`.
// The corpus deliberately mixes arithmetic, object/array literals, ternary,
// nullish, optional chaining, array methods, builtins, and method chains so
// Wave 3's refactor exercise touches the same surface real callers do.
// ============================================================================

// Notes on corpus shape (CURRENT-behavior pinning, not desired-behavior):
//
// 1. The evaluator promotes integer arithmetic to f64; expected payloads use
//    `5.0` not `5` for any value that flows through `evaluate_binary_op`.
// 2. The compiler in `PlanCompiler::compile_code` does NOT yet support
//    nullish coalescing (`??`) — it returns
//    `UnsupportedExpression("nullish coalescing")`. The evaluator DOES support
//    `ValueExpr::NullishCoalesce`, so the variant_nullish_coalesce_* tests
//    above cover that path. Once the compiler grows nullish support, add
//    corpus entries for `null ?? 42` and `0 ?? 99`.
// 3. Bare object-literal expressions (`{ a: 1 }.b`) are statement-level
//    in JS so they need a parenthesized form (`({a:1}).b`) for the parser
//    to treat them as an expression. Use the parenthesized form below.
// 4. Some method chains on bare object literals don't survive `return ...;`
//    wrapping; use `({...})` for any expression starting with `{`.
const CORPUS: &[(&str, &str)] = &[
    // --- Arithmetic (results promoted to f64 by evaluator)
    ("1 + 2", "3.0"),
    ("10 - 3", "7.0"),
    ("4 * 5", "20.0"),
    ("9 / 2", "4.5"),
    ("11 % 3", "2.0"),
    ("-7", "-7.0"),
    // --- Comparison + logical
    ("3 === 3", "true"),
    ("3 === 4", "false"),
    ("5 > 3", "true"),
    ("(1 + 1) === 2", "true"),
    // --- Strings
    (r#""hello" + " " + "world""#, r#""hello world""#),
    (r#""abc".toUpperCase()"#, r#""ABC""#),
    (r#""  trim  ".trim()"#, r#""trim""#),
    (r#""a,b,c".split(",")"#, r#"["a", "b", "c"]"#),
    // --- Arrays + methods
    ("[1, 2, 3].length", "3"),
    ("[1, 2, 3].map(x => x * 2)", "[2.0, 4.0, 6.0]"),
    ("[1, 2, 3, 4].filter(n => n % 2 === 0)", "[2, 4]"),
    ("[1, 2, 3].reduce((a, x) => a + x, 0)", "6.0"),
    ("[1, 2, 3].includes(2)", "true"),
    ("[1, 2, 3].slice(0, 2)", "[1, 2]"),
    ("[1, [2, [3]]].flat()", "[1, 2, [3]]"),
    // --- Object literals + access (parenthesize so the parser sees expressions)
    ("({ a: 1, b: 2 }).b", "2"),
    ("({ x: 1, y: 2 })", "{\"x\": 1, \"y\": 2}"),
    // --- Ternary
    ("true ? 1 : 2", "1"),
    ("false ? 1 : 2", "2"),
    // --- Builtins
    ("Math.abs(-5)", "5.0"),
    ("Math.max(1, 2, 3)", "3.0"),
    ("Object.keys({ a: 1, b: 2 })", "[\"a\", \"b\"]"),
    ("parseFloat(\"3.14\")", "3.14"),
];

#[test]
fn corpus_evaluator_semantic_baseline() {
    let mut failures: Vec<String> = Vec::new();
    for (input, expected_str) in CORPUS {
        let expr = compile_return_expr(input);
        let actual = match evaluate_with_scope(&expr, &empty_globals(), &empty_locals()) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("eval failed for `{}`: {:?}", input, err));
                continue;
            },
        };
        let expected: JsonValue = serde_json::from_str(expected_str)
            .unwrap_or_else(|_| panic!("expected payload not valid JSON: `{}`", expected_str));

        if actual != expected {
            failures.push(format!(
                "mismatch for `{}`:\n  expected: {}\n  actual:   {}",
                input, expected, actual
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} corpus entries diverged from expected output:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// Sanity check: the corpus must remain ≥20 programs to satisfy the plan's
// minimum (20-30 entries). This keeps drift-detection density meaningful.
#[test]
fn corpus_size_meets_minimum() {
    assert!(
        CORPUS.len() >= 20,
        "corpus shrank below the Wave 0 minimum (≥20 programs); found {}",
        CORPUS.len()
    );
}
