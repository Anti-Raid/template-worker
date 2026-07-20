--!nocheck
local discord = require("@antiraid-core/plugins/discord")
local discordRest = require("@discord-types/restTypes")
local discordApi = require("@discord-types/apiTypes")

--- The fully typed, sandboxed Discord API Client for template execution
export type DiscordClient = {
    -- Internal State
    _ctx: any,

    -- ==========================================
    -- AntiRaid Helpers
    -- ==========================================
    
    --- Checks an audit log reason for validity, errors out if invalid
    antiraid_check_reason: (self: DiscordClient, reason: string) -> (),
    
    --- Checks if a specified user with an ID of `data.user_id` has the specified permissions in the server
    antiraid_check_permissions: (self: DiscordClient, data: discord.AntiRaidCheckPermissionsOptions) -> discord.AntiRaidCheckPermissionsResponse,

    --- Checks if a specified user has the specified permissions in the server and is above the target user in terms of hierarchy
    antiraid_check_permissions_and_hierarchy: (self: DiscordClient, data: discord.AntiRaidCheckPermissionsAndHierarchyOptions) -> discord.AntiRaidCheckPermissionsResponse,

    --- Checks if a specified user has the specified permissions in the channel `data.channel_id`
    antiraid_check_channel_permissions: (self: DiscordClient, data: discord.AntiRaidCheckChannelPermissionsOptions) -> discord.AntiRaidCheckChannelPermissionsResponse,

    --- Fetches the member and resolved permissions of a user in the guild
    antiraid_get_fused_member: (self: DiscordClient, ids: {discordApi.Snowflake}) -> discord.LazyAntiraidFusedMember,

    -- ==========================================
    -- Audit Logs
    -- ==========================================
    
    --- Gets the audit logs
    get_audit_logs: (self: DiscordClient, data: discord.GetAuditLogOptions) -> discord.LazyAuditLogObject,

    -- ==========================================
    -- Auto Moderation
    -- ==========================================
    
    --- Lists the auto moderation rules available
    list_auto_moderation_rules: (self: DiscordClient) -> discord.LazyAutomoderationRuleObjectList,

    --- Gets an auto moderation rule by ID
    get_auto_moderation_rule: (self: DiscordClient, data: discord.GetAutoModerationRuleOptions) -> discord.LazyAutomoderationRuleObject,

    --- Creates an auto moderation rule
    create_auto_moderation_rule: (self: DiscordClient, data: discord.CreateAutoModerationRuleOptions) -> discord.LazyAutomoderationRuleObject,

    --- Edits an auto moderation rule
    edit_auto_moderation_rule: (self: DiscordClient, data: discord.EditAutoModerationRuleOptions) -> discord.LazyAutomoderationRuleObject,

    --- Deletes an auto moderation rule
    delete_auto_moderation_rule: (self: DiscordClient, data: discord.DeleteAutoModerationRuleOptions) -> (),
    
    -- ==========================================
    -- Channels
    -- ==========================================
    
    --- Gets a channel
    get_channel: (self: DiscordClient, channel_id: string) -> discord.LazyChannelObject, 

    --- Edits a channel
    edit_channel: (self: DiscordClient, data: discord.EditChannelOptions) -> discord.LazyChannelObject,

    --- Deletes a channel
    delete_channel: (self: DiscordClient, data: discord.DeleteChannelOptions) -> discord.LazyChannelObject,

    --- Edits channel permissions for a target
    edit_channel_permissions: (self: DiscordClient, data: discord.EditChannelPermissionsOptions) -> (),

    --- Gets all invites a channel has
    get_channel_invites: (self: DiscordClient) -> discord.LazyInviteObjectList,

    --- Creates a channel invite
    create_channel_invite: (self: DiscordClient, data: discord.CreateChannelInviteOptions) -> discord.LazyInviteObject,

    --- Deletes a channel permission
    delete_channel_permission: (self: DiscordClient, data: discord.DeleteChannelPermissionOptions) -> (),

    -- ==========================================
    -- Guild Base
    -- ==========================================
    
    --- Gets the guild
    get_guild: (self: DiscordClient) -> discord.LazyGuildObject,

    --- Gets the guilds preview
    get_guild_preview: (self: DiscordClient) -> discord.LazyGuildPreviewObject,

    --- Edits the guild
    modify_guild: (self: DiscordClient, data: discord.ModifyGuildOptions) -> discord.LazyGuildObject,

    --- Gets the guild channels
    get_guild_channels: (self: DiscordClient) -> discord.LazyChannelsObject,

    --- Creates a guild channel
    create_guild_channel: (self: DiscordClient, data: discord.CreateChannelOptions) -> discord.LazyChannelObject,

    --- Modify guild channel positions. Only channels to be modified are required to be passed in `data`.
    modify_guild_channel_positions: (self: DiscordClient, data: {discordRest.ModifyGuildChannelPositionsRequest}) -> (),

    --- List active guild threads
    list_active_guild_threads: (self: DiscordClient) -> discord.LazyActiveGuildThreadsResponse,

    -- ==========================================
    -- Guild Members & Bans
    -- ==========================================

    --- Gets a guild member by ID
    get_guild_member: (self: DiscordClient, user_id: string) -> discord.LazyGuildMemberObject,

    --- List all guild members
    list_guild_members: (self: DiscordClient, data: discord.GetGuildMembersOptions) -> discord.LazyGuildMembersObject,

    --- Search guild members
    search_guild_members: (self: DiscordClient, data: discord.SearchGuildMembersOptions) -> discord.LazyGuildMembersObject,

    --- Modify guild member (this includes timing out a member using `communication_disabled_until`)
    modify_guild_member: (self: DiscordClient, data: discord.ModifyGuildMemberOptions) -> discord.LazyGuildMemberObject,

    --- Adds a role to a member
    add_guild_member_role: (self: DiscordClient, data: discord.AddGuildMemberRoleOptions) -> (),

    --- Removes a role from a member
    remove_guild_member_role: (self: DiscordClient, data: discord.RemoveGuildMemberRoleOptions) -> (),

    --- Removes a member from a guild
    remove_guild_member: (self: DiscordClient, data: discord.RemoveGuildMemberOptions) -> (),

    --- Gets guild bans
    get_guild_bans: (self: DiscordClient, data: discord.GetGuildBansOptions) -> discord.LazyBanObjectList,

    --- Gets a guild ban for a user or nil if it does not exist
    get_guild_ban: (self: DiscordClient, user_id: discordApi.Snowflake) -> discord.LazyBanOptionalObject,

    --- Creates a guild ban
    create_guild_ban: (self: DiscordClient, data: discord.CreateGuildBanOptions) -> (),

    --- Removes a guild ban
    remove_guild_ban: (self: DiscordClient, data: discord.RemoveGuildBanOptions) -> (),

    -- ==========================================
    -- Guild Roles & Commands
    -- ==========================================

    --- Returns the guild roles of a guild
    get_guild_roles: (self: DiscordClient) -> discord.LazyRolesObject,

    --- Returns a guild role by ID
    get_guild_role: (self: DiscordClient, role_id: discordApi.Snowflake) -> discord.LazyRoleObject,

    --- Creates a guild role
    create_guild_role: (self: DiscordClient, data: discord.CreateGuildRoleOptions) -> discord.LazyRoleObject,

    --- Modify guild role positions
    modify_guild_role_positions: (self: DiscordClient, data: discord.ModifyRolePositionOptions) -> discord.LazyRolesObject,

    --- Modifies a guild role
    modify_guild_role: (self: DiscordClient, data: discord.EditGuildRoleOptions) -> discord.LazyRoleObject,

    --- Deletes a guild role
    delete_guild_role: (self: DiscordClient, data: discord.DeleteGuildRoleOptions) -> (),

    --- Gets all guild commands currently registered
    get_guild_commands: (self: DiscordClient) -> discord.LazyApplicationCommandsObject,

    --- Gets a specific guild command by ID
    get_guild_command: (self: DiscordClient, command_id: string) -> discord.LazyApplicationCommandObject,

    --- Creates a guild command
    create_guild_command: (self: DiscordClient, data: discord.CreateCommandOptions) -> discord.LazyApplicationCommandObject,

    --- Creates multiple guild commands
    create_guild_commands: (self: DiscordClient, data: discord.CreateCommandsOptions) -> discord.LazyApplicationCommandsObject,

    -- ==========================================
    -- Invites
    -- ==========================================

    --- Gets an invite by code
    get_invite: (self: DiscordClient, data: discord.GetInviteOptions) -> discord.LazyInviteObject,

    --- Deletes an invite by code
    delete_invite: (self: DiscordClient, data: discord.DeleteInviteOptions) -> discord.LazyInviteObject,

    -- ==========================================
    -- Messages
    -- ==========================================

    --- Gets messages from a channel
    get_channel_messages: (self: DiscordClient, data: discord.GetMessagesOptions) -> discord.LazyMessagesObject,

    --- Gets a message
    get_channel_message: (self: DiscordClient, data: discord.GetMessageOptions) -> discord.LazyMessageObject,

    --- Creates a message
    create_message: (self: DiscordClient, data: discord.CreateMessageOptions) -> discord.LazyMessageObject,

    --- Edits a message
    edit_message: (self: DiscordClient, data: discord.EditMessageOptions) -> discord.LazyMessageObject,

    --- Deletes a message
    delete_message: (self: DiscordClient, data: discord.DeleteMessageOptions) -> (),

    --- Bulk deletes messages in a channel
    bulk_delete_messages: (self: DiscordClient, data: discord.BulkDeleteMessagesOptions) -> (),

    -- ==========================================
    -- Reactions
    -- ==========================================

    --- Creates a reaction to a message
    create_reaction: (self: DiscordClient, data: discord.CreateReactionOptions) -> (),

    --- Deletes the reaction AntiRaid has made on a message
    delete_own_reaction: (self: DiscordClient, data: discord.DeleteOwnReactionOptions) -> (),

    --- Deletes a reaction another user has made on a message
    delete_user_reaction: (self: DiscordClient, data: discord.DeleteUserReactionOptions) -> (),

    --- Deletes all reactions on a message
    delete_all_reactions: (self: DiscordClient, data: discord.DeleteAllReactionsOptions) -> (),

    --- Deletes all reactions for a specific emoji on a message
    delete_all_reactions_for_emoji: (self: DiscordClient, data: discord.DeleteAllReactionsForEmojiOptions) -> (),

    -- ==========================================
    -- Interactions
    -- ==========================================
    
    --- Creates an interaction response
    create_interaction_response: (self: DiscordClient, data: discord.CreateInteractionResponseOptions) -> (),

    --- Gets the original interaction response
    get_original_interaction_response: (self: DiscordClient, interaction_token: string) -> discord.LazyMessageObject,

    --- Edits the original interaction response
    edit_original_interaction_response: (self: DiscordClient, data: discord.EditInteractionResponseOptions) -> discord.LazyMessageObject,

    --- Deletes the original interaction response
    delete_original_interaction_response: (self: DiscordClient, interaction_token: string) -> (),

    --- Gets a followup interaction response
    get_followup_message: (self: DiscordClient, data: discord.GetFollowupMessageOptions) -> discord.LazyMessageObject,

    --- Creates a followup interaction response
    create_followup_message: (self: DiscordClient, data: discord.CreateFollowupMessageOptions) -> discord.LazyMessageObject,

    --- Edits a followup interaction response
    edit_followup_message: (self: DiscordClient, data: discord.EditFollowupMessageOptions) -> discord.LazyMessageObject,

    --- Deletes a followup interaction response
    delete_followup_message: (self: DiscordClient, data: discord.DeleteFollowupMessageOptions) -> (),

    -- ==========================================
    -- Webhooks
    -- ==========================================

    --- Deletes a webhook
    delete_webhook: (self: DiscordClient, webhook_id: string) -> (),
}

-- Pre-allocate the shared metatable exactly once
local DiscordClientMethods = {}
DiscordClientMethods.__index = DiscordClientMethods

-- Internal Syscall Helper (Now a method)
function DiscordClientMethods:_call(req: any): any
    local res = self._ctx.syscall({
        op = "Discord",
        req = req
    })

    if res.op ~= "Discord" or res.res.op ~= req.op then
        error(`invalid op returned`, 3)
    end

    return res.res.res
end

-- ==========================================
-- AntiRaid Helpers
-- ==========================================
function DiscordClientMethods:antiraid_check_reason(reason)
    self:_call({ op = "AntiRaidCheckReason", data = { reason = reason } })
end
function DiscordClientMethods:antiraid_check_permissions(data)
    return self:_call({ op = "AntiRaidCheckPermissions", data = data })
end
function DiscordClientMethods:antiraid_check_permissions_and_hierarchy(data)
    return self:_call({ op = "AntiRaidCheckPermissionsAndHierarchy", data = data })
end
function DiscordClientMethods:antiraid_check_channel_permissions(data)
    return self:_call({ op = "AntiRaidCheckChannelPermissions", data = data })
end
function DiscordClientMethods:antiraid_get_fused_member(ids)
    return self:_call({ op = "AntiRaidGetFusedMember", data = { ids = ids } })
end

-- ==========================================
-- Audit Logs & Auto Moderation
-- ==========================================
function DiscordClientMethods:get_audit_logs(data)
    return self:_call({ op = "GetAuditLog", data = data })
end
function DiscordClientMethods:list_auto_moderation_rules()
    return self:_call({ op = "ListAutoModerationRules", data = {} }) -- Content required even if empty
end
function DiscordClientMethods:get_auto_moderation_rule(data)
    return self:_call({ op = "GetAutoModerationRule", data = data })
end
function DiscordClientMethods:create_auto_moderation_rule(data)
    return self:_call({ op = "CreateAutoModerationRule", data = data })
end
function DiscordClientMethods:edit_auto_moderation_rule(data)
    return self:_call({ op = "EditAutoModerationRule", data = data })
end
function DiscordClientMethods:delete_auto_moderation_rule(data)
    self:_call({ op = "DeleteAutoModerationRule", data = data })
end

-- ==========================================
-- Channels
-- ==========================================
function DiscordClientMethods:get_channel(channel_id)
    return self:_call({ op = "GetChannel", data = { channel_id = channel_id } })
end
function DiscordClientMethods:edit_channel(data)
    return self:_call({ op = "EditChannel", data = data })
end
function DiscordClientMethods:delete_channel(data)
    return self:_call({ op = "DeleteChannel", data = data })
end
function DiscordClientMethods:edit_channel_permissions(data)
    self:_call({ op = "EditChannelPermissions", data = data })
end
function DiscordClientMethods:get_channel_invites()
    return self:_call({ op = "GetChannelInvites", data = {} })
end
function DiscordClientMethods:create_channel_invite(data)
    return self:_call({ op = "CreateChannelInvite", data = data })
end
function DiscordClientMethods:delete_channel_permission(data)
    self:_call({ op = "DeleteChannelPermission", data = data })
end

-- ==========================================
-- Guild Base
-- ==========================================
function DiscordClientMethods:get_guild()
    return self:_call({ op = "GetGuild", data = {} })
end
function DiscordClientMethods:get_guild_preview()
    return self:_call({ op = "GetGuildPreview", data = {} })
end
function DiscordClientMethods:modify_guild(data)
    return self:_call({ op = "ModifyGuild", data = data })
end
function DiscordClientMethods:get_guild_channels()
    return self:_call({ op = "GetGuildChannels", data = {} })
end
function DiscordClientMethods:create_guild_channel(data)
    return self:_call({ op = "CreateGuildChannel", data = data })
end
function DiscordClientMethods:modify_guild_channel_positions(data)
    self:_call({ op = "ModifyGuildChannelPositions", data = data })
end
function DiscordClientMethods:list_active_guild_threads()
    return self:_call({ op = "ListActiveGuildThreads", data = {} })
end

-- ==========================================
-- Guild Members & Bans
-- ==========================================
function DiscordClientMethods:get_guild_member(user_id)
    return self:_call({ op = "GetGuildMember", data = { user_id = user_id } })
end
function DiscordClientMethods:list_guild_members(data)
    return self:_call({ op = "ListGuildMembers", data = data })
end
function DiscordClientMethods:search_guild_members(data)
    return self:_call({ op = "SearchGuildMembers", data = data })
end
function DiscordClientMethods:modify_guild_member(data)
    return self:_call({ op = "ModifyGuildMember", data = data })
end
function DiscordClientMethods:add_guild_member_role(data)
    self:_call({ op = "AddGuildMemberRole", data = data })
end
function DiscordClientMethods:remove_guild_member_role(data)
    self:_call({ op = "RemoveGuildMemberRole", data = data })
end
function DiscordClientMethods:remove_guild_member(data)
    self:_call({ op = "RemoveGuildMember", data = data })
end
function DiscordClientMethods:get_guild_bans(data)
    return self:_call({ op = "GetGuildBans", data = data })
end
function DiscordClientMethods:get_guild_ban(user_id)
    return self:_call({ op = "GetGuildBan", data = { user_id = user_id } })
end
function DiscordClientMethods:create_guild_ban(data)
    self:_call({ op = "CreateGuildBan", data = data })
end
function DiscordClientMethods:remove_guild_ban(data)
    self:_call({ op = "RemoveGuildBan", data = data })
end

-- ==========================================
-- Guild Roles & Commands
-- ==========================================
function DiscordClientMethods:get_guild_roles()
    return self:_call({ op = "GetGuildRoles", data = {} })
end
function DiscordClientMethods:get_guild_role(role_id)
    return self:_call({ op = "GetGuildRole", data = { role_id = role_id } })
end
function DiscordClientMethods:create_guild_role(data)
    return self:_call({ op = "CreateGuildRole", data = data })
end
function DiscordClientMethods:modify_guild_role_positions(data)
    return self:_call({ op = "ModifyGuildRolePositions", data = data })
end
function DiscordClientMethods:modify_guild_role(data)
    return self:_call({ op = "ModifyGuildRole", data = data })
end
function DiscordClientMethods:delete_guild_role(data)
    self:_call({ op = "DeleteGuildRole", data = data })
end
function DiscordClientMethods:get_guild_commands()
    return self:_call({ op = "GetGuildCommands", data = {} })
end
function DiscordClientMethods:get_guild_command(command_id)
    return self:_call({ op = "GetGuildCommand", data = { command_id = command_id } })
end
function DiscordClientMethods:create_guild_command(data)
    return self:_call({ op = "CreateGuildCommand", data = data })
end
function DiscordClientMethods:create_guild_commands(data)
    return self:_call({ op = "CreateGuildCommands", data = data })
end

-- ==========================================
-- Invites
-- ==========================================
function DiscordClientMethods:get_invite(data)
    return self:_call({ op = "GetInvite", data = data })
end
function DiscordClientMethods:delete_invite(data)
    return self:_call({ op = "DeleteInvite", data = data })
end

-- ==========================================
-- Messages
-- ==========================================
function DiscordClientMethods:get_channel_messages(data)
    return self:_call({ op = "GetChannelMessages", data = data })
end
function DiscordClientMethods:get_channel_message(data)
    return self:_call({ op = "GetChannelMessage", data = data })
end
function DiscordClientMethods:create_message(data)
    return self:_call({ op = "CreateMessage", data = data })
end
function DiscordClientMethods:edit_message(data)
    return self:_call({ op = "EditMessage", data = data })
end
function DiscordClientMethods:delete_message(data)
    self:_call({ op = "DeleteMessage", data = data })
end
function DiscordClientMethods:bulk_delete_messages(data)
    self:_call({ op = "BulkDeleteMessages", data = data })
end

-- ==========================================
-- Reactions
-- ==========================================
function DiscordClientMethods:create_reaction(data)
    self:_call({ op = "CreateReaction", data = data })
end
function DiscordClientMethods:delete_own_reaction(data)
    self:_call({ op = "DeleteOwnReaction", data = data })
end
function DiscordClientMethods:delete_user_reaction(data)
    self:_call({ op = "DeleteUserReaction", data = data })
end
function DiscordClientMethods:delete_all_reactions(data)
    self:_call({ op = "DeleteAllReactions", data = data })
end
function DiscordClientMethods:delete_all_reactions_for_emoji(data)
    self:_call({ op = "DeleteAllReactionsForEmoji", data = data })
end

-- ==========================================
-- Interactions
-- ==========================================
function DiscordClientMethods:create_interaction_response(data)
    self:_call({ op = "CreateInteractionResponse", data = data })
end
function DiscordClientMethods:get_original_interaction_response(interaction_token)
    return self:_call({ op = "GetOriginalInteractionResponse", data = { interaction_token = interaction_token } })
end
function DiscordClientMethods:edit_original_interaction_response(data)
    return self:_call({ op = "EditOriginalInteractionResponse", data = data })
end
function DiscordClientMethods:delete_original_interaction_response(interaction_token)
    self:_call({ op = "DeleteOriginalInteractionResponse", data = { interaction_token = interaction_token } })
end
function DiscordClientMethods:get_followup_message(data)
    return self:_call({ op = "GetFollowupMessage", data = data })
end
function DiscordClientMethods:create_followup_message(data)
    return self:_call({ op = "CreateFollowupMessage", data = data })
end
function DiscordClientMethods:edit_followup_message(data)
    return self:_call({ op = "EditFollowupMessage", data = data })
end
function DiscordClientMethods:delete_followup_message(data)
    self:_call({ op = "DeleteFollowupMessage", data = data })
end

-- ==========================================
-- Webhooks
-- ==========================================
function DiscordClientMethods:delete_webhook(webhook_id)
    self:_call({ op = "DeleteWebhook", data = { webhook_id = webhook_id } })
end

-- ==========================================
-- The Constructor
-- ==========================================

--- @noyield
---
--- Creates a new Discord API executor for the template
local function DiscordExecutor(ctx: Primitives.TemplateContext): DiscordClient
    -- Instead of recreating 75 closures, we instantiate a tiny table and slap a metatable on it.
    local client = {
        _ctx = ctx,
    }
    
    return (setmetatable(client, DiscordClientMethods) :: any) :: DiscordClient
end

return DiscordExecutor
