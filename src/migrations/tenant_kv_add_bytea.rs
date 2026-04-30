use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenant_kv_add_bytea",
    description: "Add bytea blob column to tenant_kv",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_state ADD COLUMN blob BYTEA",
                "ALTER TABLE tenant_state 
                ADD CONSTRAINT enforce_max_blob_size 
                CHECK (octet_length(blob) <= 524288)"
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
