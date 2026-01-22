use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenantstate",
    description: "Create tenant_states table to store per-tenant state replacing tenant_state (WIP impl of this feature)",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "DROP TABLE IF EXISTS tenant_state; -- Drop any WIP tenant state table",
                r#"
                CREATE TABLE tenant_state (
                    owner_id TEXT NOT NULL,
                    owner_type TEXT NOT NULL,
                    events TEXT[] NOT NULL DEFAULT '{}'::text[],
                    banned BOOLEAN NOT NULL DEFAULT FALSE,
                    data JSONB NOT NULL DEFAULT '{}'::jsonb,
                    PRIMARY KEY (owner_id, owner_type)
                );
                "#
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
