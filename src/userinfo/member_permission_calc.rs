use kittycat::perms::Permission;
use serenity::all::{GuildId, RoleId, UserId};
use sqlx::Row;

/// ``create_roles_list_for_guild`` creates a list of roles for a guild including the everyone role as a string
///
/// This is needed by other functions to rederive permissions such as ``rederive_perms_impl`` and ``get_user_positions_from_db``
pub fn create_roles_list_for_guild(roles: &[RoleId], guild_id: GuildId) -> Vec<String> {
    let mut roles_str = Vec::new();

    for role in roles {
        roles_str.push(role.to_string());
    }

    roles_str.push(guild_id.everyone_role().to_string());

    roles_str
}

/// Returns the user positions of the member. This can be useful for caching or to reduce DB calls
///
/// ``roles_str`` is the list of roles as strings. This can be obtained by calling ``create_roles_list_for_guild``
pub async fn get_user_positions_from_db(
    pool: &sqlx::PgPool,
    guild_id: GuildId,
    roles_str: &[String],
) -> Result<Vec<kittycat::perms::PartialStaffPosition>, crate::Error> {
    // Rederive permissions for the new perms
    let role_perms = sqlx::query(
        "SELECT role_id, perms, index FROM guild_roles WHERE guild_id = $1 AND role_id = ANY($2)",
    )
    .bind(guild_id.to_string())
    .bind(roles_str)
    .fetch_all(pool)
    .await?;

    let mut user_positions = Vec::new();

    for role in role_perms {
        user_positions.push(kittycat::perms::PartialStaffPosition {
            id: role.try_get("role_id")?,
            perms: role
                .try_get::<Vec<String>, _>("perms")?
                .iter()
                .map(|x| Permission::from_string(x))
                .collect(),
            index: role.try_get("index")?,
        })
    }

    Ok(user_positions)
}

/// Rederive permissions rederives the permissions given a member id and a list of roles
///
/// Calling rederive_perms_and_update_db has some side-effects. Use rederive_perms_impl if you do not want to update the database
async fn rederive_perms(
    pool: &sqlx::PgPool,
    guild_id: GuildId,
    user_id: UserId,
    roles: &[RoleId],
) -> Result<kittycat::perms::StaffPermissions, crate::Error> {
    let perm_overrides = match sqlx::query(
        "SELECT perm_overrides FROM guild_members WHERE guild_id = $1 AND user_id = $2",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?
    {
        Some(row) => row
            .try_get::<Vec<String>, _>("perm_overrides")?
            .iter()
            .map(|x| Permission::from_string(x))
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    let roles_str = create_roles_list_for_guild(roles, guild_id);
    let user_positions = get_user_positions_from_db(pool, guild_id, &roles_str).await?;

    Ok(kittycat::perms::StaffPermissions {
        user_positions,
        perm_overrides,
    })
}

/// Returns the kittycat permissions of a user. This function also takes into account permission overrides etc.
pub async fn get_kittycat_perms(
    pool: &sqlx::PgPool,
    guild_id: GuildId,
    guild_owner_id: UserId,
    user_id: UserId,
    roles: &[RoleId],
) -> Result<kittycat::perms::StaffPermissions, crate::Error> {
    // For now, owners have full permission, this may change in the future (maybe??)
    if guild_owner_id == user_id {
        return Ok(kittycat::perms::StaffPermissions {
            user_positions: Vec::new(),
            perm_overrides: vec!["global.*".into()],
        });
    }

    // We hardcode root users for the main server to ensure root users have control over the bot even under extreme circumstances
    if guild_id == crate::CONFIG.servers.main
        && crate::CONFIG.discord_auth.root_users.contains(&user_id)
    {
        return Ok(kittycat::perms::StaffPermissions {
            user_positions: Vec::new(),
            perm_overrides: vec!["global.*".into()],
        });
    }

    rederive_perms(pool, guild_id, user_id, roles).await
}
