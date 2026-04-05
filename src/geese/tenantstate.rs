use std::collections::{HashMap, HashSet};

use khronos_runtime::rt::mlua::prelude::*;

use crate::worker::workervmmanager::Id;

#[derive(Clone)]
/// A simple wrapper around the database pool that provides just the global key-value storage functionality
pub struct TenantStateDb {
    pool: sqlx::PgPool,
}

#[derive(sqlx::FromRow)]
/// Internally used for storing raw tenant state without refs
pub(super) struct TenantStatePartial {
    flags: i32,
    modflags: i32,
    owner_id: String,
    owner_type: String,
}

#[derive(sqlx::FromRow)]
/// Internally used for storing tenant state event refs
pub(super) struct TenantStateEventRefs {
    owner_id: String,
    owner_type: String,
    event: String, 
    systems: Vec<String>
}


impl TenantStateDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Returns the tenant state(s) for all tenant in the database
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    pub async fn get_tenant_state(&self, id: i64, num_workers: i64) -> Result<HashMap<Id, TenantState>, crate::Error> {
        let partials: Vec<TenantStatePartial> = sqlx::query_as("SELECT owner_id, owner_type, flags, modflags FROM tenant_state WHERE ((owner_id::bigint >> 22) % $1 = $2)")
            .bind(num_workers)
            .bind(id)
            .fetch_all(&self.pool)
            .await?;

        let partial_refs: Vec<TenantStateEventRefs> = sqlx::query_as("
            SELECT 
                tse.owner_id, 
                tse.owner_type, 
                tse.event, 
                array_agg(tse.system) as systems
            FROM tenant_state_events tse
            JOIN tenant_state ts 
                ON ts.owner_id = tse.owner_id AND ts.owner_type = tse.owner_type
            WHERE ((ts.owner_id::bigint >> 22) % $1 = $2)
            GROUP BY tse.owner_id, tse.owner_type, tse.event
        ")
            .bind(num_workers)
            .bind(id)
            .fetch_all(&self.pool)
            .await?;

        Ok(Self::into_tenant_state(partials, partial_refs))
    }

    pub(super) fn into_tenant_state(partials: Vec<TenantStatePartial>, partial_refs: Vec<TenantStateEventRefs>) -> HashMap<Id, TenantState> {
        let mut states = HashMap::new();  
        for partial in partials {
            let Some(id) = Id::from_parts(&partial.owner_type, &partial.owner_id) else {
                continue;
            };
            let state = TenantState {
                events: HashMap::new(),
                flags: partial.flags,
                modflags: ModFlags::from_bits_truncate(partial.modflags.try_into().unwrap_or(0))
            };

            states.insert(id, state);
        }

        for refs in partial_refs {
            let Some(id) = Id::from_parts(&refs.owner_type, &refs.owner_id) else {
                continue;
            };

            states.entry(id).or_insert(TenantState::default()).events.insert(refs.event, HashSet::from_iter(refs.systems));
        }

        states
    }

    pub(super) fn into_tenant_state_single(partial: TenantStatePartial, partial_refs: Vec<TenantStateEventRefs>) -> TenantState {
        let mut state =  TenantState {
            events: HashMap::new(),
            flags: partial.flags,
            modflags: ModFlags::from_bits_truncate(partial.modflags.try_into().unwrap_or(0))
        };

        for refs in partial_refs {
            state.events.insert(refs.event, HashSet::from_iter(refs.systems));
        }

        state
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
    pub struct ModFlags: u8 {
        /// Whether or not the tenant is banned. If true, the tenant will not be able to startup luau VMs and will instead recieve a direct banned message for any interaction
        /// created with the bot
        const BANNED = 1 << 0;
        /// Whether or not the tenant can modify guild commands
        const CAN_MANAGE_GUILD_COMMANDS = 1 << 1;
    }
}

impl Default for ModFlags {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TenantState {
    pub events: HashMap<String, HashSet<String>>,
    pub flags: i32,
    pub modflags: ModFlags,
}

// DEFAULT_EVENTS is handled by WorkerDispatch directly
pub static DEFAULT_EVENTS: [&str; 3] = [
    "INTERACTION_CREATE", "WebGetSettings", "WebExecuteSetting"
];

impl Default for TenantState {
    fn default() -> Self {
        Self {
            events: HashMap::new(),
            flags: 0,
            modflags: ModFlags::empty()
        }
    }
}

impl IntoLua for TenantState {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;

        table.set("events", self.events)?;
        table.set("flags", self.flags)?;
        Ok(LuaValue::Table(table))
    }
}