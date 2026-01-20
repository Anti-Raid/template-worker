mod khronosvalue_v2;
mod kv_generic;
mod tenantstate;
mod cleanup_v8;
mod attached_global_kv;

use futures::future::BoxFuture;
use log::info;

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub id: &'static str,
    pub description: &'static str,
    pub up: fn(sqlx::Pool<sqlx::Postgres>) -> BoxFuture<'static, Result<(), crate::Error>>,
}

pub const MIGRATIONS: [Migration; 5] = [
    // This relies on kv_generic not being applied yet so order matters
    khronosvalue_v2::MIGRATION,
    kv_generic::MIGRATION,
    tenantstate::MIGRATION,
    cleanup_v8::MIGRATION,
    attached_global_kv::MIGRATION,
];

pub async fn apply_migrations(pool: sqlx::PgPool) -> Result<(), crate::Error> {
    // Create table storing applied migrations if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations_applied (
            id TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(&pool)
    .await?;

    for migration in MIGRATIONS {
        // Check if migration has already been applied
        let already_applied: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM _migrations_applied WHERE id = $1",
        )
        .bind(migration.id)
        .fetch_one(&pool)
        .await
        .expect("Failed to check applied migrations");

        if already_applied.0 > 0 {
            info!("Migration already applied: {}", migration.id);
            continue;
        }

        info!("Applying migration: {} - {}", migration.id, migration.description);

        (migration.up)(pool.clone())
            .await
            .expect("Failed to apply migration");
        info!("Migration applied successfully");
    }

    info!("All migrations applied successfully");

    Ok(())
}