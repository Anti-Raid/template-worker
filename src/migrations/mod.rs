mod khronosvalue_v2;
mod kv_generic;
mod tenantstate;
mod cleanup_v8;
mod drop_tenant_kv_expires_at;
mod tenantstate_data_to_flags;
mod tenantstate_add_modflags;
mod tenantstate_add_eventrefs;
mod kv_scope_unnest;
mod user_oauth_v2;
mod tenant_state_drop_flags;
mod tenant_kv_add_bytea;

use std::borrow::Cow;

use futures::future::BoxFuture;
use khronos_ext::db::SimpleDbValueMapper;
use khronos_runtime::rt::mluau::prelude::*;
use khronos_runtime::rt::KhronosRuntime;
use log::info;
use rust_embed::Embed;

use crate::{master::mainthread::{RunInThreadFn, run_in_thread}, worker::builtins::TemplatingTypes};

pub const RUST_MIGRATIONS: [Migration; 11] = [
    // Do not change order of migrations without verifying dependencies
    khronosvalue_v2::MIGRATION,
    kv_generic::MIGRATION,
    tenantstate::MIGRATION,
    cleanup_v8::MIGRATION,
    drop_tenant_kv_expires_at::MIGRATION,
    tenantstate_data_to_flags::MIGRATION,
    tenantstate_add_modflags::MIGRATION,
    tenantstate_add_eventrefs::MIGRATION,
    user_oauth_v2::MIGRATION,
    tenant_state_drop_flags::MIGRATION,
    tenant_kv_add_bytea::MIGRATION,
];

pub const POST_LUAU_RUST_MIGRATIONS: [Migration; 1] = [
    // Migrations that need to be applied after all Luau migrations have finished
    kv_scope_unnest::MIGRATION,
];

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell"]
#[prefix = ""]
pub struct TwShell;

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell/_luaurcvfs"]
#[prefix = ""]
pub struct LuaurcVfs;

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell/migrations"]
#[prefix = ""]
pub struct MigrationsFolder;

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub id: &'static str,
    pub description: &'static str,
    pub up: fn(sqlx::Pool<sqlx::Postgres>) -> BoxFuture<'static, Result<(), crate::Error>>,
}

async fn apply_luau_migration(pool: sqlx::PgPool, migration_name: String) -> Result<(), crate::Error> {
    pub struct RunInThreadMigration;
    struct Data { pool: sqlx::PgPool, migration_name: String }
    impl RunInThreadFn<Data, Result<(), crate::Error>> for RunInThreadMigration {
        async fn run(rt: &KhronosRuntime, data: Data) -> Result<(), crate::Error> {
            // Patch in print()
            rt.with_lua(|lua| {
                lua.sandbox(false)?;
                let globals = lua.globals();
                let print_fn = lua.create_function(|_, args: LuaMultiValue| {
                    khronos_runtime::utils::pp::pretty_print(args);
                    Ok(())
                })?;
                globals.set("print", print_fn)?;
                lua.sandbox(true)?;
                Ok(())
            })
            .map_err(|e| format!("Failed to patch print function: {e}"))?;

            let tbl = rt
            .eval_script::<LuaTable>(
                &format!("./migrations/{}", data.migration_name.replace(".luau", "")),
            )
            .map_err(|e| format!("Failed to load Luau migration '{}': {e}", data.migration_name))?;
            let up = tbl.get::<LuaFunction>("up")
            .map_err(|e| format!("Failed to get 'up' function from migration: {e}"))?;

            let db = khronos_ext::db::Db::<SimpleDbValueMapper>::new(data.pool);

            let ud = rt.call_in_scheduler::<_, LuaMultiValue>(up, db).await
            .map_err(|e| format!("Failed to apply Luau migration: {e}"))?;
            println!("Luau migration returned: {:?}", ud);
            Ok(())
        }
    }

    run_in_thread::<RunInThreadMigration, _, _, _>(
    vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<LuaurcVfs>::new().into(),
            vfs::EmbeddedFS::<TwShell>::new().into(),
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ]),
        Data { pool, migration_name }
    )?;

    Ok(())
}

enum MigrationType {
    Rust(Migration),
    Luau(Cow<'static, str>)
}

/// Computes the list of migrations to apply, including both hardcoded Rust migrations and Luau migrations embedded in the binary
fn migrations() -> Result<Vec<MigrationType>, crate::Error> {
    let mut base_migrations = Vec::new();

    for migration in RUST_MIGRATIONS {
        base_migrations.push(MigrationType::Rust(migration));
    }

    // Add luau migrations from MigrationsFolder after all base rust once have finished
    for entry in MigrationsFolder::iter() {
        base_migrations.push(MigrationType::Luau(entry));
    }

    for migration in POST_LUAU_RUST_MIGRATIONS {
        base_migrations.push(MigrationType::Rust(migration));
    }

    Ok(base_migrations)
}

/// Note: this function may leak memory if there are Luau migrations, due to the way we convert the migration names to &'static str. 
/// 
/// This is generally acceptable since migrations are only applied once within the dedicated migration binary
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

    let migrations = migrations()?;

    /// Check if migration has already been applied
    async fn check_migration_applied(pool: &sqlx::PgPool, migration_id: &str) -> Result<bool, crate::Error> {
        let already_applied: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM _migrations_applied WHERE id = $1",
        )
        .bind(migration_id)
        .fetch_one(pool)
        .await?;

        Ok(already_applied.0 > 0)
    }

    /// Record that the migration has been applied
    async fn mark_migration_applied(pool: &sqlx::PgPool, migration_id: &str) -> Result<(), crate::Error> {
        sqlx::query(
            "INSERT INTO _migrations_applied (id) VALUES ($1)",
        )
        .bind(migration_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    for migration in migrations {
        match migration {
            MigrationType::Rust(migration) => {
                // Check if migration has already been applied
                if check_migration_applied(&pool, migration.id).await? {
                    info!("Migration already applied: {}", migration.id);
                    continue;
                }

                info!("Applying migration: {} - {}", migration.id, migration.description);

                (migration.up)(pool.clone())
                    .await
                    .expect("Failed to apply migration");

                info!("Migration applied successfully");

                // Record that the migration has been applied
                mark_migration_applied(&pool, migration.id).await?;
            }
            MigrationType::Luau(migration) => {
                // Check if migration has already been applied
                if check_migration_applied(&pool, &migration).await? {
                    info!("Migration already applied: {}", &migration);
                    continue;
                }

                info!("Applying luau migration: {migration}");

                apply_luau_migration(pool.clone(), migration.to_string()).await?;

                info!("Migration applied successfully");

                // Record that the migration has been applied
                mark_migration_applied(&pool, &migration).await?;
            }
        }
    }

    info!("All migrations applied successfully");

    Ok(())
}