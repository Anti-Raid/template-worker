use std::collections::{HashMap, HashSet};

use khronos_runtime::rt::mlua::prelude::*;

use crate::worker::workervmmanager::Id;

#[derive(Clone)]
/// A simple wrapper around the database pool that provides just the global key-value storage functionality
pub struct TenantStateDb {
    pool: sqlx::PgPool,
}

impl TenantStateDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Returns the tenant state(s) for all tenant in the database
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    pub async fn get_tenant_state(&self, id: Option<(i64, i64)>) -> Result<HashMap<Id, TenantState>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct TenantStatePartial {
            events: Vec<String>,
            flags: i32,
            modflags: i32,
            owner_id: String,
            owner_type: String,
        }

        let partials: Vec<TenantStatePartial> = match id {
            Some((id, num_workers)) => sqlx::query_as("SELECT owner_id, owner_type, events, flags, modflags FROM tenant_state WHERE ((owner_id::bigint >> 22) % $1 = $2)")
            .bind(num_workers)
            .bind(id)
            .fetch_all(&self.pool)
            .await?,
            None => sqlx::query_as("SELECT owner_id, owner_type, events, flags, modflags FROM tenant_state")
            .fetch_all(&self.pool)
            .await?
        };

        let mut states = HashMap::new();  
        for partial in partials {
            let Some(id) = Id::from_parts(&partial.owner_type, &partial.owner_id) else {
                continue;
            };
            let state = TenantState {
                events: HashSet::from_iter(partial.events),
                flags: partial.flags,
                modflags: ModFlags::from_bits_truncate(partial.modflags.try_into().unwrap_or(0))
            };

            states.insert(id, state);
        }

        Ok(states)
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TenantState {
    pub events: HashSet<String>,
    pub flags: i32,
    pub modflags: ModFlags,
}

impl IntoLua for TenantState {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;

        table.set("events", self.events)?;
        table.set("flags", self.flags)?;
        Ok(LuaValue::Table(table))
    }
}