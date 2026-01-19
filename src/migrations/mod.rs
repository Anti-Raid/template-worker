mod khronosvalue_v2;
mod kv_generic;

use futures::future::BoxFuture;

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub id: &'static str,
    #[allow(dead_code)] // description is used as a comment
    pub description: &'static str,
    pub up: fn(sqlx::Pool<sqlx::Postgres>) -> BoxFuture<'static, Result<(), crate::Error>>,
}

pub const MIGRATIONS: [Migration; 2] = [
    // This relies on kv_generic not being applied yet
    khronosvalue_v2::MIGRATION,
    kv_generic::MIGRATION,
];