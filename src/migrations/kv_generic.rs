use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "kv_generic",
    description: "Migrate from guild_templates_kv to use the new tenant_kv table with generic tenant type support",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE guild_templates_kv RENAME TO tenant_kv",
                "ALTER TABLE tenant_kv DROP CONSTRAINT kv_unique_entry",
                "ALTER TABLE tenant_kv ADD COLUMN owner_type TEXT NOT NULL DEFAULT 'guild'",
                "ALTER TABLE tenant_kv RENAME COLUMN guild_id TO owner_id",
                "ALTER TABLE tenant_kv ALTER COLUMN owner_id DROP DEFAULT",
                "ALTER TABLE tenant_kv ADD CONSTRAINT kv_unique_entry UNIQUE (owner_id, owner_type, key, scopes)",
            ];

            for stmt in stmts.iter() {
                sqlx::query(stmt)
                    .execute(&mut *tx)
                    .await?;
            }

            tx.commit().await?;

            Ok(())
        })
    },
};
