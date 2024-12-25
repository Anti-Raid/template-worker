use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppData {
    pub data: Arc<silverpelt::data::Data>,
    pub serenity_context: serenity::all::Context,
}

impl AppData {
    pub fn new(data: Arc<silverpelt::data::Data>, ctx: &serenity::all::Context) -> Self {
        Self {
            data,
            serenity_context: ctx.clone(),
        }
    }
}

type Response<T> = Result<Json<T>, (StatusCode, String)>;

pub fn create(
    data: Arc<silverpelt::data::Data>,
    ctx: &serenity::all::Context,
) -> axum::routing::IntoMakeService<Router> {
    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
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
