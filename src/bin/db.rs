use sqlx::postgres::PgPoolOptions;
use tw::migrations::apply_migrations;

const SQL_EXTENSIONS: &str = include_str!("../../sql/00_extensions.sql");
const SQL_SCHEMA: &str = include_str!("../../sql/01_schema.sql");

/// If `-d` (override_url) is set, use that, otherwise try DATABASE_URL and lastly try loading config
/// 
/// If all fails, just use postgres:///antiraid and hope user knows what theyre doing
fn resolve_db_url(override_url: Option<String>) -> String {
    if let Some(url) = override_url {
        return url;
    }
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    tw::config::Config::load()
        .map(|c| c.postgres_url)
        .unwrap_or_else(|_| "postgres:///antiraid".to_string())
}

fn print_help(db_url: &str) {
    println!("AntiRaid Database CLI");
    println!();
    println!("Usage:");
    println!("  cargo run --bin db -- <command> [options]");
    println!();
    println!("Commands:");
    println!("  schema         Apply extensions and baseline schema (00_extensions.sql, 01_schema.sql)");
    println!("  reset          Drop public schema, recreate schema, and apply baseline schema");
    println!("  migrate        Run incremental migrations from tw::migrations");
    println!("  psql [args...] Connect to the database with psql (interactive or run command)");
    println!("  dump <dir>     Dump database schema into <dir>/01_schema.sql (pure Rust)");
    println!("  url            Print resolved database connection URL");
    println!();
    println!("Options:");
    println!("  -d, --db-url <URL>  Override PostgreSQL connection URL");
    println!("  -h, --help          Show this help message");
    println!();
    println!("Resolved Database URL: {db_url}");
}

fn run_psql(db_url: &str, extra_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("==> Connecting with psql to {db_url}...");
    let mut cmd = std::process::Command::new("psql");
    cmd.arg(db_url);
    cmd.args(extra_args);

    let status = cmd.status().map_err(|e| {
        format!("Failed to execute 'psql': {e}. Ensure psql is installed and available in PATH.")
    })?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

async fn apply_schema(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("==> Applying extensions (sql/00_extensions.sql)...");
    sqlx::raw_sql(SQL_EXTENSIONS).execute(pool).await?;
    println!("==> Applying schema (sql/01_schema.sql)...");
    sqlx::raw_sql(SQL_SCHEMA).execute(pool).await?;
    println!("==> Schema applied successfully.");
    Ok(())
}

async fn apply_reset(pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    println!("==> WARNING: Resetting database (dropping public schema)...");
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO PUBLIC;")
        .execute(pool)
        .await?;
    println!("==> Public schema recreated. Applying schema...");
    apply_schema(pool).await?;
    println!("==> Database reset successfully.");
    Ok(())
}

fn main() {
    let mut db_url_override: Option<String> = None;
    let mut subcommand: Option<String> = None;
    let mut sub_args: Vec<String> = Vec::new();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-d" | "--db-url" => {
                i += 1;
                if i < args.len() {
                    db_url_override = Some(args[i].clone());
                } else {
                    eprintln!("Error: -d / --db-url requires a value");
                    std::process::exit(1);
                }
            }
            "-h" | "--help" | "help" => {
                let db_url = resolve_db_url(db_url_override);
                print_help(&db_url);
                std::process::exit(0);
            }
            _ if subcommand.is_none() => {
                subcommand = Some(arg.clone());
            }
            _ => {
                sub_args.push(arg.clone());
            }
        }
        i += 1;
    }

    let db_url = resolve_db_url(db_url_override);

    let subcmd = match subcommand {
        Some(cmd) => cmd,
        None => {
            print_help(&db_url);
            std::process::exit(0);
        }
    };

    match subcmd.as_str() {
        "url" => {
            println!("{db_url}");
        }
        "psql" => {
            if let Err(e) = run_psql(&db_url, &sub_args) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        "schema" | "reset" | "migrate" | "dump" => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                println!("==> Connecting to database: {db_url}...");
                let pool = match PgPoolOptions::new()
                    .max_connections(5)
                    .connect(&db_url)
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error: Failed to connect to PostgreSQL at '{db_url}': {e}");
                        std::process::exit(1);
                    }
                };

                let res = match subcmd.as_str() {
                    "schema" => apply_schema(&pool).await,
                    "reset" => apply_reset(&pool).await,
                    "migrate" => {
                        println!("==> Running incremental migrations...");
                        apply_migrations(pool).await.map_err(|e| e.to_string().into())
                    }
                    "dump" => {
                        let out_dir = match sub_args.first() {
                            Some(dir) => dir.as_str(),
                            None => {
                                eprintln!("Error: 'dump' requires an output directory argument (e.g. 'cargo run --bin db -- dump sql')");
                                std::process::exit(1);
                            }
                        };
                        tw::db::schema_dump::dump_schema(&pool, out_dir)
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string().into())
                    }
                    _ => unreachable!(),
                };

                if let Err(e) = res {
                    eprintln!("Error during operation: {e}");
                    std::process::exit(1);
                }
            });
        }
        unknown => {
            eprintln!("Unknown command: '{unknown}'\n");
            print_help(&db_url);
            std::process::exit(1);
        }
    }
}
