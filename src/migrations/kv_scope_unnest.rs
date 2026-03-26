use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "kv_scope_unnest",
    description: "Move scopes to one single text from (text[] with GIN)",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE tenant_kv ADD COLUMN scopes_2 TEXT;",
                "ALTER TABLE tenant_kv DROP CONSTRAINT kv_unique_entry;"
            ];

            for stmt in stmts.iter() {
                sqlx::query(stmt)
                    .execute(&mut *tx)
                    .await?;
            }

            #[derive(sqlx::FromRow, Debug)]
            pub struct TenantKv {
                owner_id: String,
                owner_type: String,
                key: String,
                scopes: Vec<String>
            }

            let rows = sqlx::query_as::<_, TenantKv>("SELECT owner_id, owner_type, key, scopes FROM tenant_kv")
            .fetch_all(&mut *tx)
            .await?;

            for row in rows {
                if row.scopes.is_empty() {
                    panic!("No scopes found for {row:?}!");
                }
                let scope = row.scopes.iter().find(|elem| {
                    elem.chars().any(|c| !c.is_numeric())
                });

                let Some(scope) = scope else {
                    panic!("No non-numeric scopes found for {row:?}!");
                };

                sqlx::query("UPDATE tenant_kv SET scopes_2 = $1 WHERE owner_id = $2 and owner_type = $3 and key = $4")
                .bind(scope)
                .bind(row.owner_id)
                .bind(row.owner_type)
                .bind(row.key)
                .execute(&mut *tx)
                .await?;
            }

            let stmts = [
                "ALTER TABLE tenant_kv ALTER COLUMN scopes_2 SET NOT NULL;",
                "ALTER TABLE tenant_kv ADD CONSTRAINT kv_unique_entry UNIQUE (owner_id, owner_type, key, scopes_2);",
                "ALTER TABLE tenant_kv DROP COLUMN scopes",
                "ALTER TABLE tenant_kv RENAME COLUMN scopes_2 to scope",
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
