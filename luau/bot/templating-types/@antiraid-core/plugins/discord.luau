local discord = require "@discord-types/apiTypes"
local discordRest = require "@discord-types/restTypes"
local lazy = require "@antiraid-core/lazy"
local typesext = require "@antiraid/typesext"

--- Options for getting audit logs in Discord
export type GetAuditLogOptions = {
    --- The action type to filter by
    action_type: discord.AuditLogEventType?,
    --- The user ID to filter by
    user_id: discord.Snowflake?,
    --- The audit log entry ID to filter
    before: discord.Snowflake?,
    --- The number of entries to return
    limit: number?
}

--- Options for getting an auto moderation rule in Discord
export type GetAutoModerationRuleOptions = {
    --- The rule ID
    rule_id: discord.Snowflake
}

--- Options for creating an auto moderation rule in Discord
export type CreateAutoModerationRuleOptions = {
    --- The reason for creating the rule
    reason: string,
    --- The data to create the rule with
    data: discordRest.CreateAutoModerationRuleRequest
}

--- Options for editing an auto moderation rule in Discord
export type EditAutoModerationRuleOptions = {
    --- The rule ID
    rule_id: discord.Snowflake,
    --- The reason for editing the rule
    reason: string,
    --- The data to edit the rule with
    data: discordRest.ModifyAutoModerationRuleRequest
}

--- Options for deleting an auto moderation rule in Discord
export type DeleteAutoModerationRuleOptions = {
    --- The rule ID
    rule_id: discord.Snowflake,
    --- The reason for deleting the rule
    reason: string
}

--- Options for editing a channel in Discord
export type EditChannelOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The reason for the edit
    reason: string,
    --- The data to edit the channel with
    data: discordRest.ModifyChannelRequest
}

--- Options for deleting a channel in Discord
export type DeleteChannelOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The reason for the deletion
    reason: string
}

--- Options for editting channel permissions
export type EditChannelPermissionsOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The target ID to edit permissions of
    target_id: discord.Snowflake,
    --- The allow permissions
    allow: typesext.MultiOption<string>,
    --- The deny permissions
    deny: typesext.MultiOption<string>,
    --- The type of the target
    kind: discord.OverwriteObjectType,
    --- The reason for the edit
    reason: string,
}

--- Options for editing a channel in Discord
export type CreateChannelOptions = {
    --- The reason for the create
    reason: string,
    --- The data to edit the channel with
    data: discordRest.CreateGuildChannelRequest
}

--- Options for adding a role to a member
export type AddGuildMemberRoleOptions = {
    --- The member ID
    user_id: discord.Snowflake,
    --- The role ID
    role_id: discord.Snowflake,
    --- The reason for adding the role
    reason: string
}

--- Options for removing a role from a member
export type RemoveGuildMemberRoleOptions = {
    --- The member ID
    user_id: discord.Snowflake,
    --- The role ID
    role_id: discord.Snowflake,
    --- The reason for adding the role
    reason: string
}

--- Options for removing a member from a guild
export type RemoveGuildMemberOptions = {
    --- The member ID
    user_id: discord.Snowflake,
    --- The reason for removing the member
    reason: string
}

--- Options for getting guild bans
---
--- Note: If both `before` and `after` are provided, `before` will take precedence.
export type GetGuildBansOptions = {
    --- The limit of bans to get (max 100)
    limit: number?,
    --- Before a certain user ID
    before: discord.Snowflake?,
    --- After a certain user ID
    after: discord.Snowflake?
}

--- Options for sending a message to a channel in Discord
export type CreateMessageOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The data to send the message with
    data: discordRest.CreateMessageRequest
}

export type ReactionType = {
    type: "Unicode",
    data: string
} | {
    type: "Custom",
    animated: boolean,
    id: discord.Snowflake,
    name: string?
}

--- Options for creating a reaction in Discord
export type CreateReactionOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The message ID
    message_id: discord.Snowflake,
    --- The reaction to add
    reaction: ReactionType
}

--- Options for deleting the reaction AntiRaid has made on Discord
export type DeleteOwnReactionOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The message ID
    message_id: discord.Snowflake,
    --- The reaction to add
    reaction: ReactionType
}

--- Options for deleting the reaction of a user on Discord
export type DeleteUserReactionOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The message ID
    message_id: discord.Snowflake,
    --- The user ID
    user_id: discord.Snowflake,
    --- The reaction to add
    reaction: ReactionType
}

--- Options for deleting all reactions on a message
export type DeleteAllReactionsForEmojiOptions = {
    channel_id: discord.Snowflake,
    message_id: discord.Snowflake,
    reaction: ReactionType,
}

export type EditMessageOptions = {
    channel_id: discord.Snowflake,
    message_id: discord.Snowflake,
    data: discordRest.EditMessageRequest    
}

--- Options for creating a command in Discord
export type CreateCommandOptions = {
    --- The data to create the command with
    data: discordRest.CreateGuildApplicationCommandRequest
}

--- Options for creating multiple command in Discord
export type CreateCommandsOptions = {
    --- The data to create the command with
    data: {discordRest.CreateGuildApplicationCommandRequest}
}

--- Options for creating an interaction response in Discord
export type CreateInteractionResponseOptions = {
    --- The interaction ID
    interaction_id: discord.Snowflake,
    --- The interaction token
    interaction_token: string,
    --- The data to create the interaction response with
    data: discordRest.CreateInteractionRequest
}

--- Options for editting an interaction response in Discord
export type EditInteractionResponseOptions = {
    --- The interaction token
    interaction_token: string,
    --- The data to edit the interaction response with
    data: discordRest.EditWebhookMessageRequest
}

--- Options for getting a followup message in Discord
export type GetFollowupMessageOptions = {
    --- The interaction token
    interaction_token: string,
    --- The message ID
    message_id: discord.Snowflake
}

--- Options for creating a followup message in Discord
export type CreateFollowupMessageOptions = {
    --- The interaction token
    interaction_token: string,
    --- The data to create the followup message with
    data: discordRest.CreateFollowupMessageRequest
}

--- Options for editting a followup message in Discord
export type EditFollowupMessageOptions = {
    --- The interaction token
    interaction_token: string,
    --- The message ID
    message_id: discord.Snowflake,
    --- The data to edit the followup message with
    data: discordRest.EditWebhookMessageRequest
}

--- Options for deleting a followup message in Discord
export type DeleteFollowupMessageOptions = {
    --- The interaction token
    interaction_token: string,
    --- The message ID
    message_id: discord.Snowflake
}

--- A message pagination object
export type MessagePagination = { type: "After" | "Around" | "Before", id: discord.Snowflake }

--- Options for getting messages in Discord
export type GetMessagesOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The target message
    target: MessagePagination?,
    --- The limit of messages to get
    limit: number?
}

--- Options for getting a message in Discord
export type GetMessageOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The message ID
    message_id: discord.Snowflake
}

--- Options for creating a guild ban in Discord
export type CreateGuildBanOptions = {
    --- The user ID to ban
    user_id: discord.Snowflake,
    --- The reason for the ban
    reason: string,
    --- The number of seconds to delete messages from
    delete_message_seconds: number?
}

--- Options for removing a guild ban in Discord
export type RemoveGuildBanOptions = {
    --- The user ID to unban
    user_id: discord.Snowflake,
    --- The reason for the unban
    reason: string
}

--- Options for getting guild members
export type GetGuildMembersOptions = {
    --- The limit of members to get
    limit: number?,
    --- The user ID to get members after
    after: discord.Snowflake?
}

--- Options for searching guild members
export type SearchGuildMembersOptions = {
    --- The query to search for
    query: string,
    --- The limit of members to get
    limit: number?,
}

--- Options for modifying a guild member
export type ModifyGuildMemberOptions = {
    --- The user ID to modify
    user_id: discord.Snowflake,
    --- The reason for the modification
    reason: string,
    --- The data to modify the member with
    data: discordRest.ModifyGuildMemberRequest
}

--- Options for modifying a guild
export type ModifyGuildOptions = {
    data: discordRest.ModifyGuildRequest,
    reason: string
}

--- Options for checking if a user has the needed Discord permissions to perform an action
export type AntiRaidCheckPermissionsOptions = {
    --- The user ID to check permissions for
    user_id: discord.Snowflake,
    --- The needed permissions
    needed_permissions: string
}

--- Options for checking if a user has the needed Discord permissions to perform an action 
--- and is above the target user in terms of hierarchy
export type AntiRaidCheckPermissionsAndHierarchyOptions = {
    --- The user ID to check permissions for
    user_id: discord.Snowflake,
    --- The target ID to check permissions for
    target_id: discord.Snowflake,
    --- The needed permissions
    needed_permissions: string
}

--- Extra/additional success response for checking if a user has the needed Discord permissions to perform an action
export type AntiRaidCheckPermissionsResponse = {
    --- The partial guild
    partial_guild: discord.Partial<discord.GuildObject>,
    --- The member
    member: discord.GuildMemberObject,
    --- The permissions
    permissions: string
}

export type AntiRaidCheckChannelPermissionsOptions = {
    --- The user ID to check permissions for
    user_id: discord.Snowflake,
    --- The channel ID to check permissions for
    channel_id: discord.Snowflake,
    --- The needed permissions
    needed_permissions: string
}

export type AntiRaidCheckChannelPermissionsResponse = {
    --- The partial guild
    partial_guild: discord.Partial<discord.GuildObject>,
    --- The channel
    channel: discord.ChannelObject,
    --- The member
    member: discord.GuildMemberObject,
    --- The permissions
    permissions: string
}

export type AntiraidFusedMemberSingle = {
    --- The member
    member: discord.GuildMemberObject,
    --- The resolved permissions of the member in the guild
    resolved_perms: string
}

--- A fused member contains both a member, the guild and the resolved permissions of
--- the member in the guild. This is useful for operations that require both the member and the guild context, such as permission checks.
export type AntiraidFusedMember = {
    --- The partial guild
    guild: discord.Partial<discord.GuildObject>,
    --- The member and resolved permissions
    members: {AntiraidFusedMemberSingle}
}

--- Options for creating a guild role
export type CreateGuildRoleOptions = {
    --- The reason for the creation
    reason: string,
    --- The data to create the role with
    data: discordRest.CreateGuildRoleRequest
}

--- Options for modifying a guild role position
export type ModifyRolePositionOptions = {
    --- The data to modify the role position with
    data: {discordRest.ModifyGuildRolePositionsRequest},
    --- The reason for the modification
    reason: string
}

--- Options for modifying a guild role
export type EditGuildRoleOptions = {
    --- The reason for the creation
    reason: string,
    --- The data to create the role with
    data: discordRest.ModifyGuildRoleRequest,
    --- Role ID
    role_id: discord.Snowflake
}

--- Options for deleting a guild role
export type DeleteGuildRoleOptions = {
    --- The reason for the creation
    reason: string,
    --- Role ID
    role_id: discord.Snowflake
}

export type CreateChannelInviteOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The data to create the invite with
    data: discordRest.CreateChannelInviteRequest,
    --- The reason for the creation
    reason: string
}

export type DeleteChannelPermissionOptions = {
    --- The channel ID
    channel_id: discord.Snowflake,
    --- The overwrite ID
    overwrite_id: discord.Snowflake,
    --- The reason for the deletion
    reason: string
}

export type FollowAnnouncementChannelOptions = {
    --- The Channel ID
    channel_id: discord.Snowflake,
    --- Data
    data: discordRest.FollowAnnouncementChannelRequest,
    --- Reason
    reason: string,
}

--[[
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GetInviteOptions {
    pub code: String,
    pub with_counts: Option<bool>, // default to false
    pub with_expiration: Option<bool>, // default to false
    pub guild_scheduled_event_id: Option<serenity::all::ScheduledEventId>,    
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DeleteInviteOptions {
    pub code: String,
    pub reason: String,
}
]]
export type GetInviteOptions = {
    --- The invite code
    code: string,
    --- Whether to include counts
    with_counts: boolean?,
    --- Whether to include expiration
    with_expiration: boolean?,
    --- The guild scheduled event ID
    guild_scheduled_event_id: discord.Snowflake?
}

export type DeleteInviteOptions = {
    --- The invite code
    code: string,
    --- The reason for the deletion
    reason: string
}

export type DeleteMessageOptions = {
    channel_id: discord.Snowflake, 
    message_id: discord.Snowflake, 
    reason: string
}

export type BulkDeleteMessagesOptions = {
    channel_id: discord.Snowflake, 
    message_ids: {discord.Snowflake}, 
    reason: string
}

export type CrosspostMessageOptions = {
    channel_id: discord.Snowflake,
    message_id: discord.Snowflake
}

export type DeleteAllReactionsOptions = {
    channel_id: discord.Snowflake,
    message_id: discord.Snowflake
}

--- Lazy<{discord.UserObject}>
export type LazyUsersObject = lazy.Lazy<discord.UserObject>
-- Lazy<AntiraidFusedMember>
export type LazyAntiraidFusedMember = lazy.Lazy<AntiraidFusedMember>
--- Lazy<discord.AuditLogObject>
export type LazyAuditLogObject = lazy.Lazy<discord.AuditLogObject>
--- Lazy<discord.ChannelObject>
export type LazyChannelObject = lazy.Lazy<discord.ChannelObject>
--- Lazy<{discord.ChannelObject}>
export type LazyChannelsObject = lazy.Lazy<{discord.ChannelObject}>
--- Lazy<discord.MessageObject> 
export type LazyMessageObject = lazy.Lazy<discord.MessageObject>
--- Lazy<{discord.MessageObject}>
export type LazyMessagesObject = lazy.Lazy<{discord.MessageObject}>
--- Lazy<discord.ApplicationCommandObject>
export type LazyApplicationCommandObject = lazy.Lazy<discord.ApplicationCommandObject>
--- Lazy<{discord.ApplicationCommandObject}>
export type LazyApplicationCommandsObject = lazy.Lazy<{discord.ApplicationCommandObject}>
--- Lazy<{discord.AutomoderationRuleObject}>
export type LazyAutomoderationRuleObjectList = lazy.Lazy<{discord.AutomoderationRuleObject}>
--- Lazy<discord.AutomoderationRuleObject>
export type LazyAutomoderationRuleObject = lazy.Lazy<discord.AutomoderationRuleObject>

--- Lazy<discord.GuildBanObject?>
export type LazyBanOptionalObject = lazy.Lazy<discord.BanObject?>
--- Lazy<{discord.GuildBanObject}>
export type LazyBanObjectList = lazy.Lazy<{discord.BanObject}>

--- Lazy<discord.GuildRoleObject>
export type LazyRoleObject = lazy.Lazy<discord.GuildRoleObject>
--- Lazy<{[discord.Snowflake]: discord.GuildRoleObject}>
export type LazyRolesObject = lazy.Lazy<{discord.GuildRoleObject}>
--- Lazy<discordRest.ListActiveGuildThreadsResponse>
export type LazyActiveGuildThreadsResponse = lazy.Lazy<discordRest.ListActiveGuildThreadsResponse>
--- Lazy<discord.GuildObject>
export type LazyGuildObject = lazy.Lazy<discord.GuildObject>
--- Lazy<discord.GuildPreviewObject>
export type LazyGuildPreviewObject = lazy.Lazy<discord.GuildPreviewObject>
--- Lazy<discord.GuildMemberObject>
export type LazyGuildMemberObject = lazy.Lazy<discord.GuildMemberObject>
--- Lazy<{discord.GuildMemberObject}>
export type LazyGuildMembersObject = lazy.Lazy<{discord.GuildMemberObject}>
--- Lazy<{discord.InviteObject}>
export type LazyInviteObjectList = lazy.Lazy<{discord.InviteObject}>
--- Lazy<discord.InviteObject>
export type LazyInviteObject = lazy.Lazy<discord.InviteObject>

export type DiscordRequest = 
    -- AntiRaid Helpers
    
    --- Checks an audit log reason for validity, errors out if invalid
    | { op: "AntiRaidCheckReason", reason: string }
    --- Checks if a specified user with an ID of `data.user_id` has the specified permissions in the server
    | { op: "AntiRaidCheckPermissions", data: AntiRaidCheckPermissionsOptions }
    --- Checks if a specified user has the specified permissions in the server and is above the target user in terms of hierarchy
    | { op: "AntiRaidCheckPermissionsAndHierarchy", data: AntiRaidCheckPermissionsAndHierarchyOptions }
    --- Checks if a specified user has the specified permissions in the channel
    | { op: "AntiRaidCheckChannelPermissions", data: AntiRaidCheckChannelPermissionsOptions }
    --- Fetches the member and resolved permissions of a user in the guild (also called a 'fused member')
    | { op: "AntiRaidGetFusedMember", ids: {discord.Snowflake} }

    -- Audit Logs

    --- Gets the audit logs
    | { op: "GetAuditLog", data: GetAuditLogOptions }

    -- Auto Moderation

    --- Lists the auto moderation rules available
    | { op: "ListAutoModerationRules" }
    --- Gets an auto moderation rule by ID
    | { op: "GetAutoModerationRule", data: GetAutoModerationRuleOptions }
    --- Creates an auto moderation rule
    | { op: "CreateAutoModerationRule", data: CreateAutoModerationRuleOptions }
    --- Edits an auto moderation rule
    | { op: "EditAutoModerationRule", data: EditAutoModerationRuleOptions }
    --- Deletes an auto moderation rule
    | { op: "DeleteAutoModerationRule", data: DeleteAutoModerationRuleOptions }

    -- Channels (By ID)

    --- Gets a channel
    | { op: "GetChannel", channel_id: string }
    --- Edits a channel
    | { op: "EditChannel", data: EditChannelOptions }
    --- Deletes a channel
    | { op: "DeleteChannel", data: DeleteChannelOptions }
    --- Edits channel permissions for a target
    | { op: "EditChannelPermissions", data: EditChannelPermissionsOptions }
    --- Gets all invites a channel has
    | { op: "GetChannelInvites" } 
    --- Creates a channel invite
    | { op: "CreateChannelInvite", data: CreateChannelInviteOptions }
    --- Deletes a channel permission
    | { op: "DeleteChannelPermission", data: DeleteChannelPermissionOptions }
    --- Follows an announcement channel
    | { op: "FollowAnnouncementChannel", data: FollowAnnouncementChannelOptions }

    -- Guild Base

    --- Gets the guild
    | { op: "GetGuild" }
    --- Gets the guilds preview
    | { op: "GetGuildPreview" }
    --- Edits the guild
    | { op: "ModifyGuild", data: ModifyGuildOptions }
    --- Gets the guild channels
    | { op: "GetGuildChannels" }
    --- Creates a guild channel
    | { op: "CreateGuildChannel", data: CreateChannelOptions }
    --- Modify guild channel positions. Only channels to be modified are required to be passed in `data`.
    | { op: "ModifyGuildChannelPositions", data: {discordRest.ModifyGuildChannelPositionsRequest} }
    --- List active guild threads
    | { op: "ListActiveGuildThreads" }

    -- Guild Members

    --- Gets a guild member by ID
    | { op: "GetGuildMember", user_id: string }
    --- List all guild members
    | { op: "ListGuildMembers", data: GetGuildMembersOptions }
    --- Search guild members
    | { op: "SearchGuildMembers", data: SearchGuildMembersOptions }
    --- Modify guild member (this includes timing out a member using `communication_disabled_until`)
    | { op: "ModifyGuildMember", data: ModifyGuildMemberOptions }
    --- Adds a role to a member
    | { op: "AddGuildMemberRole", data: AddGuildMemberRoleOptions }
    --- Removes a role from a member
    | { op: "RemoveGuildMemberRole", data: RemoveGuildMemberRoleOptions }
    --- Removes a member from a guild
    | { op: "RemoveGuildMember", data: RemoveGuildMemberOptions }

    -- Guild Bans

    --- Gets guild bans
    | { op: "GetGuildBans", data: GetGuildBansOptions }
    --- Gets a guild ban for a user or nil if it does not exist
    | { op: "GetGuildBan", user_id: string }
    --- Creates a guild ban
    | { op: "CreateGuildBan", data: CreateGuildBanOptions }
    --- Removes a guild ban
    | { op: "RemoveGuildBan", data: RemoveGuildBanOptions }

    -- Guild Roles

    --- Returns the guild roles of a guild
    | { op: "GetGuildRoles" }
    --- Returns a guild role by ID
    | { op: "GetGuildRole", role_id: string }
    --- Creates a guild role
    | { op: "CreateGuildRole", data: CreateGuildRoleOptions }
    --- Modify guild role positions
    | { op: "ModifyGuildRolePositions", data: ModifyRolePositionOptions }
    --- Modifies a guild role
    | { op: "ModifyGuildRole", data: EditGuildRoleOptions }
    --- Deletes a guild role
    | { op: "DeleteGuildRole", data: DeleteGuildRoleOptions }

    -- Guild Commands

    --- Gets all guild commands currently registered
    | { op: "GetGuildCommands" }
    --- Gets a guild command by ID
    | { op: "GetGuildCommand", command_id: string }
    --- Creates a guild command
    | { op: "CreateGuildCommand", data: CreateCommandOptions }
    --- Creates multiple guild commands
    | { op: "CreateGuildCommands", data: CreateCommandsOptions }

    -- Invites (By Code)

    --- Gets an invite by code
    | { op: "GetInvite", data: GetInviteOptions }
    --- Deletes an invite by code
    | { op: "DeleteInvite", data: DeleteInviteOptions }

    -- Messages

    --- Gets messages from a channel
    | { op: "GetChannelMessages", data: GetMessagesOptions }
    --- Gets a message
    | { op: "GetChannelMessage", data: GetMessageOptions }
    --- Creates a message
    | { op: "CreateMessage", data: CreateMessageOptions }
    --- Crossposts a message to an announcement channel
    | { op: "CrosspostMessage", data: CrosspostMessageOptions }
    --- Edits a message
    | { op: "EditMessage", data: EditMessageOptions }
    --- Deletes a message
    | { op: "DeleteMessage", data: DeleteMessageOptions }
    --- Bulk deletes messages in a channel
    | { op: "BulkDeleteMessages", data: BulkDeleteMessagesOptions }

    -- Reactions

    --- Creates a reaction to a message
    | { op: "CreateReaction", data: CreateReactionOptions }
    --- Deletes the reaction AntiRaid has made on a message
    | { op: "DeleteOwnReaction", data: DeleteOwnReactionOptions }
    --- Deletes a reaction another user has made on a message
    | { op: "DeleteUserReaction", data: DeleteUserReactionOptions }
    --- Deletes all reactions on a message
    | { op: "DeleteAllReactions", data: DeleteAllReactionsOptions }
    --- Deletes all reactions for a specific emoji on a message
    | { op: "DeleteAllReactionsForEmoji", data: DeleteAllReactionsForEmojiOptions }

    -- Interactions

    --- Creates an interaction response
    | { op: "CreateInteractionResponse", data: CreateInteractionResponseOptions }
    --- Gets the original interaction response
    | { op: "GetOriginalInteractionResponse", interaction_token: string }
    --- Edits the original interaction response
    | { op: "EditOriginalInteractionResponse", data: EditInteractionResponseOptions }
    --- Deletes the original interaction response
    | { op: "DeleteOriginalInteractionResponse", interaction_token: string }
    --- Gets a followup interaction response
    | { op: "GetFollowupMessage", data: GetFollowupMessageOptions }
    --- Creates a followup interaction response
    | { op: "CreateFollowupMessage", data: CreateFollowupMessageOptions }
    --- Edits a followup interaction response
    | { op: "EditFollowupMessage", data: EditFollowupMessageOptions }
    --- Deletes a followup interaction response
    | { op: "DeleteFollowupMessage", data: DeleteFollowupMessageOptions }

export type DiscordResponse = 
    -- AntiRaid Helpers
    | { op: "AntiRaidCheckReason" } -- Ack (No return data)
    | { op: "AntiRaidCheckPermissions", res: AntiRaidCheckPermissionsResponse }
    | { op: "AntiRaidCheckPermissionsAndHierarchy", res: AntiRaidCheckPermissionsResponse }
    | { op: "AntiRaidCheckChannelPermissions", res: AntiRaidCheckChannelPermissionsResponse }
    | { op: "AntiRaidGetFusedMember", res: LazyAntiraidFusedMember }

    -- Audit Logs
    | { op: "GetAuditLog", res: LazyAuditLogObject }

    -- Auto Moderation
    | { op: "ListAutoModerationRules", res: LazyAutomoderationRuleObjectList }
    | { op: "GetAutoModerationRule", res: LazyAutomoderationRuleObject }
    | { op: "CreateAutoModerationRule", res: LazyAutomoderationRuleObject }
    | { op: "EditAutoModerationRule", res: LazyAutomoderationRuleObject }
    | { op: "DeleteAutoModerationRule", res: LazyAutomoderationRuleObject }

    -- Channels
    | { op: "GetChannel", res: LazyChannelObject }
    | { op: "EditChannel", res: LazyChannelObject }
    | { op: "DeleteChannel", res: LazyChannelObject }
    | { op: "EditChannelPermissions" } -- Ack
    | { op: "GetChannelInvites", res: LazyInviteObjectList }
    | { op: "CreateChannelInvite", res: LazyInviteObject }
    | { op: "DeleteChannelPermission" } -- Ack
    | { op: "FollowAnnouncementChannel", res: LazyChannelObject }

    -- Guild Base
    | { op: "GetGuild", res: LazyGuildObject }
    | { op: "GetGuildPreview", res: LazyGuildPreviewObject }
    | { op: "ModifyGuild", res: LazyGuildObject }
    | { op: "GetGuildChannels", res: LazyChannelsObject }
    | { op: "CreateGuildChannel", res: LazyChannelObject }
    | { op: "ModifyGuildChannelPositions" } -- Ack
    | { op: "ListActiveGuildThreads", res: LazyActiveGuildThreadsResponse }

    -- Guild Members
    | { op: "GetGuildMember", res: LazyGuildMemberObject }
    | { op: "ListGuildMembers", res: LazyGuildMembersObject }
    | { op: "SearchGuildMembers", res: LazyGuildMembersObject }
    | { op: "ModifyGuildMember", res: LazyGuildMemberObject }
    | { op: "AddGuildMemberRole" } -- Ack
    | { op: "RemoveGuildMemberRole" } -- Ack
    | { op: "RemoveGuildMember" } -- Ack

    -- Guild Bans
    | { op: "GetGuildBans", res: LazyBanObjectList }
    | { op: "GetGuildBan", res: LazyBanOptionalObject }
    | { op: "CreateGuildBan" } -- Ack
    | { op: "RemoveGuildBan" } -- Ack

    -- Guild Roles
    | { op: "GetGuildRoles", res: LazyRolesObject }
    | { op: "GetGuildRole", res: LazyRoleObject }
    | { op: "CreateGuildRole", res: LazyRoleObject }
    | { op: "ModifyGuildRolePositions", res: LazyRolesObject }
    | { op: "ModifyGuildRole", res: LazyRoleObject }
    | { op: "DeleteGuildRole" } -- Ack

    -- Guild Commands
    | { op: "GetGuildCommands", res: LazyApplicationCommandsObject }
    | { op: "GetGuildCommand", res: LazyApplicationCommandObject }
    | { op: "CreateGuildCommand", res: LazyApplicationCommandObject }
    | { op: "CreateGuildCommands", res: LazyApplicationCommandsObject }

    -- Invites
    | { op: "GetInvite", res: LazyInviteObject }
    | { op: "DeleteInvite", res: LazyInviteObject }

    -- Messages
    | { op: "GetChannelMessages", res: LazyMessagesObject }
    | { op: "GetChannelMessage", res: LazyMessageObject }
    | { op: "CreateMessage", res: LazyMessageObject }
    | { op: "CrosspostMessage", res: LazyMessageObject }
    | { op: "EditMessage", res: LazyMessageObject }
    | { op: "DeleteMessage" } -- Ack
    | { op: "BulkDeleteMessages" } -- Ack

    -- Reactions
    | { op: "CreateReaction" } -- Ack
    | { op: "DeleteOwnReaction" } -- Ack
    | { op: "DeleteUserReaction" } -- Ack
    | { op: "DeleteAllReactions" } -- Ack
    | { op: "DeleteAllReactionsForEmoji" } -- Ack

    -- Interactions
    | { op: "CreateInteractionResponse" } -- Ack
    | { op: "GetOriginalInteractionResponse", res: LazyMessageObject }
    | { op: "EditOriginalInteractionResponse", res: LazyMessageObject }
    | { op: "DeleteOriginalInteractionResponse" } -- Ack
    | { op: "GetFollowupMessage", res: LazyMessageObject }
    | { op: "CreateFollowupMessage", res: LazyMessageObject }
    | { op: "EditFollowupMessage", res: LazyMessageObject }
    | { op: "DeleteFollowupMessage" } -- Ack

return {}