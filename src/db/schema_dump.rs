use sqlx::postgres::PgRow;
use sqlx::{FromRow, PgPool, Row};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

const INDENT: &str = "    ";
pub const EXCLUDED_TABLES: &[&str] = &["_migrations_applied", "seed_info", "a", "test"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConstraintType {
    PrimaryKey,
    Unique,
    Check,
    ForeignKey,
    Other,
}

impl ConstraintType {
    pub fn from_pg_contype(raw: &str) -> Self {
        match raw {
            "p" => ConstraintType::PrimaryKey,
            "f" => ConstraintType::ForeignKey,
            "u" => ConstraintType::Unique,
            "c" => ConstraintType::Check,
            _ => ConstraintType::Other,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct EnumDef {
    typname: String,
    labels: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ColumnDef {
    table_name: String,
    column_name: String,
    column_type: String,
    not_null: bool,
    default_value: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct ConstraintDef {
    table_name: String,
    constraint_name: String,
    constraint_type: ConstraintType,
    definition: String,
    referenced_table: Option<String>,
}

impl<'r> FromRow<'r, PgRow> for ConstraintDef {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let contype_raw: &str = row.try_get("constraint_type")?;
        Ok(Self {
            table_name: row.try_get("table_name")?,
            constraint_name: row.try_get("constraint_name")?,
            constraint_type: ConstraintType::from_pg_contype(contype_raw),
            definition: row.try_get("definition")?,
            referenced_table: row.try_get("referenced_table")?,
        })
    }
}

#[derive(Debug, Clone, FromRow)]
struct IndexDef {
    index_def: String,
}

/// Fetch custom enum types defined in the public schema
async fn fetch_enums(pool: &PgPool) -> Result<Vec<EnumDef>, crate::Error> {
    let enums = sqlx::query_as::<_, EnumDef>(
        r#"
        SELECT t.typname, array_agg(e.enumlabel ORDER BY e.enumsortorder) AS labels
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        JOIN pg_enum e ON e.enumtypid = t.oid
        WHERE n.nspname = 'public'
        GROUP BY t.typname
        ORDER BY t.typname;
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(enums)
}

/// Fetch columns for all active public tables
async fn fetch_columns(pool: &PgPool) -> Result<Vec<ColumnDef>, crate::Error> {
    let cols = sqlx::query_as::<_, ColumnDef>(
        r#"
        SELECT
            c.relname AS table_name,
            a.attname AS column_name,
            format_type(a.atttypid, a.atttypmod) AS column_type,
            a.attnotnull AS not_null,
            pg_get_expr(d.adbin, d.adrelid) AS default_value,
            col_description(c.oid, a.attnum) AS description
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_attribute a ON a.attrelid = c.oid
        LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
        WHERE n.nspname = 'public'
          AND c.relkind = 'r'
          AND a.attnum > 0
          AND NOT a.attisdropped
          AND c.relname != ALL($1)
        ORDER BY c.relname, a.attnum;
        "#,
    )
    .bind(EXCLUDED_TABLES)
    .fetch_all(pool)
    .await?;

    Ok(cols)
}

/// Fetch primary keys, unique constraints, checks, and foreign keys
async fn fetch_constraints(pool: &PgPool) -> Result<Vec<ConstraintDef>, crate::Error> {
    let constraints = sqlx::query_as::<_, ConstraintDef>(
        r#"
        SELECT
            c.relname AS table_name,
            con.conname AS constraint_name,
            con.contype::text AS constraint_type,
            pg_get_constraintdef(con.oid) AS definition,
            conf.relname AS referenced_table
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_class conf ON conf.oid = con.confrelid
        WHERE n.nspname = 'public'
          AND con.contype IN ('p', 'u', 'c', 'f')
          AND c.relname != ALL($1);
        "#,
    )
    .bind(EXCLUDED_TABLES)
    .fetch_all(pool)
    .await?;

    Ok(constraints)
}

/// Fetch standalone secondary indexes (excluding indexes backing PKs and UNIQUE constraints)
async fn fetch_indexes(pool: &PgPool) -> Result<Vec<IndexDef>, crate::Error> {
    let indexes = sqlx::query_as::<_, IndexDef>(
        r#"
        SELECT
            pg_get_indexdef(idx.indexrelid) AS index_def
        FROM pg_index idx
        JOIN pg_class c ON c.oid = idx.indrelid
        JOIN pg_class ic ON ic.oid = idx.indexrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relname != ALL($1)
          AND NOT EXISTS (
              SELECT 1 FROM pg_constraint con WHERE con.conindid = idx.indexrelid
          )
        ORDER BY c.relname, ic.relname;
        "#,
    )
    .bind(EXCLUDED_TABLES)
    .fetch_all(pool)
    .await?;

    Ok(indexes)
}

/// Sort tables in topological dependency order based on foreign keys
fn sort_tables_topologically(
    tables: &[String],
    constraints: &[ConstraintDef],
) -> Vec<String> {
    let table_set: HashSet<String> = tables.iter().cloned().collect();

    // Map: table -> set of tables it depends on
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for t in tables {
        in_degree.insert(t.clone(), 0);
        dependents.insert(t.clone(), Vec::new());
    }

    for con in constraints {
        if con.constraint_type == ConstraintType::ForeignKey {
            if let Some(ref parent) = con.referenced_table {
                if table_set.contains(parent) && parent != &con.table_name {
                    dependents
                        .entry(parent.clone())
                        .or_default()
                        .push(con.table_name.clone());
                    *in_degree.entry(con.table_name.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut ready: VecDeque<String> = tables
        .iter()
        .filter(|t| in_degree.get(*t).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();

    let mut sorted = Vec::new();
    while let Some(node) = ready.pop_front() {
        sorted.push(node.clone());
        if let Some(children) = dependents.get(&node) {
            for child in children {
                if let Some(deg) = in_degree.get_mut(child) {
                    *deg -= 1;
                    if *deg == 0 {
                        ready.push_back(child.clone());
                    }
                }
            }
        }
    }

    // Append any tables not sorted (in case of cycles or isolated tables)
    for t in tables {
        if !sorted.contains(t) {
            sorted.push(t.clone());
        }
    }

    sorted
}

/// Canonicalize and uppercase PostgreSQL column types
fn normalize_column_type(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(base) = trimmed.strip_suffix("[]") {
        return format!("{}[]", normalize_column_type(base));
    }

    let lower = trimmed.to_lowercase();
    if let Some(rest) = lower.strip_prefix("character varying") {
        return format!("VARCHAR{}", rest.to_uppercase());
    }
    if let Some(rest) = lower.strip_prefix("character") {
        return format!("CHAR{}", rest.to_uppercase());
    }
    match lower.as_str() {
        "timestamp with time zone" => "TIMESTAMPTZ".to_string(),
        "timestamp without time zone" => "TIMESTAMP".to_string(),
        "time with time zone" => "TIMETZ".to_string(),
        "time without time zone" => "TIME".to_string(),
        "double precision" => "DOUBLE PRECISION".to_string(),
        other => other.to_uppercase(),
    }
}

/// Strip redundant outer parentheses if the expression is fully enclosed by them
fn strip_outer_parens(mut s: &str) -> &str {
    s = s.trim();
    while s.starts_with('(') && s.ends_with(')') {
        let mut depth = 0;
        let mut fully_wrapped = true;
        for (i, c) in s.char_indices() {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 && i < s.len() - 1 {
                    fully_wrapped = false;
                    break;
                }
            }
        }
        if fully_wrapped && depth == 0 {
            s = s[1..s.len() - 1].trim();
        } else {
            break;
        }
    }
    s
}

/// Normalize default expressions by stripping redundant type casts (e.g. `'pending'::text` -> `'pending'`, `'{}'::text[]` -> `'{}'`)
fn normalize_default_value(raw: &str) -> String {
    let s = strip_outer_parens(raw);

    if s.starts_with('\'') {
        if let Some(quote_idx) = s.rfind('\'') {
            if quote_idx > 0 {
                let after_quote = &s[quote_idx + 1..];
                if after_quote.starts_with("::") {
                    return s[..=quote_idx].to_string();
                }
            }
        }
    }
    s.to_string()
}

/// Generate pure SQL DDL string from PostgreSQL system catalog
pub async fn generate_schema_ddl(pool: &PgPool) -> Result<String, crate::Error> {
    let enums = fetch_enums(pool).await?;
    let columns = fetch_columns(pool).await?;
    let constraints = fetch_constraints(pool).await?;
    let indexes = fetch_indexes(pool).await?;

    let mut ddl = String::new();

    // Enums
    for e in enums {
        let labels = e
            .labels
            .iter()
            .map(|l| format!("'{}'", l.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        ddl.push_str(&format!(
            "DO $$ BEGIN\n{INDENT}CREATE TYPE {} AS ENUM ({});\nEXCEPTION\n{INDENT}WHEN duplicate_object THEN null;\nEND $$;\n\n",
            e.typname.to_uppercase(), labels
        ));
    }

    // Group columns by table
    let mut table_cols: BTreeMap<String, Vec<ColumnDef>> = BTreeMap::new();
    for col in columns {
        table_cols.entry(col.table_name.clone()).or_default().push(col);
    }

    // Group constraints by table (by reference to avoid cloning)
    let mut table_constraints: HashMap<&str, Vec<&ConstraintDef>> = HashMap::new();
    for con in &constraints {
        table_constraints.entry(con.table_name.as_str()).or_default().push(con);
    }

    // Sort constraints canonically in Rust: PK -> Unique -> Check -> FK, then by name
    for list in table_constraints.values_mut() {
        list.sort_by_key(|c| (c.constraint_type, &c.constraint_name));
    }

    // Topological table ordering
    let table_names: Vec<String> = table_cols.keys().cloned().collect();
    let sorted_tables = sort_tables_topologically(&table_names, &constraints);

    let mut all_comment_stmts: Vec<String> = Vec::new();

    // Tables
    for tbl_name in &sorted_tables {
        let cols = match table_cols.get(tbl_name) {
            Some(c) => c,
            None => continue,
        };

        let empty_cons: Vec<&ConstraintDef> = Vec::new();
        let cons = table_constraints.get(tbl_name.as_str()).unwrap_or(&empty_cons);

        // Extract primary key column names in order
        let mut pk_cols: Vec<String> = Vec::new();
        for con in cons {
            if con.constraint_type == ConstraintType::PrimaryKey {
                let def = con.definition.trim();
                if def.starts_with("PRIMARY KEY (") && def.ends_with(')') {
                    let inner = &def["PRIMARY KEY (".len()..def.len() - 1];
                    pk_cols = inner
                        .split(',')
                        .map(|s| {
                            let trimmed = s.trim();
                            trimmed
                                .strip_prefix('"')
                                .and_then(|t| t.strip_suffix('"'))
                                .unwrap_or(trimmed)
                                .to_string()
                        })
                        .collect();
                }
                break;
            }
        }

        let is_single_pk = pk_cols.len() == 1;
        let single_pk_col = if is_single_pk { pk_cols.first().cloned() } else { None };

        // Order columns: PK column(s) hoisted to top, then remaining columns preserving original order
        let mut ordered_cols: Vec<&ColumnDef> = Vec::new();
        for pk_name in &pk_cols {
            if let Some(col) = cols.iter().find(|c| &c.column_name == pk_name) {
                ordered_cols.push(col);
            }
        }
        for col in cols {
            if !pk_cols.iter().any(|pk| pk == &col.column_name) {
                ordered_cols.push(col);
            }
        }

        let mut lines: Vec<String> = Vec::new();

        // Columns
        for col in ordered_cols {
            let mut col_comment_prefix = String::new();
            if let Some(ref desc) = col.description {
                let trimmed = desc.trim();
                if !trimmed.is_empty() {
                    for line in trimmed.lines() {
                        col_comment_prefix.push_str(&format!("{INDENT}-- {line}\n"));
                    }
                    let escaped = trimmed.replace('\'', "''");
                    all_comment_stmts.push(format!(
                        "COMMENT ON COLUMN {}.{} IS '{}';",
                        tbl_name, col.column_name, escaped
                    ));
                }
            }

            let col_type = normalize_column_type(&col.column_type);
            let is_single_pk_col = single_pk_col.as_deref() == Some(&col.column_name);

            let mut line = format!("{col_comment_prefix}{INDENT}{} {}", col.column_name, col_type);
            if is_single_pk_col {
                line.push_str(" PRIMARY KEY");
            } else if col.not_null {
                line.push_str(" NOT NULL");
            }

            if let Some(ref def) = col.default_value {
                let norm_def = normalize_default_value(def);
                line.push_str(&format!(" DEFAULT {}", norm_def));
            }
            lines.push(line);
        }

        // Constraints
        for con in cons {
            // Omit table-level PRIMARY KEY if it was already attached to the single PK column
            if con.constraint_type == ConstraintType::PrimaryKey && single_pk_col.is_some() {
                continue;
            }

            let line = match con.constraint_type {
                ConstraintType::PrimaryKey | ConstraintType::ForeignKey => {
                    format!("{INDENT}{}", con.definition)
                }
                _ => {
                    format!("{INDENT}CONSTRAINT {} {}", con.constraint_name, con.definition)
                }
            };
            lines.push(line);
        }

        ddl.push_str(&format!("CREATE TABLE IF NOT EXISTS {} (\n", tbl_name));
        ddl.push_str(&lines.join(",\n"));
        ddl.push_str("\n);\n\n");
    }

    // Indexes
    for idx in &indexes {
        let mut def = idx.index_def.trim().to_string();
        // Normalize "ON public.table" to "ON table"
        def = def.replacen(" ON public.", " ON ", 1);
        // Normalize "CREATE INDEX" to "CREATE INDEX IF NOT EXISTS"
        if def.starts_with("CREATE UNIQUE INDEX ") {
            def = def.replacen("CREATE UNIQUE INDEX ", "CREATE UNIQUE INDEX IF NOT EXISTS ", 1);
        } else if def.starts_with("CREATE INDEX ") {
            def = def.replacen("CREATE INDEX ", "CREATE INDEX IF NOT EXISTS ", 1);
        }
        if !def.ends_with(';') {
            def.push(';');
        }
        ddl.push_str(&format!("{}\n", def));
    }
    if !indexes.is_empty() {
        ddl.push('\n');
    }

    // All column comments at the very bottom of the file
    if !all_comment_stmts.is_empty() {
        for stmt in all_comment_stmts {
            ddl.push_str(&stmt);
            ddl.push('\n');
        }
    }

    Ok(ddl)
}

/// Main entrypoint to dump schema to target directory or file
pub async fn dump_schema(pool: &PgPool, output_dir: &str) -> Result<PathBuf, crate::Error> {
    let out_path = PathBuf::from(output_dir);
    let target = if out_path.extension().is_some_and(|ext| ext == "sql") {
        out_path
    } else {
        out_path.join("01_schema.sql")
    };

    println!("==> Introspecting database schema directly via PostgreSQL catalog...");
    let sql = generate_schema_ddl(pool).await?;

    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    tokio::fs::write(&target, sql).await?;
    println!("==> Schema successfully written to {}", target.display());
    Ok(target)
}
