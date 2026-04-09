use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "user_access_token_last_fetched",
    description: "Create access_token_last_fetched on users table",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE users ADD COLUMN access_token_last_fetched timestamptz not null default now()"
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
