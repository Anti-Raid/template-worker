use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "user_oauth_v2",
    description: "Create refresh token and last_set/expiry for access tokens on new user_oauths table",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;

            let stmts = [
                "ALTER TABLE users DROP COLUMN IF EXISTS access_token_last_fetched",
                "ALTER TABLE users DROP COLUMN IF EXISTS access_token",
                "CREATE TABLE user_oauths (
                    user_id TEXT PRIMARY KEY REFERENCES users(user_id),
                    access_token text not null,
                    refresh_token text not null,
                    access_token_last_set timestamptz not null,
                    access_token_expiry integer not null,
                    scope text not null
                )",

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
