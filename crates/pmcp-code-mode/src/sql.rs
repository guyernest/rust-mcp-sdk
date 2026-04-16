//! SQL validation for Code Mode.
//!
//! Parses SQL statements with [`sqlparser`], classifies the statement type
//! (`SELECT`/`INSERT`/`UPDATE`/`DELETE`/DDL), and extracts the tables, columns,
//! and structural metadata that the Cedar policy evaluator needs.
//!
//! Gated behind the `sql-code-mode` feature.

use crate::types::{
    CodeType, Complexity, SecurityAnalysis, SecurityIssue, SecurityIssueType, ValidationError,
};
use sqlparser::ast::{
    AssignmentTarget, Expr, FromTable, GroupByExpr, Join, LimitClause, ObjectName, Query, Select,
    SelectItem, SetExpr, Statement, TableFactor, TableObject, TableWithJoins,
};
use sqlparser::dialect::{Dialect, GenericDialect};
use sqlparser::parser::Parser;
use std::collections::HashSet;

/// High-level category of a SQL statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlStatementType {
    /// `SELECT`, `SHOW`, `EXPLAIN`, `DESCRIBE`
    Select,
    /// `INSERT`
    Insert,
    /// `UPDATE`, `MERGE`
    Update,
    /// `DELETE`, `TRUNCATE`
    Delete,
    /// `CREATE`/`ALTER`/`DROP`/`GRANT`/`REVOKE` (DDL/admin)
    Ddl,
    /// Unrecognized or unsupported statement
    Other,
}

impl SqlStatementType {
    /// The canonical uppercase string ("SELECT", "INSERT", etc.) used by
    /// the Cedar schema and [`UnifiedAction::from_sql`](crate::UnifiedAction::from_sql).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Select => "SELECT",
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Ddl => "DDL",
            Self::Other => "OTHER",
        }
    }

    /// Whether this statement is read-only.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Select)
    }

    /// Whether this statement writes data (INSERT/UPDATE).
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Insert | Self::Update)
    }

    /// Whether this statement deletes data (DELETE/TRUNCATE).
    pub fn is_delete(&self) -> bool {
        matches!(self, Self::Delete)
    }

    /// Whether this statement changes schema or permissions.
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Ddl)
    }
}

/// Structural information extracted from a parsed SQL statement.
#[derive(Debug, Clone)]
pub struct SqlStatementInfo {
    /// High-level statement category.
    pub statement_type: SqlStatementType,

    /// Raw uppercase verb ("SELECT", "INSERT", "CREATE TABLE", etc.) — used
    /// for explanations. For Cedar entity building use [`Self::statement_type`].
    pub verb: String,

    /// All tables referenced by name (final path segment if qualified).
    pub tables: HashSet<String>,

    /// All columns referenced (where determinable). `*` recorded for wildcards.
    pub columns: HashSet<String>,

    /// Whether the statement has a `WHERE` clause.
    pub has_where: bool,

    /// Whether the statement has a `LIMIT` clause.
    pub has_limit: bool,

    /// Whether the statement has an `ORDER BY` clause.
    pub has_order_by: bool,

    /// Whether the statement includes `GROUP BY` or aggregate functions.
    pub has_aggregation: bool,

    /// Number of `JOIN` clauses across all FROM items.
    pub join_count: u32,

    /// Number of subqueries (naive count of nested SELECTs).
    pub subquery_count: u32,

    /// Row-count estimate: `LIMIT n` when present, otherwise a configurable default.
    pub estimated_rows: u64,

    /// Raw length of the SQL string (characters).
    pub sql_length: usize,
}

/// SQL validator that parses and analyzes SQL statements.
#[derive(Debug, Clone)]
pub struct SqlValidator {
    dialect: DialectBox,
    default_row_estimate: u64,
}

impl Default for SqlValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlValidator {
    /// Create a new SQL validator with the generic ANSI dialect.
    pub fn new() -> Self {
        Self {
            dialect: DialectBox::Generic,
            default_row_estimate: 1000,
        }
    }

    /// Parse SQL and extract statement info.
    ///
    /// Returns an error if the SQL fails to parse, is empty, or contains
    /// multiple statements (SQL Code Mode validates one statement at a time).
    pub fn validate(&self, sql: &str) -> Result<SqlStatementInfo, ValidationError> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::ParseError {
                message: "SQL statement is empty".to_string(),
                line: 1,
                column: 1,
            });
        }

        let statements = Parser::parse_sql(self.dialect.as_dialect(), trimmed).map_err(|e| {
            ValidationError::ParseError {
                message: format!("SQL parse error: {}", e),
                line: 1,
                column: 1,
            }
        })?;

        match statements.len() {
            0 => Err(ValidationError::ParseError {
                message: "SQL contains no statements".to_string(),
                line: 1,
                column: 1,
            }),
            1 => Ok(self.analyze_statement(&statements[0], trimmed)),
            n => Err(ValidationError::ParseError {
                message: format!("SQL Code Mode validates one statement at a time; got {}", n),
                line: 1,
                column: 1,
            }),
        }
    }

    /// Produce a security analysis for the given statement info.
    ///
    /// The issues produced here are warnings only — config-level and
    /// policy-level authorization are enforced separately in
    /// [`ValidationPipeline::validate_sql_query`](crate::ValidationPipeline::validate_sql_query).
    pub fn analyze_security(&self, info: &SqlStatementInfo) -> SecurityAnalysis {
        let mut issues: Vec<SecurityIssue> = Vec::new();

        // UPDATE/DELETE without WHERE affects every row — classify as UnboundedQuery.
        if (info.statement_type.is_write() || info.statement_type.is_delete()) && !info.has_where {
            issues.push(SecurityIssue::new(
                SecurityIssueType::UnboundedQuery,
                format!(
                    "{} statement has no WHERE clause — affects all rows in the table",
                    info.verb
                ),
            ));
        }

        // Pure SELECT without LIMIT is also unbounded.
        if info.statement_type.is_read_only() && !info.has_limit {
            issues.push(SecurityIssue::new(
                SecurityIssueType::UnboundedQuery,
                format!(
                    "{} statement has no LIMIT — result set may be large",
                    info.verb
                ),
            ));
        }

        // Excessive joins or subqueries — complexity signal.
        if info.join_count > 5 {
            issues.push(SecurityIssue::new(
                SecurityIssueType::HighComplexity,
                format!(
                    "Query has {} JOINs, which may be expensive to execute",
                    info.join_count
                ),
            ));
        }
        if info.subquery_count > 3 {
            issues.push(SecurityIssue::new(
                SecurityIssueType::DeepNesting,
                format!("Query has {} nested subqueries", info.subquery_count),
            ));
        }

        let complexity = estimate_complexity(info);

        SecurityAnalysis {
            is_read_only: info.statement_type.is_read_only(),
            tables_accessed: info.tables.clone(),
            fields_accessed: info.columns.clone(),
            has_aggregation: info.has_aggregation,
            has_subqueries: info.subquery_count > 0,
            estimated_complexity: complexity,
            potential_issues: issues,
            estimated_rows: Some(info.estimated_rows),
        }
    }

    /// Map parsed statement info to [`CodeType`].
    pub fn to_code_type(&self, info: &SqlStatementInfo) -> CodeType {
        if info.statement_type.is_read_only() {
            CodeType::SqlQuery
        } else {
            CodeType::SqlMutation
        }
    }

    fn analyze_statement(&self, stmt: &Statement, sql: &str) -> SqlStatementInfo {
        let mut info = SqlStatementInfo {
            statement_type: SqlStatementType::Other,
            verb: verb_for(stmt),
            tables: HashSet::new(),
            columns: HashSet::new(),
            has_where: false,
            has_limit: false,
            has_order_by: false,
            has_aggregation: false,
            join_count: 0,
            subquery_count: 0,
            estimated_rows: self.default_row_estimate,
            sql_length: sql.len(),
        };

        match stmt {
            Statement::Query(query) => {
                info.statement_type = SqlStatementType::Select;
                self.analyze_query(query, &mut info);
            },
            Statement::Insert(insert) => {
                info.statement_type = SqlStatementType::Insert;
                if let TableObject::TableName(name) = &insert.table {
                    add_object_name(&mut info.tables, name);
                }
                for col in &insert.columns {
                    info.columns.insert(col.value.clone());
                }
                if let Some(source) = &insert.source {
                    self.analyze_query(source, &mut info);
                }
            },
            Statement::Update(update) => {
                info.statement_type = SqlStatementType::Update;
                self.analyze_table_with_joins(&update.table, &mut info);
                for assignment in &update.assignments {
                    match &assignment.target {
                        AssignmentTarget::ColumnName(name) => {
                            add_object_name(&mut info.columns, name);
                        },
                        AssignmentTarget::Tuple(names) => {
                            for n in names {
                                add_object_name(&mut info.columns, n);
                            }
                        },
                    }
                    self.analyze_expr(&assignment.value, &mut info);
                }
                if let Some(expr) = &update.selection {
                    info.has_where = true;
                    self.analyze_expr(expr, &mut info);
                }
            },
            Statement::Delete(delete) => {
                info.statement_type = SqlStatementType::Delete;
                match &delete.from {
                    FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => {
                        for t in tables {
                            self.analyze_table_with_joins(t, &mut info);
                        }
                    },
                }
                // Multi-table delete names
                for t in &delete.tables {
                    add_object_name(&mut info.tables, t);
                }
                if let Some(expr) = &delete.selection {
                    info.has_where = true;
                    self.analyze_expr(expr, &mut info);
                }
            },
            Statement::Truncate(truncate) => {
                info.statement_type = SqlStatementType::Delete;
                for tn in &truncate.table_names {
                    add_object_name(&mut info.tables, &tn.name);
                }
            },
            Statement::CreateTable(create) => {
                info.statement_type = SqlStatementType::Ddl;
                add_object_name(&mut info.tables, &create.name);
            },
            Statement::AlterTable(alter) => {
                info.statement_type = SqlStatementType::Ddl;
                add_object_name(&mut info.tables, &alter.name);
            },
            Statement::Drop { .. }
            | Statement::CreateIndex(_)
            | Statement::CreateView { .. }
            | Statement::Grant { .. }
            | Statement::Revoke { .. } => {
                info.statement_type = SqlStatementType::Ddl;
            },
            _ => {
                // Unknown statement — leave as Other.
            },
        }

        info
    }

    fn analyze_query(&self, query: &Query, info: &mut SqlStatementInfo) {
        if query.order_by.is_some() {
            info.has_order_by = true;
        }
        if let Some(limit_clause) = &query.limit_clause {
            info.has_limit = true;
            let limit_expr = match limit_clause {
                LimitClause::LimitOffset { limit, .. } => limit.as_ref(),
                LimitClause::OffsetCommaLimit { limit, .. } => Some(limit),
            };
            if let Some(Expr::Value(v)) = limit_expr {
                if let sqlparser::ast::Value::Number(n, _) = &v.value {
                    if let Ok(parsed) = n.parse::<u64>() {
                        info.estimated_rows = parsed;
                    }
                }
            }
        }

        self.analyze_set_expr(&query.body, info);
    }

    fn analyze_set_expr(&self, set_expr: &SetExpr, info: &mut SqlStatementInfo) {
        match set_expr {
            SetExpr::Select(select) => self.analyze_select(select, info),
            SetExpr::Query(inner) => {
                info.subquery_count += 1;
                self.analyze_query(inner, info);
            },
            SetExpr::SetOperation { left, right, .. } => {
                self.analyze_set_expr(left, info);
                self.analyze_set_expr(right, info);
            },
            _ => {},
        }
    }

    fn analyze_select(&self, select: &Select, info: &mut SqlStatementInfo) {
        // Projection columns
        for item in &select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => self.analyze_expr(expr, info),
                SelectItem::ExprWithAlias { expr, .. } => self.analyze_expr(expr, info),
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                    info.columns.insert("*".to_string());
                },
            }
        }

        // FROM tables + joins
        for table in &select.from {
            self.analyze_table_with_joins(table, info);
        }

        // WHERE
        if let Some(expr) = &select.selection {
            info.has_where = true;
            self.analyze_expr(expr, info);
        }

        // GROUP BY / aggregation
        if !group_by_is_empty(&select.group_by) {
            info.has_aggregation = true;
        }
    }

    fn analyze_table_with_joins(&self, item: &TableWithJoins, info: &mut SqlStatementInfo) {
        self.analyze_table_factor(&item.relation, info);
        for join in &item.joins {
            info.join_count += 1;
            self.analyze_join(join, info);
        }
    }

    fn analyze_join(&self, join: &Join, info: &mut SqlStatementInfo) {
        self.analyze_table_factor(&join.relation, info);
    }

    fn analyze_table_factor(&self, factor: &TableFactor, info: &mut SqlStatementInfo) {
        match factor {
            TableFactor::Table { name, .. } => add_object_name(&mut info.tables, name),
            TableFactor::Derived { subquery, .. } => {
                info.subquery_count += 1;
                self.analyze_query(subquery, info);
            },
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => self.analyze_table_with_joins(table_with_joins, info),
            _ => {},
        }
    }

    fn analyze_expr(&self, expr: &Expr, info: &mut SqlStatementInfo) {
        match expr {
            Expr::Identifier(id) => {
                info.columns.insert(id.value.clone());
            },
            Expr::CompoundIdentifier(ids) => {
                if let Some(last) = ids.last() {
                    info.columns.insert(last.value.clone());
                }
            },
            Expr::Subquery(q)
            | Expr::Exists { subquery: q, .. }
            | Expr::InSubquery { subquery: q, .. } => {
                info.subquery_count += 1;
                self.analyze_query(q, info);
            },
            Expr::Function(f) => {
                let name = f.name.to_string().to_uppercase();
                if matches!(
                    name.as_str(),
                    "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "ARRAY_AGG" | "STRING_AGG"
                ) {
                    info.has_aggregation = true;
                }
            },
            _ => {},
        }
    }
}

fn estimate_complexity(info: &SqlStatementInfo) -> Complexity {
    let joins = info.join_count;
    let subs = info.subquery_count;
    if joins >= 5 || subs >= 3 {
        Complexity::High
    } else if joins >= 2 || subs >= 1 || info.has_aggregation {
        Complexity::Medium
    } else {
        Complexity::Low
    }
}

fn group_by_is_empty(group_by: &GroupByExpr) -> bool {
    match group_by {
        GroupByExpr::All(_) => true,
        GroupByExpr::Expressions(exprs, _) => exprs.is_empty(),
    }
}

fn add_object_name(out: &mut HashSet<String>, name: &ObjectName) {
    if let Some(last) = name.0.last() {
        out.insert(last.to_string());
    } else {
        out.insert(name.to_string());
    }
}

fn verb_for(stmt: &Statement) -> String {
    match stmt {
        Statement::Query(_) => "SELECT".to_string(),
        Statement::Insert(_) => "INSERT".to_string(),
        Statement::Update { .. } => "UPDATE".to_string(),
        Statement::Delete(_) => "DELETE".to_string(),
        Statement::Truncate { .. } => "TRUNCATE".to_string(),
        Statement::CreateTable(_) => "CREATE TABLE".to_string(),
        Statement::AlterTable { .. } => "ALTER TABLE".to_string(),
        Statement::Drop { .. } => "DROP".to_string(),
        Statement::CreateIndex(_) => "CREATE INDEX".to_string(),
        Statement::CreateView { .. } => "CREATE VIEW".to_string(),
        Statement::Grant { .. } => "GRANT".to_string(),
        Statement::Revoke { .. } => "REVOKE".to_string(),
        other => format!("{:?}", other)
            .split('(')
            .next()
            .unwrap_or("OTHER")
            .to_uppercase(),
    }
}

/// Enum wrapper around concrete dialects so `SqlValidator` stays `Clone` and
/// avoids trait-object gymnastics.
#[derive(Debug, Clone)]
enum DialectBox {
    Generic,
}

impl DialectBox {
    fn as_dialect(&self) -> &dyn Dialect {
        match self {
            Self::Generic => &GenericDialect {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_simple() {
        let v = SqlValidator::new();
        let info = v.validate("SELECT id, name FROM users").unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Select);
        assert!(info.tables.contains("users"));
        assert!(info.columns.contains("id"));
        assert!(info.columns.contains("name"));
        assert!(!info.has_where);
        assert!(!info.has_limit);
    }

    #[test]
    fn select_with_where_limit_order() {
        let v = SqlValidator::new();
        let info = v
            .validate("SELECT id FROM users WHERE active = 1 ORDER BY id LIMIT 10")
            .unwrap();
        assert!(info.has_where);
        assert!(info.has_limit);
        assert!(info.has_order_by);
        assert_eq!(info.estimated_rows, 10);
    }

    #[test]
    fn select_star() {
        let v = SqlValidator::new();
        let info = v.validate("SELECT * FROM users").unwrap();
        assert!(info.columns.contains("*"));
    }

    #[test]
    fn select_join_and_subquery() {
        let v = SqlValidator::new();
        let info = v
            .validate(
                "SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id \
                 WHERE u.id IN (SELECT id FROM admins)",
            )
            .unwrap();
        assert_eq!(info.join_count, 1);
        assert!(info.subquery_count >= 1);
        assert!(info.tables.contains("users"));
        assert!(info.tables.contains("orders"));
        assert!(info.tables.contains("admins"));
    }

    #[test]
    fn insert_extracts_table_and_columns() {
        let v = SqlValidator::new();
        let info = v
            .validate("INSERT INTO users (id, name) VALUES (1, 'Alice')")
            .unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Insert);
        assert!(info.tables.contains("users"));
        assert!(info.columns.contains("id"));
        assert!(info.columns.contains("name"));
    }

    #[test]
    fn update_without_where_flagged() {
        let v = SqlValidator::new();
        let info = v.validate("UPDATE users SET active = 0").unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Update);
        assert!(!info.has_where);
        let sa = v.analyze_security(&info);
        assert!(sa
            .potential_issues
            .iter()
            .any(|i| i.issue_type == SecurityIssueType::UnboundedQuery));
    }

    #[test]
    fn update_with_where() {
        let v = SqlValidator::new();
        let info = v
            .validate("UPDATE users SET active = 0 WHERE id = 1")
            .unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Update);
        assert!(info.has_where);
        assert!(info.columns.contains("active"));
    }

    #[test]
    fn delete_with_where() {
        let v = SqlValidator::new();
        let info = v.validate("DELETE FROM users WHERE id = 1").unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Delete);
        assert!(info.has_where);
    }

    #[test]
    fn ddl_is_admin() {
        let v = SqlValidator::new();
        let info = v.validate("CREATE TABLE foo (id INT)").unwrap();
        assert_eq!(info.statement_type, SqlStatementType::Ddl);
        assert!(info.statement_type.is_admin());
    }

    #[test]
    fn empty_sql_rejected() {
        let v = SqlValidator::new();
        assert!(matches!(
            v.validate(""),
            Err(ValidationError::ParseError { .. })
        ));
        assert!(matches!(
            v.validate("   "),
            Err(ValidationError::ParseError { .. })
        ));
    }

    #[test]
    fn syntax_error_rejected() {
        let v = SqlValidator::new();
        assert!(matches!(
            v.validate("SELEC id FRM users"),
            Err(ValidationError::ParseError { .. })
        ));
    }

    #[test]
    fn multiple_statements_rejected() {
        let v = SqlValidator::new();
        assert!(matches!(
            v.validate("SELECT 1; SELECT 2"),
            Err(ValidationError::ParseError { .. })
        ));
    }

    #[test]
    fn aggregation_detected() {
        let v = SqlValidator::new();
        let info = v.validate("SELECT COUNT(*) FROM users").unwrap();
        assert!(info.has_aggregation);
    }

    #[test]
    fn group_by_detected() {
        let v = SqlValidator::new();
        let info = v
            .validate("SELECT role, COUNT(*) FROM users GROUP BY role")
            .unwrap();
        assert!(info.has_aggregation);
    }
}
