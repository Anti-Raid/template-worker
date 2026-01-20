use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "attached_global_kv",
    description: "Create attached_global_kv table to link tenants to global KV entries w/ proof of purchase etc.",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;
            sqlx::query("
                CREATE TABLE global_kv_attachments (
                    owner_id TEXT NOT NULL,
                    owner_type TEXT NOT NULL,
                    key TEXT NOT NULL,
                    version INT NOT NULL,
                    scope TEXT NOT NULL,
                    attached_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    PRIMARY KEY (owner_id, owner_type, key, version, scope),
                    FOREIGN KEY (key, version, scope) REFERENCES global_kv(key, version, scope) ON DELETE CASCADE
                );
            ")
            .execute(&mut *tx)
            .await?;

            Ok(())
        })
    },
};
