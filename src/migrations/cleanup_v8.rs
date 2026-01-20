use crate::migrations::Migration;

pub static MIGRATION: Migration = Migration {
    id: "cleanup_v8",
    description: "Clean up deprecated tables no longer in use",
    up: |pool| {
        Box::pin(async move {
            let mut tx = pool.begin().await?;
            sqlx::query("
                DROP TABLE IF EXISTS template_shop_listings; -- replaced by global_kv
                DROP TABLE IF EXISTS guild_templates; -- replaced by tenant_kv + luau (templatemanager)
                DROP TABLE IF EXISTS attached_templates; -- replaced by tenant_kv + luau (templatemanager)
                DROP TABLE IF EXISTS jobs; -- deprecated for a long time now
                DROP TABLE IF EXISTS ongoing_jobs; -- deprecated for a long time now
            ")
            .execute(&mut *tx)
            .await?;

            Ok(())
        })
    },
};
