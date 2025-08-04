# template-worker
Process that handles dispatching templates and runs expiry tasks as needed

## HTTP Documentation Notes

- All types used by the HTTP API must either be in ``src/api/types.rs`` or ``src/events``. They must also be annotated with `#[derive(utoipa::ToSchema)]` to ensure they are documented in the OpenAPI spec.