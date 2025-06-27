pub mod member_permission_calc;

use antiraid_types::userinfo::UserInfo;

pub struct NoMember {}

impl AsRef<serenity::all::Member> for NoMember {
    fn as_ref(&self) -> &serenity::all::Member {
        unimplemented!()
    }
}

#[allow(async_fn_in_trait)]
pub trait UserInfoOperations: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    /// A simple, generic implementation to get UserInfo object
    async fn get(
        guild_id: serenity::all::GuildId,
        user_id: serenity::all::UserId,
        pool: &sqlx::PgPool,
        serenity_context: &serenity::all::Context,
        reqwest: &reqwest::Client,
        member_opt: Option<impl AsRef<serenity::all::Member>>,
    ) -> Result<UserInfo, crate::Error>;
}

impl UserInfoOperations for UserInfo {
    /// A simple, generic implementation to get UserInfo object
    async fn get(
        guild_id: serenity::all::GuildId,
        user_id: serenity::all::UserId,
        pool: &sqlx::PgPool,
        serenity_context: &serenity::all::Context,
        reqwest: &reqwest::Client,
        // In some cases, we *do* have the member object, so we can pass it here
        member_opt: Option<impl AsRef<serenity::all::Member>>,
    ) -> Result<Self, crate::Error> {
        let cached_data = {
            if let Some(cached_guild) = guild_id.to_guild_cached(&serenity_context.cache) {
                if let Some(ref member) = member_opt {
                    let member = member.as_ref();

                    Some((
                        cached_guild.owner_id,
                        cached_guild.roles.clone(),
                        Some(member.roles.clone()),
                    ))
                } else if let Some(member) = cached_guild.members.get(&user_id) {
                    Some((
                        cached_guild.owner_id,
                        cached_guild.roles.clone(),
                        Some(member.roles.clone()),
                    ))
                } else {
                    Some((cached_guild.owner_id, cached_guild.roles.clone(), None))
                }
            } else {
                None
            }
        };

        if let Some((guild_owner, guild_roles, member_roles)) = cached_data {
            let member_roles = match member_roles {
                Some(member_roles) => member_roles,
                None => {
                    let member = crate::sandwich::member_in_guild(
                        &serenity_context.cache,
                        &serenity_context.http,
                        reqwest,
                        guild_id,
                        user_id,
                    )
                    .await?;

                    let Some(member) = member else {
                        return Err("Member could not fetched".into());
                    };

                    member.roles
                }
            };

            let kittycat_staff_permissions = member_permission_calc::get_kittycat_perms(
                pool,
                guild_id,
                guild_owner,
                user_id,
                &member_roles,
            )
            .await?;

            return Ok(Self {
                discord_permissions: botox::serenity_backports::user_permissions(
                    user_id,
                    &member_roles,
                    guild_id,
                    &guild_roles,
                    guild_owner,
                ),
                kittycat_resolved_permissions: kittycat_staff_permissions.resolve(),
                kittycat_staff_permissions,
                guild_owner_id: guild_owner,
                guild_roles,
                member_roles: member_roles.to_vec(),
            });
        }

        let guild = guild_id.to_partial_guild(&serenity_context).await?;

        // Either we have the member object, or we have to fetch it
        if let Some(member) = member_opt {
            let member = member.as_ref();

            let kittycat_staff_permissions = member_permission_calc::get_kittycat_perms(
                pool,
                guild_id,
                guild.owner_id,
                user_id,
                &member.roles,
            )
            .await?;

            return Ok(Self {
                discord_permissions: botox::serenity_backports::user_permissions(
                    member.user.id,
                    &member.roles,
                    guild.id,
                    &guild.roles,
                    guild.owner_id,
                ),
                kittycat_resolved_permissions: kittycat_staff_permissions.resolve(),
                kittycat_staff_permissions,
                guild_owner_id: guild.owner_id,
                guild_roles: guild.roles,
                member_roles: member.roles.to_vec(),
            });
        }

        let member = {
            let member = crate::sandwich::member_in_guild(
                &serenity_context.cache,
                &serenity_context.http,
                reqwest,
                guild_id,
                user_id,
            )
            .await?;

            let Some(member) = member else {
                return Err("Member could not fetched".into());
            };

            member
        };

        let kittycat_staff_permissions = member_permission_calc::get_kittycat_perms(
            pool,
            guild_id,
            guild.owner_id,
            user_id,
            &member.roles,
        )
        .await?;

        Ok(Self {
            discord_permissions: botox::serenity_backports::user_permissions(
                member.user.id,
                &member.roles,
                guild.id,
                &guild.roles,
                guild.owner_id,
            ),
            kittycat_resolved_permissions: kittycat_staff_permissions.resolve(),
            kittycat_staff_permissions,
            guild_owner_id: guild.owner_id,
            guild_roles: guild.roles,
            member_roles: member.roles.to_vec(),
        })
    }
}
