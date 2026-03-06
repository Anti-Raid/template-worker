use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenantstate_data_to_flags",
    description: "Create tenant_states table to store per-tenant state replacing tenant_state (WIP impl of this feature)",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_state DROP COLUMN IF EXISTS data;", // Drop the old data column
                "ALTER TABLE tenant_state ADD COLUMN flags INTEGER NOT NULL DEFAULT 0;", // Add the new flags column
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
