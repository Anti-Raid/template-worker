use std::rc::Rc;

use mlua::prelude::*;
use silverpelt::userinfo::{NoMember, UserInfo};

use crate::{lang_lua::state, TemplateContextRef};

use super::promise::lua_promise;
#[derive(Clone)]
/// An user info executor is used to fetch UserInfo's about users
pub struct UserInfoExecutor {
    pragma: crate::TemplatePragma,
    guild_id: serenity::all::GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
    ratelimits: Rc<state::Ratelimits>,
}

// @userdata LockdownExecutor
//
// Executes actions on discord
impl UserInfoExecutor {
    pub fn check_action(&self, action: String) -> LuaResult<()> {
        if !self
            .pragma
            .allowed_caps
            .contains(&format!("userinfo:{}", action))
        {
            return Err(LuaError::runtime(
                "User info action is not allowed in this template context",
            ));
        }

        self.ratelimits
            .userinfo
            .check(&action)
            .map_err(|e| LuaError::external(e.to_string()))?;

        Ok(())
    }
}

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/userinfo")
        .description(
            "This plugin allows for templates to interact with user's core information on AntiRaid (permissions etc)",
        )
}

impl LuaUserData for UserInfoExecutor {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_, this, (user,):(String,)| {
            let user: serenity::all::UserId = user
            .parse()
            .map_err(|e| LuaError::external(format!("Error while parsing user id: {}", e)))?;

            Ok(lua_promise!(this, user, |lua, this, user|, {
                this.check_action("get".to_string())?;

                let userinfo = UserInfo::get(this.guild_id, user, &this.pool, &this.serenity_context, &this.reqwest_client, None::<NoMember>).await
                .map_err(|e| LuaError::external(e.to_string()))?;

                let userinfo_val = lua.to_value(&userinfo)?;

                Ok(userinfo_val)
            }))
        });
    }
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "new",
        lua.create_function(|_, token: TemplateContextRef| {
            /*let user: serenity::all::UserId = user_id
            .parse()
            .map_err(|e| LuaError::external(format!("Error while parsing role id: {}", e)))?;*/

            let executor = UserInfoExecutor {
                pragma: token.template_data.pragma.clone(),
                guild_id: token.guild_state.guild_id,
                serenity_context: token.guild_state.serenity_context.clone(),
                ratelimits: token.guild_state.ratelimits.clone(),
                pool: token.guild_state.pool.clone(),
                reqwest_client: token.guild_state.reqwest_client.clone(),
            };

            Ok(executor)
        })?,
    )?;

    module.set_readonly(true);
    Ok(module)
}
