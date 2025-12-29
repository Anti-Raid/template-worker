mod khronosvalue_v2;

use futures::future::BoxFuture;

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub id: &'static str,
    #[allow(dead_code)] // description is used as a comment
    pub description: &'static str,
    pub up: fn(sqlx::Pool<sqlx::Postgres>) -> BoxFuture<'static, Result<(), crate::Error>>,
}

pub const MIGRATIONS: [Migration; 1] = [
    khronosvalue_v2::MIGRATION,
];