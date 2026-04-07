use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "tenantstate_add_eventrefs",
    description: "Add tenant_state_",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;
            let stmts = [
                "CREATE TABLE tenant_state_events (
                    owner_id TEXT NOT NULL, owner_type TEXT NOT NULL, 
                    event TEXT NOT NULL, 
                    system TEXT NOT NULL,
                    PRIMARY KEY (owner_id, owner_type, event, system),
                    FOREIGN KEY (owner_id, owner_type) REFERENCES tenant_state(owner_id, owner_type) ON DELETE CASCADE
                )",
                "ALTER TABLE tenant_state DROP COLUMN events",
                "CREATE INDEX idx_tenant_events ON tenant_state_events(owner_id, owner_type, event);"
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
