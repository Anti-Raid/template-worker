use std::{borrow::Cow, cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::{Arc, LazyLock}};
use chrono::{DateTime, Utc};

use crate::worker::workervmmanager::Id;

#[derive(Debug)]
pub struct KeyExpiry {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TenantState {
    pub events: HashSet<String>,
    pub banned: bool,
    pub flags: i32,
    pub startup_events: bool,
}

static DEFAULT_TENANT_STATE: LazyLock<TenantState> = LazyLock::new(|| TenantState {
    events: {
        let mut set = HashSet::new();
        set.insert("INTERACTION_CREATE".to_string());
        set.insert("KeyExpiry".to_string());
        set.insert("GetSettings".to_string());
        set.insert("ExecuteSetting".to_string());
        set
    },
    banned: false,
    flags: 0,
    startup_events: false,
});

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct CreateWorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub pool: sqlx::PgPool,
    pub current_user: Arc<serenity::all::CurrentUser>,
}

impl CreateWorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        reqwest_client: reqwest::Client,
        object_store: Arc<crate::objectstore::ObjectStore>,
        pool: sqlx::PgPool,
        current_user: Arc<serenity::all::CurrentUser>,
    ) -> Self {
        Self {
            serenity_http,
            reqwest_client,
            object_store,
            pool,
            current_user,
        }
    }
}

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub pool: sqlx::PgPool,
    pub current_user: Arc<serenity::all::CurrentUser>,
    tenant_state_cache: Rc<RefCell<HashMap<Id, TenantState>>>, // Maps tenant IDs to their states
    startup_events: Rc<RefCell<HashSet<Id>>>, // Tracks which tenants have had their startup events fired
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub async fn new(cws: CreateWorkerState) -> Result<Self, crate::Error> {
        let tenant_state_cache = Rc::new(RefCell::new(HashMap::new()));
        let startup_events = Rc::new(RefCell::new(HashSet::new()));
        let s = Self {
            serenity_http: cws.serenity_http,
            reqwest_client: cws.reqwest_client,
            object_store: cws.object_store,
            pool: cws.pool,
            current_user: cws.current_user,
            tenant_state_cache,
            startup_events,
        };

        // Initialize the tenant state cache with the current tenant states from the database
        //
        // The tenant state cache acts as a routing table
        let (t_states, startup_events) = s.get_tenant_state().await?;
        *s.tenant_state_cache.borrow_mut() = t_states;
        *s.startup_events.borrow_mut() = startup_events;

        Ok(s)
    }

    /// Returns the tenant state(s) for all guilds in the database as well as a set of guild IDs that have startup events enabled
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    async fn get_tenant_state(&self) -> Result<(HashMap<Id, TenantState>, HashSet<Id>), crate::Error> {
        #[derive(sqlx::FromRow)]
        struct TenantStatePartial {
            events: Vec<String>,
            banned: bool,
            flags: i32,
            startup_events: bool,
            owner_id: String,
            owner_type: String,
        }

        let partials: Vec<TenantStatePartial> =
            sqlx::query_as("SELECT owner_id, owner_type, events, banned, flags, startup_events FROM tenant_state")
            .fetch_all(&self.pool)
            .await?;

        let mut states = HashMap::new();  
        let mut startup_events = HashSet::new();  
        for partial in partials {
            let id = match partial.owner_type.as_str() {
                "guild" => Id::GuildId(partial.owner_id.parse()?),
                _ => continue, // Unknown type, skip
            };

            let state = TenantState {
                events: HashSet::from_iter(partial.events),
                banned: partial.banned,
                flags: partial.flags,
                startup_events: partial.startup_events,
            };

            // Track startup events
            if partial.startup_events {
                startup_events.insert(id.clone());
            }

            states.insert(id, state);
        }

        Ok((states, startup_events))
    }

    /// Returns the set of tenant IDs that have startup events enabled
    pub fn get_startup_event_tenants(&self) -> Result<std::cell::Ref<'_, HashSet<Id>>, crate::Error> {
        Ok(self.startup_events.try_borrow()?)
    }

    /// Gets the tenant state for a specific tenant
    pub fn get_cached_tenant_state_for<'a>(&'a self, id: Id) -> Result<Cow<'a, TenantState>, crate::Error> {
        let cache = self.tenant_state_cache.borrow();
        match cache.get(&id) {
            Some(state) => Ok(Cow::Owned(state.clone())),
            None => {
                // Return the default tenant state if not found in cache
                Ok(Cow::Borrowed(&*DEFAULT_TENANT_STATE))
            }
        }
    }

    /// Sets the tenant state for a specific tenant
    pub async fn set_tenant_state_for(&self, id: Id, state: TenantState) -> Result<(), crate::Error> {
        let events = state.events.iter().collect::<Vec<_>>();
        match id {
            Id::GuildId(guild_id) => {
                sqlx::query(
                    "INSERT INTO tenant_state (owner_id, owner_type, events, banned, flags, startup_events) VALUES ($1, 'guild', $2, $3, $4, $5) ON CONFLICT (owner_id, owner_type) DO UPDATE SET events = EXCLUDED.events, banned = EXCLUDED.banned, flags = EXCLUDED.flags, startup_events = EXCLUDED.startup_events",
                )
                .bind(guild_id.to_string())
                .bind(&events)
                .bind(state.banned)
                .bind(state.flags as i32)
                .bind(state.startup_events)
                .execute(&self.pool)
                .await?;
            }
        }

        // Update the cache
        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            cache.insert(id, state);
        }

        Ok(())
    }

    /// Gets all key expiries from the database
    pub async fn get_key_expiries(&self) -> Result<HashMap<Id, Vec<KeyExpiry>>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct KeyExpiryPartial {
            guild_id: String,
            id: String,
            key: String,
            scopes: Vec<String>,
            expires_at: chrono::DateTime<chrono::Utc>,
        }

        let partials: Vec<KeyExpiryPartial> =
            sqlx::query_as("SELECT guild_id, id, key, scopes, expires_at FROM guild_templates_kv WHERE expires_at IS NOT NULL ORDER BY expires_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let mut expiries: HashMap<Id, Vec<KeyExpiry>> = HashMap::new();

        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            let expiry = KeyExpiry {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
                expires_at: partial.expires_at,
            };

            let id = Id::GuildId(guild_id);
            if let Some(expiries_vec) = expiries.get_mut(&id) {
                expiries_vec.push(expiry);
            } else {
                expiries.insert(id, vec![expiry]);
            }
        }

        Ok(expiries)
    }

    /// Removes keys with the given ID
    pub async fn remove_key_expiry(&self, id: Id, kv_id: &str) -> Result<(), crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2")
                    .bind(guild_id.to_string())
                    .bind(kv_id)
                    .execute(&self.pool)
                    .await?;
            }
        }

        Ok(())
    }
}

