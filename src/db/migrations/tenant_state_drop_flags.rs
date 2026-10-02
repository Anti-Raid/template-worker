use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenant_state_drop_flags",
    description: "Drop flags from tenant state",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_state DROP COLUMN flags"

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
