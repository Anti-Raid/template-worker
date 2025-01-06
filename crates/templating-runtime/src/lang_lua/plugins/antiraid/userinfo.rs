use std::rc::Rc;

use mlua::prelude::*;
use silverpelt::userinfo::{NoMember, UserInfo};

use crate::{lang_lua::state, TemplateContextRef};

use super::promise::lua_promise;
#[derive(Clone)]
/// An user info executor is used to fetch UserInfo's about users
pub struct UserInfoExecutor {
    allowed_caps: Vec<String>,
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
        if !self.allowed_caps.contains(&format!("userinfo:{}", action)) {
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
        .type_mut("UserInfo", "A user info object", |t| {
            t.example(std::sync::Arc::new(
                UserInfo {
                    discord_permissions: serenity::all::Permissions::all(),
                    kittycat_staff_permissions: kittycat::perms::StaffPermissions {
                        user_positions: vec![],
                        perm_overrides: vec!["global.*".into()],
                    },
                    kittycat_resolved_permissions: vec!["moderation.kick".into(), "moderation.ban".into()],
                    guild_owner_id: serenity::all::UserId::new(1234567890),
                    guild_roles: extract_map::ExtractMap::new(),
                    member_roles: vec![serenity::all::RoleId::new(1234567890)],
                }
            ))
            .field("discord_permissions", |f| {
                f.description("The discord permissions of the user")
                .typ("string")
            })
            .field("kittycat_staff_permissions", |f| {
                f.description("The staff permissions of the user")
                .typ("StaffPermissions")
            })
            .field("kittycat_resolved_permissions", |f| {
                f.description("The resolved permissions of the user")
                .typ("{Permission}")
            })
            .field("guild_owner_id", |f| {
                f.description("The guild owner id")
                .typ("string")
            })
            .field("guild_roles", |f| {
                f.description("The roles of the guild")
                .typ("{[string]: Serenity.Role}")
            })
            .field("member_roles", |f| {
                f.description("The roles of the member")
                .typ("{string}")
            })
        })
        .type_mut(
            "UserInfoExecutor",
            "UserInfoExecutor allows templates to access/use user infos not otherwise sent via events.",
            |mut t| {
                t
                .method_mut("get", |typ| {
                    typ
                    .description("Gets the user info of a user.")
                    .parameter("user", |p| {
                        p.typ("string").description("The user id to get the info of.")
                    })
                    .return_("UserInfo", |p| {
                        p.description("The user info of the user.")
                    })
                    .is_promise(true)
                })
            })
        .method_mut("new", |mut m| {
            m.parameter("token", |p| p.typ("TemplateContext").description("The token of the template to use."))
            .return_("executor", |r| r.typ("UserInfoExecutor").description("A userinfo executor."))
        })
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
                allowed_caps: token.template_data.allowed_caps.clone(),
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
