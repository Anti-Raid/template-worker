# template-worker
Process that handles dispatching templates and runs expiry tasks as needed

## Public API Documentation Notes

- All types used by the HTTP API must be in ``src/api/types.rs``. They must also be annotated with `#[derive(utoipa::ToSchema)]` to ensure they are documented in the OpenAPI spec.

## Components

- ``api``: Contains the HTTP API server and related types.
- ``mesophyll``: Contains Mesophyll, which is the main (currently Websocket-based) communication layer between the master template-worker process and the worker processes.
- ``fauxpas``: Contains Fauxpas, which will provide the staff-related mock API.
- ``worker``: Contains the worker pool and worker implementations (process and the more primitive thread based workers that back them).
- ``geese``: Contains core storage and data input systems such as Sandwich (Gateway) and ObjectStore client code. Named after the Canadian Goose/Geese.

## Fauxpas Shell

template-worker includes a shell that can be used to execute Lua code. Note that while this does not yet provide access to the full worker environment, it is useful for moderation purposes and will/may be expanded in the future.