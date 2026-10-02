use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "drop_tenant_kv_expires_at",
    description: "Drop expires_at in tenant_kv (is now redundant",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_kv DROP COLUMN IF EXISTS expires_at;",
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
