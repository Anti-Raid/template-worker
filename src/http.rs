use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use rust_rpc_server::AppData;
use std::sync::Arc;

type Response<T> = Result<Json<T>, (StatusCode, String)>;

pub fn create(
    data: Arc<silverpelt::data::Data>,
    ctx: &serenity::all::Context,
) -> axum::routing::IntoMakeService<Router> {
    let router = rust_rpc_server::create_blank_rpc_server()
        // Returns the list of modules [Modules]
        .route("/dispatch-event/:guild_id", post(dispatch_event));
    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}

/// Dispatches a new event
async fn dispatch_event(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<silverpelt::ar_event::AntiraidEvent>,
) -> Response<()> {
    // Clear cache if event is OnStartup
    if let silverpelt::ar_event::AntiraidEvent::OnStartup(_) = event {
        templating::cache::clear_cache(guild_id).await;
    }

    match crate::dispatch::event_listener(silverpelt::ar_event::EventHandlerContext {
        guild_id,
        data: data.clone(),
        event,
        serenity_context: serenity_context.clone(),
    })
    .await
    {
        Ok(_) => Ok(Json(())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
