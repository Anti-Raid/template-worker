use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenantstate_add_modflags",
    description: "Add modflags integer field",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_state ADD COLUMN modflags INTEGER NOT NULL DEFAULT 0;", // Add the new flags column
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
