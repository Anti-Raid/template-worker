use antiraid_types::userinfo::UserInfo;
use ar_settings::types::{
    settings_wrap, Column, ColumnSuggestion, ColumnType, InnerColumnType,
    OperationType, Setting, SettingOperations,
};
use ar_settings::types::{
    SettingCreator, SettingDeleter, SettingUpdater, SettingView,
};
use kittycat::perms::Permission;
use serde_json::Value;
use antiraid_types::ar_event::{AntiraidEvent, ExternalKeyUpdateEventData, ExternalKeyUpdateEventDataAction};
use crate::templatingrt::state::LuaKVConstraints;
use crate::templatingrt::template::Template;
use crate::userinfo::{NoMember, UserInfoOperations, member_permission_calc};
use std::sync::LazyLock;
use async_trait::async_trait;
use super::data::SettingsData;
use crate::templatingrt::cache::{DeferredCacheRegenMode, DEFERRED_CACHE_REGENS};
use crate::Error;
use crate::lockdowns::LockdownData;
use sqlx::Row;

async fn check_perms(
    ctx: &SettingsData,
    perm: kittycat::perms::Permission,
) -> Result<(), Error> {
    let guild_id = ctx.scope.guild_id()?;
    let user_id = ctx.scope.user_id()?;

    let user_info = UserInfo::get(
        guild_id,
        user_id,
        &ctx.data.pool,
        &ctx.serenity_context,
        &ctx.data.reqwest,
        None::<NoMember>, // No poise context available
    )
    .await?;

    if !kittycat::perms::has_perm(
        &user_info.kittycat_resolved_permissions,
        &perm,
    ) {
        return Err(
            format!("You do not have permission to perform this action: {}", perm).into(),
        );
    }

    Ok(())
}

pub static GUILD_TEMPLATES: LazyLock<Setting<SettingsData>> = LazyLock::new(|| {
    Setting {
        id: "scripts".to_string(),
        name: "Scripts".to_string(),
        description: "Configure your servers' custom scripts.".to_string(),
        columns: settings_wrap(vec![
            ar_settings::common_columns::guild_id("guild_id", "Guild ID", "The Guild ID"),
            Column {
                id: "name".to_string(),
                name: "Name".to_string(),
                description: "The name to give to the script".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: true,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "language".to_string(),
                name: "Language".to_string(),
                description: "The language of the script. Only Roblox Luau is currently supported here.".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec!["luau".to_string()],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "content".to_string(),
                name: "Content".to_string(),
                description: "The content of the script".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::Json {
                    kind: "template".to_string(),
                    max_bytes: Some(1024 * 1024 * 5), // 5MB
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "paused".to_string(),
                name: "Paused".to_string(),
                description: "Whether the script is paused or not".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::Boolean {}),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "events".to_string(),
                name: "Events".to_string(),
                description: "The events that this script can be executed on.".to_string(),
                column_type: ColumnType::new_array(InnerColumnType::String { 
                    min_length: None, 
                    max_length: None, 
                    allowed_values: vec![],
                    kind: "normal".to_string()
                }),
                primary_key: false,
                nullable: true,
                suggestions: ColumnSuggestion::Static {
                    suggestions: {
                        let mut vec = AntiraidEvent::variant_names()
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                        
                        vec.extend(gwevent::core::event_list().iter().copied().map(|x| x.to_string()).collect::<Vec<String>>());

                        vec
                    }
                },
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "allowed_caps".to_string(),
                name: "Capabilities".to_string(),
                description: "The capabilities the script will have.".to_string(),
                column_type: ColumnType::new_array(InnerColumnType::String { min_length: None, max_length: None, allowed_values: vec![], kind: "normal".to_string() }),
                primary_key: false,
                nullable: true,
                suggestions: ColumnSuggestion::Static {
                    suggestions: vec![
                        "discord:create_message".to_string()
                    ]
                },
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "error_channel".to_string(),
                name: "Error Channel".to_string(),
                description: "The channel to report any errors to".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "channel".to_string(),
                    min_length: None,
                    max_length: None,
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: true,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            ar_settings::common_columns::created_at(),
            ar_settings::common_columns::created_by(),
            ar_settings::common_columns::last_updated_at(),
            ar_settings::common_columns::last_updated_by(),
        ]),
        title_template: "{name}".to_string(),
        operations: SettingOperations::from(GuildTemplateExecutor),
    }
});

#[derive(Clone)]
pub struct GuildTemplateExecutor;

impl GuildTemplateExecutor {
    async fn validate_channel(&self, ctx: &SettingsData, channel_field: &str, channel_id: serenity::all::ChannelId) -> Result<(), Error> {
        // Perform required checks
        let channel = crate::sandwich::channel(
            &ctx.serenity_context.cache,
            &ctx.serenity_context.http,
            &ctx.data.reqwest,
            Some(ctx.scope.guild_id()?),
            channel_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch channel id: {} with field: {}", e, channel_field))?;

        let Some(channel) = channel else {
            return Err(format!("Could not find channel with id: {} and field: {}", channel_id, channel_field).into());
        };

        let Some(guild_channel) = channel.guild() else {
            return Err(format!("Channel with id: {} and field: {} is not in a guild", channel_id, channel_field).into());
        };

        if guild_channel.guild_id != ctx.scope.guild_id()? {
            return Err(format!("Channel with id: {} and field: {} is not in the same guild as the setting", channel_id, channel_field).into());
        }

        let bot_user_id =
            ctx.serenity_context.cache.current_user().id;

        let bot_user = crate::sandwich::member_in_guild(
            &ctx.serenity_context.cache,
            &ctx.serenity_context.http,
            &ctx.data.reqwest,
            ctx.scope.guild_id()?,
            bot_user_id,
        )
        .await
        .map_err(|e| {
            format!(
                "Failed to get bot user: {}",
                e
            )
        })?;

        let Some(bot_user) = bot_user else {
            return Err(
                format!(
                    "Could not find bot user: {}",
                    bot_user_id
                )
                .into()
            );
        };

        let guild = crate::sandwich::guild(
            &ctx.serenity_context.cache,
            &ctx.serenity_context.http,
            &ctx.data.reqwest,
            ctx.scope.guild_id()?,
        )
        .await
        .map_err(|e| 
            format!(
                "Failed to get guild: {}",
                e
            )
        )?;

        let permissions =
            guild.user_permissions_in(&guild_channel, &bot_user);

        if !permissions.contains(serenity::all::Permissions::SEND_MESSAGES) {
            return Err(
                format!("Bot does not have permission to `Send Messages` in channel with id: {} and field: {}", channel_id, channel_field).into()
            );
        }

        Ok(())        
    }

    async fn validate(&self, ctx: &SettingsData, name: &str) -> Result<(), Error> {
        if name.starts_with("$shop/") {
            let (shop_tname, shop_tversion) = Template::parse_shop_template(name)
                .map_err(|e| format!("Failed to parse shop template: {:?}", e))?;

            let shop_template_count = sqlx::query(
                "SELECT COUNT(*) FROM template_shop WHERE name = $1 AND version = $2",
            )
            .bind(shop_tname)
            .bind(shop_tversion)
            .fetch_one(&ctx.data.pool)
            .await
            .map_err(|e| format!("Failed to get shop template: {:?}", e))?
            .try_get::<Option<i64>, _>(0)
            .map_err(|e| format!("Failed to get count: {:?}", e))?
            .unwrap_or_default();

            if shop_template_count == 0 {
                return Err("Shop template does not exist".into());
            }
        }

        Ok(())
    }

    async fn post_action(&self, ctx: &SettingsData, name: &str) -> Result<(), Error> {
        DEFERRED_CACHE_REGENS.insert(ctx.scope.guild_id()?, DeferredCacheRegenMode::OnReady 
        { 
            modified: vec![name.to_string()], 
        }).await;

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingView<SettingsData> for GuildTemplateExecutor {
    async fn view<'a>(
        &self,
        context: &SettingsData,
        _filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        log::info!("Viewing guild templates for guild id: {}", context.scope.guild_id()?);

        check_perms(context,"guild_templates.view".into()).await?;

        #[derive(sqlx::FromRow)]
        struct TemplateRow {
            name: String,
            content: serde_json::Value,
            language: String,
            allowed_caps: Vec<String>,
            paused: bool,
            events: Vec<String>,
            error_channel: Option<String>,
            created_at: chrono::DateTime<chrono::Utc>,
            created_by: String,
            last_updated_at: chrono::DateTime<chrono::Utc>,
            last_updated_by: String,
        }

        let rows: Vec<TemplateRow> = sqlx::query_as("SELECT name, content, language, allowed_caps, paused, events, error_channel, created_at, created_by, last_updated_at, last_updated_by FROM guild_templates WHERE guild_id = $1")
        .bind(context.scope.guild_id()?.to_string())
        .fetch_all(&context.data.pool)
        .await
        .map_err(|e| format!("Error while fetching guild templates: {}", e))?;

        let mut result = vec![];

        for row in rows {
            let map = indexmap::indexmap! {
                "guild_id".to_string() => Value::String(context.scope.guild_id()?.to_string()),
                "name".to_string() => Value::String(row.name),
                "content".to_string() => row.content,
                "language".to_string() => Value::String(row.language),
                "allowed_caps".to_string() => {
                    Value::Array(row.allowed_caps.iter().map(|x| Value::String(x.to_string())).collect())
                },
                "paused".to_string() => Value::Bool(row.paused),
                "events".to_string() => {
                    Value::Array(row.events.iter().map(|x| Value::String(x.to_string())).collect())
                },
                "error_channel".to_string() => {
                    match row.error_channel {
                        Some(error_channel) => Value::String(error_channel),
                        None => Value::Null,
                    }
                },
                "created_at".to_string() => Value::String(row.created_at.to_string()),
                "created_by".to_string() => Value::String(row.created_by),
                "last_updated_at".to_string() => Value::String(row.last_updated_at.to_string()),
                "last_updated_by".to_string() => Value::String(row.last_updated_by),
            };

            result.push(map);
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
impl SettingCreator<SettingsData> for GuildTemplateExecutor {
    async fn create<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates.create".into()).await?;

        let Some(Value::String(name)) = entry.get("name") else {
            return Err("Missing or invalid field: `name`".into());
        };

        let count = sqlx::query(
            "SELECT COUNT(*) FROM guild_templates WHERE guild_id = $1 AND name = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(name)
        .fetch_one(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to check if template exists: {:?}", e))?
        .try_get::<Option<i64>, _>(0)
        .map_err(|e| format!("Failed to get count: {:?}", e))?
        .unwrap_or_default();

        if count > 0 {
            return Err("Template already exists".into());
        }

        self.validate(ctx, name).await?;

        let Some(Value::String(language)) = entry.get("language") else {
            return Err("Missing or invalid field: `language`".into());
        };

        let Some(content) = entry.get("content") else {
            return Err("Missing or invalid field: `content`".into());
        };

        // Try to parse content as a hashmap<String, String>
        let string_form = serde_json::to_string(&content)
            .map_err(|e| format!("Failed to convert content to string: {:?}", e))?;

        let _: indexmap::IndexMap<String, Value> = serde_json::from_str(&string_form)   
            .map_err(|e| format!("Failed to parse content: {:?}", e))?;     

        let Some(Value::Bool(paused)) = entry.get("paused") else {
            return Err("Missing or invalid field: `paused`".into());
        };

        let events = match entry.get("events") {
            Some(Value::Array(events)) => 
                events
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err("Failed to parse events".into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let allowed_caps = match entry.get("allowed_caps") {
            Some(Value::Array(allowed_caps)) => 
                allowed_caps
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err(format!("Failed to parse allowed capabilities due to invalid capability: {:?}", x).into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let error_channel = match entry.get("error_channel") {
            Some(Value::String(error_channel)) => {
                let channel_id: serenity::all::ChannelId = error_channel.parse()
                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                self.validate_channel(ctx, "error_channel", channel_id).await?;

                Some(error_channel.to_string())
            },
            _ => None,
        };

        sqlx::query(
            "INSERT INTO guild_templates (guild_id, name, language, content, events, paused, allowed_caps, error_channel, created_by, last_updated_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(name)
        .bind(language)
        .bind(content)
        .bind(&events)
        .bind(paused)
        .bind(&allowed_caps)
        .bind(&error_channel)
        .bind(ctx.scope.user_id()?.to_string())
        .bind(ctx.scope.user_id()?.to_string())
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to insert template: {:?}", e))?;

        self.post_action(ctx, name).await?;

        Ok(indexmap::indexmap! {
            "guild_id".to_string() => Value::String(ctx.scope.guild_id()?.to_string()),
            "name".to_string() => Value::String(name.to_string()),
            "language".to_string() => Value::String(language.to_string()),
            "content".to_string() => content.clone(),
            "events".to_string() => Value::Array(events.iter().map(|x| Value::String(x.to_string())).collect()),
            "paused".to_string() => Value::Bool(*paused),
            "allowed_caps".to_string() => Value::Array(allowed_caps.iter().map(|x| Value::String(x.to_string())).collect()),
            "error_channel".to_string() => {
                match error_channel {
                    Some(error_channel) => Value::String(error_channel),
                    None => Value::Null,
                }
            },
        })
    }
}

#[async_trait::async_trait]
impl SettingUpdater<SettingsData> for GuildTemplateExecutor {
    async fn update<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates.update".into()).await?;

        let Some(Value::String(name)) = entry.get("name") else {
            return Err("Missing or invalid field: `name`".into());
        };

        self.validate(ctx, name).await?;

        let Some(Value::String(language)) = entry.get("language") else {
            return Err("Missing or invalid field: `language`".into());
        };

        let Some(content) = entry.get("content") else {
            return Err("Missing or invalid field: `content`".into());
        };

        // Try to parse content as a hashmap<String, String>
        let string_form = serde_json::to_string(&content)
            .map_err(|e| format!("Failed to convert content to string: {:?}", e))?;

        let _: indexmap::IndexMap<String, Value> = serde_json::from_str(&string_form)   
            .map_err(|e| format!("Failed to parse content: {:?}", e))?;     

        let events = match entry.get("events") {
            Some(Value::Array(events)) => 
                events
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err(format!("Failed to parse events due to invalid event: {:?}", x).into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let Some(Value::Bool(paused)) = entry.get("paused") else {
            return Err("Missing or invalid field: `paused`".into());
        };

        let allowed_caps = match entry.get("allowed_caps") {
            Some(Value::Array(allowed_caps)) => 
                allowed_caps
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err(format!("Failed to parse allowed capabilities due to invalid capability: {:?}", x).into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };
        

        let error_channel = match entry.get("error_channel") {
            Some(Value::String(error_channel)) => {
                let channel_id: serenity::all::ChannelId = error_channel.parse()
                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                self.validate_channel(ctx, "error_channel", channel_id).await?;

                Some(error_channel.to_string())
            },
            _ => None,
        };

        sqlx::query(
            "UPDATE guild_templates SET content = $1, events = $2, allowed_caps = $3, language = $4, paused = $9, last_updated_at = NOW(), last_updated_by = $5, error_channel = $6 WHERE guild_id = $7 AND name = $8",
        )
        .bind(content)
        .bind(&events)
        .bind(&allowed_caps)
        .bind(language)
        .bind(ctx.scope.user_id()?.to_string())
        .bind(error_channel)
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(name)
        .bind(paused)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to update template: {:?}", e))?;

        self.post_action(ctx, name).await?;

        Ok(entry)
    }
}

#[async_trait::async_trait]
impl SettingDeleter<SettingsData> for GuildTemplateExecutor {
    async fn delete<'a>(
        &self,
        ctx: &SettingsData,
        mut fields: indexmap::IndexMap<String, Value>,
    ) -> Result<(), Error> {
        check_perms(ctx, "guild_templates.delete".into()).await?;

        let Some(Value::String(primary_key)) = fields.swap_remove("name") else {
            return Err("Invalid primary key".into());
        };

        let Some(row) = sqlx::query(
            "SELECT name FROM guild_templates WHERE guild_id = $1 AND name = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(primary_key)
        .fetch_optional(&ctx.data.pool)
        .await
        .map_err(|e| format!("Error while fetching template: {}", e))?
        else {
            return Err("Template not found when trying to delete it!".into());
        };

        let name = row.try_get::<String, _>(0).map_err(|e| format!("Failed to get name: {:?}", e))?;

        sqlx::query(
            "DELETE FROM guild_templates WHERE guild_id = $1 AND name = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(&name)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to delete template: {:?}", e))?;

        self.post_action(ctx, &name).await?;

        Ok(())
    }
}

pub static GUILD_TEMPLATES_KV: LazyLock<Setting<SettingsData>> = LazyLock::new(|| Setting {
    id: "script_kv".to_string(),
    name: "Scripts (key-value db)".to_string(),
    description: "Key-value database available to scripts on this server".to_string(),
    columns: settings_wrap(vec![
        ar_settings::common_columns::guild_id("guild_id", "Guild ID", "The Guild ID"),
        Column {
            id: "key".to_string(),
            name: "Key".to_string(),
            description: "Key".to_string(),
            column_type: ColumnType::new_scalar(InnerColumnType::String {
                kind: "normal".to_string(),
                min_length: None,
                max_length: Some(LuaKVConstraints::default().max_key_length),
                allowed_values: vec![],
            }),
            primary_key: true,
            nullable: false,
            suggestions: ColumnSuggestion::None {},
            ignored_for: vec![],
            secret: false,
        },
        Column {
            id: "scope".to_string(),
            name: "Scope".to_string(),
            description: "Scope of the key. 'unscoped' is default if unset".to_string(),
            column_type: ColumnType::new_scalar(InnerColumnType::String {
                kind: "normal".to_string(),
                min_length: None,
                max_length: Some(LuaKVConstraints::default().max_key_length),
                allowed_values: vec![],
            }),
            primary_key: true,
            nullable: false,
            suggestions: ColumnSuggestion::None {},
            ignored_for: vec![],
            secret: false,
        },
        Column {
            id: "value".to_string(),
            name: "Value".to_string(),
            description: "The value of the record".to_string(),
            column_type: ColumnType::new_scalar(InnerColumnType::Json {
                kind: "kv_value".to_string(),
                max_bytes: Some(LuaKVConstraints::default().max_value_bytes),
            }),
            primary_key: false,
            nullable: true,
            suggestions: ColumnSuggestion::None {},
            ignored_for: vec![],
            secret: false,
        },
        ar_settings::common_columns::created_at(),
        ar_settings::common_columns::last_updated_at(),
    ]),
    title_template: "{key}".to_string(),
    operations: SettingOperations::from(GuildTemplatesKVExecutor),
});

#[derive(Clone)]
pub struct GuildTemplatesKVExecutor;

#[async_trait::async_trait]
impl SettingView<SettingsData> for GuildTemplatesKVExecutor {
    async fn view<'a>(
        &self,
        context: &SettingsData,
        _filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        check_perms(context,"guild_templates_kv.view".into()).await?;

        #[derive(sqlx::FromRow)]
        struct GuildTemplatesKVRow {
            scope: String,
            key: String,
            value: Option<Value>,
            created_at: chrono::DateTime<chrono::Utc>,
            last_updated_at: chrono::DateTime<chrono::Utc>,
        }

        let rows: Vec<GuildTemplatesKVRow> = sqlx::query_as("SELECT scope, key, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1")
        .bind(context.scope.guild_id()?.to_string())
        .fetch_all(&context.data.pool)
        .await
        .map_err(|e| format!("Error while fetching guild templates kv: {}", e))?;

        let mut result = vec![];

        for row in rows {
            let map = indexmap::indexmap! {
                "guild_id".to_string() => Value::String(context.scope.guild_id()?.to_string()),
                "scope".to_string() => Value::String(row.scope),
                "key".to_string() => Value::String(row.key),
                "value".to_string() => row.value.unwrap_or(Value::Null),
                "created_at".to_string() => Value::String(row.created_at.to_string()),
                "last_updated_at".to_string() => Value::String(row.last_updated_at.to_string()),
            };

            result.push(map);
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
impl SettingCreator<SettingsData> for GuildTemplatesKVExecutor {
    async fn create<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates_kv.create".into()).await?;

        let Some(Value::String(key)) = entry.get("key") else {
            return Err("Missing or invalid field: `key`".into());
        };

        let Some(Value::String(scope)) = entry.get("scope") else {
            return Err("Missing or invalid field: `scope`".into());
        };

        let count = sqlx::query(
            "SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(key)
        .bind(scope)
        .fetch_one(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to check if kv exists: {:?}", e))?
        .try_get::<Option<i64>, _>(0)
        .map_err(|e| format!("Failed to determine if key already exists: {:?}", e))?
        .unwrap_or_default();

        if count > 0 {
            return Err("Key already exists in key-value DB".into());
        }

        let Some(value) = entry.get("value") else {
            return Err("Missing or invalid field: `value`".into());
        };

        sqlx::query(
            "INSERT INTO guild_templates_kv (guild_id, key, value, scope, created_at, last_updated_at) VALUES ($1, $2, $3, $4, NOW(), NOW())",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(key)
        .bind(value)
        .bind(scope)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to insert kv: {:?}", e))?;

        // Dispatch a ExternalKeyUpdate event for the template
        let ce = crate::dispatch::parse_event(&AntiraidEvent::ExternalKeyUpdate(ExternalKeyUpdateEventData {
            key_modified: key.to_string(),
            author: ctx.scope.user_id()?,
            action: ExternalKeyUpdateEventDataAction::Create
        }))?;

        crate::dispatch::dispatch(
            &ctx.serenity_context, 
            &ctx.data, 
            ce, 
            ctx.scope.guild_id()?)
            .await
            .map_err(|e| format!("Failed to dispatch ExternalKeyUpdate event: {:?}", e))?;

        Ok(entry)
    }
}

#[async_trait::async_trait]
impl SettingUpdater<SettingsData> for GuildTemplatesKVExecutor {
    async fn update<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates_kv.update".into()).await?;

        let Some(Value::String(key)) = entry.get("key") else {
            return Err("Missing or invalid field: `key`".into());
        };

        let Some(Value::String(scope)) = entry.get("scope") else {
            return Err("Missing or invalid field: `scope`".into());
        };

        let Some(value) = entry.get("value") else {
            return Err("Missing or invalid field: `value`".into());
        };

        sqlx::query(
            "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW() WHERE guild_id = $2 AND key = $3 AND scope = $4",
        )
        .bind(value)
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(key)
        .bind(scope)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to update kv: {:?}", e))?;

    // Dispatch a ExternalKeyUpdate event for the template
    let ce = crate::dispatch::parse_event(&AntiraidEvent::ExternalKeyUpdate(ExternalKeyUpdateEventData {
        key_modified: key.to_string(),
        author: ctx.scope.user_id()?,
        action: ExternalKeyUpdateEventDataAction::Update
    }))?;

        crate::dispatch::dispatch(
            &ctx.serenity_context, 
            &ctx.data, 
            ce, 
            ctx.scope.guild_id()?)
            .await
            .map_err(|e| format!("Failed to dispatch ExternalKeyUpdate event: {:?}", e))?;

        Ok(entry)
    }
}

#[async_trait::async_trait]
impl SettingDeleter<SettingsData> for GuildTemplatesKVExecutor {
    async fn delete<'a>(
        &self,
        ctx: &SettingsData,
        mut fields: indexmap::IndexMap<String, Value>,
    ) -> Result<(), Error> {
        check_perms(ctx, "guild_templates_kv.delete".into()).await?;

        let Some(Value::String(primary_key)) = fields.swap_remove("key") else {
            return Err("Invalid primary key".into());
        };

        let Some(Value::String(scope)) = fields.swap_remove("scope") else {
            return Err("Invalid scope".into());
        };

        if sqlx::query(
            "SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(&primary_key)
        .bind(&scope)
        .fetch_one(&ctx.data.pool)
        .await
        .map_err(|e| format!("Error while fetching kv: {}", e))?
        .try_get::<Option<i64>, _>(0)
        .map_err(|e| format!("Failed to get count: {:?}", e))?
        .unwrap_or_default()
            <= 0
        {
            return Err("Row requested to be deleted does not exist".into());
        };

        sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(&primary_key)
        .bind(&scope)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to delete kv: {:?}", e))?;

        // Dispatch a ExternalKeyUpdate event for the template
        let ce = crate::dispatch::parse_event(&AntiraidEvent::ExternalKeyUpdate(ExternalKeyUpdateEventData {
            key_modified: primary_key,
            author: ctx.scope.user_id()?,
            action: ExternalKeyUpdateEventDataAction::Delete
        }))?;

        crate::dispatch::dispatch(
            &ctx.serenity_context, 
            &ctx.data, 
            ce, 
            ctx.scope.guild_id()?)
            .await
            .map_err(|e| format!("Failed to dispatch ExternalKeyUpdate event: {:?}", e))?;

        Ok(())
    }
}

pub static GUILD_TEMPLATE_SHOP: LazyLock<Setting<SettingsData>> = LazyLock::new(|| {
    Setting {
        id: "script_shop".to_string(),
        name: "Created/Published Scripts".to_string(),
        description: "Publish new scripts to the shop that can be used by any other server".to_string(),
        columns: settings_wrap(vec![
            Column {
                id: "id".to_string(),
                name: "ID".to_string(),
                description: "The internal ID of the script".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    min_length: Some(30),
                    max_length: Some(64),
                    allowed_values: vec![],
                    kind: "uuid".to_string(),
                }),
                primary_key: true,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Create],
                secret: false,
            },
            Column {
                id: "name".to_string(),
                name: "Name".to_string(),
                description: "The name of the script on the shop. Cannot be updated once set".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Update],
                secret: false,
            },
            Column {
                id: "friendly_name".to_string(),
                name: "Friendly Name".to_string(),
                description: "The friendly name of the script on the shop.".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "language".to_string(),
                name: "Language".to_string(),
                description: "The language of the script. Only Roblox Luau is currently supported here. Cannot be updated once set".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec!["luau".to_string()],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Update],
                secret: false,
            },
            Column {
                id: "version".to_string(),
                name: "Version".to_string(),
                description: "The version of the template. Cannot be updated once set".to_string(), 
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Update],
                secret: false,
            },
            Column {
                id: "description".to_string(),
                name: "Description".to_string(),
                description: "The description of the script".to_string(), 
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(4096),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "content".to_string(),
                name: "Content".to_string(),
                description: "The content of the script.".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::Json {
                    kind: "template".to_string(),
                    max_bytes: Some(1024 * 1024 * 5), // 5MB
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "events".to_string(),
                name: "Events".to_string(),
                description: "The events that this script should be executed on.".to_string(),
                column_type: ColumnType::new_array(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: None,
                    allowed_values: {
                        let mut vec = AntiraidEvent::variant_names()
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                        
                        vec.extend(gwevent::core::event_list().iter().copied().map(|x| x.to_string()).collect::<Vec<String>>());

                        vec
                    },
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "allowed_caps".to_string(),
                name: "Capabilities".to_string(),
                description: "The capabilities the script needs to perform its full functionality.".to_string(),
                column_type: ColumnType::new_array(InnerColumnType::String { min_length: None, max_length: None, allowed_values: vec![], kind: "normal".to_string() }),
                primary_key: false,
                nullable: true,
                suggestions: ColumnSuggestion::Static {
                    suggestions: vec![
                        "discord:create_message".to_string()
                    ]
                },
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "type".to_string(),
                name: "Type".to_string(),
                description: "The type of the script".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: None,
                    allowed_values: vec!["public".to_string(), "hidden".to_string()],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            ar_settings::common_columns::guild_id("owner_guild", "Guild ID", "The Guild ID"),
            ar_settings::common_columns::created_at(),
            ar_settings::common_columns::created_by(),
            ar_settings::common_columns::last_updated_at(),
            ar_settings::common_columns::last_updated_by(),
        ]),
        title_template: "{name}#{version}".to_string(),
        operations: SettingOperations::from(GuildTemplateShopExecutor),
    }
});

#[derive(Clone)]
pub struct GuildTemplateShopExecutor;

#[async_trait::async_trait]
impl SettingView<SettingsData> for GuildTemplateShopExecutor {
    async fn view<'a>(
        &self,
        context: &SettingsData,
        _filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        check_perms(context,"guild_templates_shop.view".into()).await?;

        #[derive(sqlx::FromRow)]
        struct GuildTemplateShopRow {
            id: uuid::Uuid,
            name: String,
            friendly_name: String,
            language: String,
            allowed_caps: Vec<String>,
            version: String,
            description: String,
            content: serde_json::Value,
            r#type: String,
            events: Vec<String>,
            created_at: chrono::DateTime<chrono::Utc>,
            created_by: String,
            last_updated_at: chrono::DateTime<chrono::Utc>,
            last_updated_by: String,
        }

        let rows: Vec<GuildTemplateShopRow> = sqlx::query_as("SELECT id, name, friendly_name, language, allowed_caps, version, description, content, type, events, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE owner_guild = $1")
        .bind(context.scope.guild_id()?.to_string())
        .fetch_all(&context.data.pool)
        .await
        .map_err(|e| format!("Error while fetching shop templates: {}", e))?;

        let mut result = vec![];

        for row in rows {
            let map = indexmap::indexmap! {
                "id".to_string() => Value::String(row.id.to_string()),
                "name".to_string() => Value::String(row.name),
                "friendly_name".to_string() => Value::String(row.friendly_name),
                "language".to_string() => Value::String(row.language),
                "allowed_caps".to_string() => {
                    Value::Array(row.allowed_caps.iter().map(|x| Value::String(x.to_string())).collect())
                },
                "version".to_string() => Value::String(row.version),
                "description".to_string() => Value::String(row.description),
                "type".to_string() => Value::String(row.r#type),
                "content".to_string() => row.content,
                "events".to_string() => {
                    Value::Array(row.events.iter().map(|x| Value::String(x.to_string())).collect())
                },
                "owner_guild".to_string() => Value::String(context.scope.guild_id()?.to_string()),
                "created_at".to_string() => Value::String(row.created_at.to_string()),
                "created_by".to_string() => Value::String(row.created_by),
                "last_updated_at".to_string() => Value::String(row.last_updated_at.to_string()),
                "last_updated_by".to_string() => Value::String(row.last_updated_by),
            };

            result.push(map);
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
impl SettingCreator<SettingsData> for GuildTemplateShopExecutor {
    async fn create<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates_shop.create".into()).await?;

        let Some(Value::String(name)) = entry.get("name") else {
            return Err("Missing or invalid field: `name`".into());
        };

        // Rules for name:
        // Only namespaced templates can contain @ or /
        // Namespaced templates must use a namespace owned by the server
        // Namespaced templates must be in the format @namespace/<pkgname>. <pkgname> itself cannot contain '@' but may use '/'

        if !name.is_ascii() {
            return Err("Name must be ASCII".into());
        }

        if name.starts_with('@') {
            // This is a namespaced template, check that the server owns the namespace
            if !name.contains('/') {
                return Err("Please contact support to claim ownership over a specific namespace".into());
            }

            let namespace = name.split('/').next().unwrap();
            let pkgname = name.replace(&format!("{}{}", namespace, "/"), "");

            if pkgname.contains("@") {
                return Err("Package name cannot contain '@'".into());
            }

            let count = sqlx::query(
                "SELECT COUNT(*) FROM template_shop WHERE owner_guild = $1 AND name = $2",
            )
            .bind(ctx.scope.guild_id()?.to_string())
            .bind(namespace)
            .fetch_one(&ctx.data.pool)
            .await
            .map_err(|e| format!("Failed to check if namespace exists: {:?}", e))?
            .try_get::<Option<i64>, _>(0)
            .map_err(|e| format!("Failed to get count: {:?}", e))?
            .unwrap_or_default();

            if count <= 0 {
                return Err("Namespace does not exist. Please contact support".into());
            }
        } else if name.contains('@') || name.contains('/') {
            return Err("Name cannot contain '@' or '/' unless it is a namespace".into());
        }

        let Some(Value::String(friendly_name)) = entry.get("friendly_name") else {
            return Err("Missing or invalid field: `friendly_name`".into());
        };

        let Some(Value::String(language)) = entry.get("language") else {
            return Err("Missing or invalid field: `language`".into());
        };

        let Some(Value::String(version)) = entry.get("version") else {
            return Err("Missing or invalid field: `version`".into());
        };

        if version == "latest" {
            return Err("Version cannot be 'latest'".into());
        }

        let count = sqlx::query(
            "SELECT COUNT(*) FROM template_shop WHERE owner_guild = $1 AND name = $2 AND version = $3",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(name)
        .bind(version)
        .fetch_one(&ctx.data.pool)
        .await
        .map_err(|e| 
            format!("Failed to check if shop template exists: {:?}", e)
        )?
        .try_get::<Option<i64>, _>(0)
        .map_err(|e| format!("Failed to get template shop count: {:?}", e))?
        .unwrap_or_default();

        if count > 0 {
            return Err("Shop template with this name and version already exists".into());
        }

        let Some(Value::String(description)) = entry.get("description") else {
            return Err("Missing or invalid field: `description`".into());
        };

        let Some(content) = entry.get("content") else {
            return Err("Missing or invalid field: `content`".into());
        };

        // Try to parse content as a hashmap<String, String>
        let string_form = serde_json::to_string(&content)
            .map_err(|e| format!("Failed to convert content to string: {:?}", e))?;

        let _: indexmap::IndexMap<String, Value> = serde_json::from_str(&string_form)   
            .map_err(|e| format!("Failed to parse content: {:?}", e))?;     

        let Some(Value::String(r#type)) = entry.get("type") else {
            return Err("Missing or invalid field: `type`".into());
        };

        let events = match entry.get("events") {
            Some(Value::Array(events)) => 
                events
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err("Failed to parse events".into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let allowed_caps = match entry.get("allowed_caps") {
            Some(Value::Array(allowed_caps)) => 
                allowed_caps
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err(format!("Failed to parse allowed capabilities due to invalid capability: {:?}", x).into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let id = sqlx::query(
            "INSERT INTO template_shop (name, friendly_name, language, version, description, content, type, events, owner_guild, created_by, last_updated_by, allowed_caps) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
        )
        .bind(name)
        .bind(friendly_name)
        .bind(language)
        .bind(version)
        .bind(description)
        .bind(content)
        .bind(r#type)
        .bind(&events)
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(ctx.scope.user_id()?.to_string())
        .bind(ctx.scope.user_id()?.to_string())
        .bind(&allowed_caps)
        .fetch_one(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to insert shop template: {:?}", e))?;

        let id: uuid::Uuid = id.try_get(0).map_err(|e| format!("Failed to get ID of created setting: {:?}", e))?;

        // Add returned ID to entry
        let mut entry = entry;
        entry.insert("id".to_string(), Value::String(id.to_string()));

        Ok(entry)
    }
}

#[async_trait::async_trait]
impl SettingUpdater<SettingsData> for GuildTemplateShopExecutor {
    async fn update<'a>(
        &self,
        ctx: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(ctx, "guild_templates_shop.update".into()).await?;

        let Some(Value::String(id)) = entry.get("id") else {
            return Err("Missing or invalid field: `id`".into());
        };

        let id: uuid::Uuid = id.parse().map_err(|e| format!("Failed to parse ID: {:?}", e))?;

        #[derive(sqlx::FromRow)]
        pub struct TemplateShopData {
            pub name: String,
            pub version: String,
        }

        let data: TemplateShopData = sqlx::query_as(
            "SELECT name, version FROM template_shop WHERE owner_guild = $1 AND id = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(id)
        .fetch_optional(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to check if shop template exists: {:?}", e))?
        .ok_or_else(|| "Shop template does not exist".to_string())?;

        let Some(Value::String(friendly_name)) = entry.get("friendly_name") else {
            return Err("Missing or invalid field: `friendly_name`".into());
        };

        let Some(Value::String(description)) = entry.get("description") else {
            return Err("Missing or invalid field: `description`".into());
        };

        let Some(Value::String(typ)) = entry.get("type") else {
            return Err("Missing or invalid field: `type`".into());
        };

        let Some(content) = entry.get("content") else {
            return Err("Missing or invalid field: `content`".into());
        };

        // Try to parse content as a hashmap<String, String>
        let string_form = serde_json::to_string(&content)
            .map_err(|e| format!("Failed to convert content to string: {:?}", e))?;

        let _: indexmap::IndexMap<String, Value> = serde_json::from_str(&string_form)   
            .map_err(|e| format!("Failed to parse content: {:?}", e))?;     

        let events = match entry.get("events") {
            Some(Value::Array(events)) => 
                events
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err("Failed to parse events".into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        let allowed_caps = match entry.get("allowed_caps") {
            Some(Value::Array(allowed_caps)) => 
                allowed_caps
                    .iter()
                    .map(|x| {
                        if let Value::String(x) = x {
                            Ok(x.to_string())
                        } else {
                            Err(format!("Failed to parse allowed capabilities due to invalid capability: {:?}", x).into())
                        }
                    })
                    .collect::<Result<Vec<String>, Error>>()?,
            _ => {
                vec![]
            },
        };

        sqlx::query(
            "UPDATE template_shop SET description = $1, type = $2, friendly_name = $3, last_updated_at = NOW(), last_updated_by = $4, events = $7, allowed_caps = $8, content = $9 WHERE owner_guild = $5 AND id = $6",
        )
        .bind(description)
        .bind(typ)
        .bind(friendly_name)
        .bind(ctx.scope.user_id()?.to_string())
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(id)
        .bind(&events)
        .bind(&allowed_caps)
        .bind(content)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to update shop template: {:?}", e))?;

        #[derive(sqlx::FromRow)]
        struct GuildTemplateShopGuildRow {
            guild_id: String,
        }

        // Find all guilds with this template and dispatch an OnStartup event for all of them
        let guilds: Vec<GuildTemplateShopGuildRow> = sqlx::query_as(
            "SELECT guild_id FROM guild_templates WHERE name = $1 AND paused = false",
        )
        .bind(
            Template::create_shop_template(
                &data.name,
                &data.version,
            )
        )
        .fetch_all(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to fetch guilds with this template: {:?}", e))?;

        let mut other_affected_guilds = Vec::with_capacity(guilds.len());
        for guild in &guilds {
            let guild_id = match guild.guild_id.parse::<serenity::all::GuildId>() {
                Ok(guild_id) => guild_id,
                Err(e) => {
                    log::error!("Failed to parse guild id: {:?}", e);
                    continue;
                }
            };

            other_affected_guilds.push(guild_id);
        }

        DEFERRED_CACHE_REGENS.insert(
            ctx.scope.guild_id()?,
            DeferredCacheRegenMode::FlushMultiple { other_guilds: other_affected_guilds, flush_self: false },
        ).await;  

        Ok(entry)
    }
}

#[async_trait::async_trait]
impl SettingDeleter<SettingsData> for GuildTemplateShopExecutor {
    async fn delete<'a>(
        &self,
        ctx: &SettingsData,
        mut fields: indexmap::IndexMap<String, Value>,
    ) -> Result<(), Error> {
        check_perms(ctx, "guild_templates_shop.delete".into()).await?;

        let Some(Value::String(primary_key)) = fields.swap_remove("id") else {
            return Err("Missing or invalid field: `id`".into());
        };

        let primary_key = primary_key.parse::<uuid::Uuid>().map_err(|e| format!("Failed to parse ID: {:?}", e))?;

        #[derive(sqlx::FromRow)]
        struct GuildTemplateShopRow {
            id: uuid::Uuid,
            name: String,
            version: String,
        }

        let row: GuildTemplateShopRow = sqlx::query_as(
            "SELECT id, name, version FROM template_shop WHERE owner_guild = $1 AND id = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(primary_key)
        .fetch_optional(&ctx.data.pool)
        .await
        .map_err(|e| format!("Error while fetching shop template: {}", e))?
        .ok_or_else(|| "Shop template not found when trying to delete it!".to_string())?;

        sqlx::query(
            "DELETE FROM template_shop WHERE owner_guild = $1 AND id = $2",
        )
        .bind(ctx.scope.guild_id()?.to_string())
        .bind(row.id)
        .execute(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to delete shop template: {:?}", e))?;

        // Dispatch a OnStartup event for the template
        #[derive(sqlx::FromRow)]
        struct GuildTemplateShopGuildRow {
            guild_id: String,
        }

        // Find all guilds with this template and dispatch an OnStartup event for all of them
        let guilds: Vec<GuildTemplateShopGuildRow> = sqlx::query_as(
            "SELECT guild_id FROM guild_templates WHERE name = $1 AND paused = false",
        )
        .bind(
            Template::create_shop_template(
                &row.name,
                &row.version,
            )
        )
        .fetch_all(&ctx.data.pool)
        .await
        .map_err(|e| format!("Failed to fetch guilds with this template: {:?}", e))?;

        let mut other_affected_guilds = Vec::with_capacity(guilds.len());
        for guild in &guilds {
            let guild_id = match guild.guild_id.parse::<serenity::all::GuildId>() {
                Ok(guild_id) => guild_id,
                Err(e) => {
                    log::error!("Failed to parse guild id: {:?}", e);
                    continue;
                }
            };

            other_affected_guilds.push(guild_id);
        }

        DEFERRED_CACHE_REGENS.insert(
            ctx.scope.guild_id()?,
            DeferredCacheRegenMode::FlushMultiple { other_guilds: other_affected_guilds, flush_self: false },
        ).await;  

        Ok(())
    }
}

pub static GUILD_TEMPLATE_SHOP_PUBLIC_LIST: LazyLock<Setting<SettingsData>> = LazyLock::new(|| {
    Setting {
        id: "template_shop_public_list".to_string(),
        name: "Explore the shop!".to_string(),
        description: "Explore other templates published by other servers".to_string(),
        columns: settings_wrap(vec![
            Column {
                id: "id".to_string(),
                name: "ID".to_string(),
                description: "The internal ID of the template".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    min_length: Some(30),
                    max_length: Some(64),
                    allowed_values: vec![],
                    kind: "uuid".to_string(),
                }),
                primary_key: true,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "name".to_string(),
                name: "Name".to_string(),
                description: "The name of the template on the shop. Cannot be updated once set".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Update],
                secret: false,
            },
            Column {
                id: "version".to_string(),
                name: "Version".to_string(),
                description: "The version of the template. Cannot be updated once set".to_string(), 
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(64),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![OperationType::Update],
                secret: false,
            },
            Column {
                id: "description".to_string(),
                name: "Description".to_string(),
                description: "The description of the template".to_string(), 
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: Some(4096),
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "type".to_string(),
                name: "Type".to_string(),
                description: "The type of the template".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::String {
                    kind: "normal".to_string(),
                    min_length: None,
                    max_length: None,
                    allowed_values: vec!["public".to_string(), "hidden".to_string()],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            ar_settings::common_columns::guild_id("owner_guild", "Guild ID", "The ID of the server which owns the templaye"),
            ar_settings::common_columns::created_at(),
            ar_settings::common_columns::created_by(),
            ar_settings::common_columns::last_updated_at(),
            ar_settings::common_columns::last_updated_by(),
        ]),
        title_template: "{name}".to_string(),
        operations: SettingOperations::to_view_op(GuildTemplateShopPublicListExecutor),
    }
});

#[derive(Clone)]
pub struct GuildTemplateShopPublicListExecutor;

#[async_trait::async_trait]
impl SettingView<SettingsData> for GuildTemplateShopPublicListExecutor {
    // Note: can be used anonymously
    async fn view<'a>(
        &self,
        context: &SettingsData,
        _filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        #[derive(sqlx::FromRow)]
        struct GuildTemplateShopRow {
            id: uuid::Uuid,
            name: String,
            version: String,
            description: String,
            r#type: String,
            owner_guild: String,
            created_at: chrono::DateTime<chrono::Utc>,
            created_by: String,
            last_updated_at: chrono::DateTime<chrono::Utc>,
            last_updated_by: String,
        }

        let rows: Vec<GuildTemplateShopRow> = sqlx::query_as("SELECT id, name, version, description, type, owner_guild, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE type = 'public'")
        .fetch_all(&context.data.pool)
        .await
        .map_err(|e| format!("Error while fetching shop templates: {}", e))?;

        let mut result = vec![];

        for row in rows {
            let map = indexmap::indexmap! {
                "id".to_string() => Value::String(row.id.to_string()),
                "name".to_string() => Value::String(row.name),
                "version".to_string() => Value::String(row.version),
                "description".to_string() => Value::String(row.description),
                "type".to_string() => Value::String(row.r#type),
                "owner_guild".to_string() => Value::String(row.owner_guild),
                "created_at".to_string() => Value::String(row.created_at.to_string()),
                "created_by".to_string() => Value::String(row.created_by),
                "last_updated_at".to_string() => Value::String(row.last_updated_at.to_string()),
                "last_updated_by".to_string() => Value::String(row.last_updated_by),
            };

            result.push(map);
        }

        Ok(result)
    }
}

pub static LOCKDOWN_SETTINGS: LazyLock<Setting<SettingsData>> = LazyLock::new(|| {
    let mut gid_col = ar_settings::common_columns::guild_id(
        "guild_id",
        "Guild ID",
        "Guild ID of the server in question",
    );

    gid_col.primary_key = true;

    Setting {
        id: "lockdown_guilds".to_string(),
        name: "Lockdown Settings".to_string(),
        description: "Setup standard lockdown settings for a server".to_string(),
        columns: settings_wrap(vec![
            gid_col,
            Column {
                id: "member_roles".to_string(),
                name: "Member Roles".to_string(),
                description: "Which roles to use as member roles for the purpose of lockdown. These roles will be explicitly modified during lockdown".to_string(),
                column_type: ColumnType::new_array(InnerColumnType::String {
                    kind: "role".to_string(),
                    min_length: None,
                    max_length: None,
                    allowed_values: vec![],
                }),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            Column {
                id: "require_correct_layout".to_string(),
                name: "Require Correct Layout".to_string(),
                description: "Whether or not a lockdown can proceed even without correct critical role permissions. May lead to partial lockdowns if disabled".to_string(),
                column_type: ColumnType::new_scalar(InnerColumnType::Boolean {}),
                primary_key: false,
                nullable: false,
                suggestions: ColumnSuggestion::None {},
                ignored_for: vec![],
                secret: false,
            },
            ar_settings::common_columns::created_at(),
            ar_settings::common_columns::created_by(),
            ar_settings::common_columns::last_updated_at(),
            ar_settings::common_columns::last_updated_by(),
        ]),
        title_template: "Lockdown Settings".to_string(),
        operations: SettingOperations::from(LockdownSettingsExecutor),
    }
});

#[derive(Clone)]
pub struct LockdownSettingsExecutor;

#[async_trait]
impl SettingView<SettingsData> for LockdownSettingsExecutor {
    async fn view<'a>(
        &self,
        context: &SettingsData,
        _filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        check_perms(context,"lockdown_settings.view".into()).await?;

        #[derive(sqlx::FromRow)]
        struct LockdownRow {
            member_roles: Vec<String>,
            require_correct_layout: bool,
            created_at: chrono::DateTime<chrono::Utc>,
            created_by: String,
            last_updated_at: chrono::DateTime<chrono::Utc>,
            last_updated_by: String,
        }

        let rows: Vec<LockdownRow> = sqlx::query_as("SELECT member_roles, require_correct_layout, created_at, created_by, last_updated_at, last_updated_by FROM lockdown__guilds WHERE guild_id = $1")
            .bind(context.scope.guild_id()?.to_string())
            .fetch_all(&context.data.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdowns: {}", e))?;

        let mut result = vec![];

        for row in rows {
            let map = indexmap::indexmap! {
                "guild_id".to_string() => Value::String(context.scope.guild_id()?.to_string()),
                "member_roles".to_string() => Value::Array(row.member_roles.into_iter().map(Value::String).collect()),
                "require_correct_layout".to_string() => Value::Bool(row.require_correct_layout),
                "created_at".to_string() => Value::String(row.created_at.to_string()),
                "created_by".to_string() => Value::String(row.created_by),
                "last_updated_at".to_string() => Value::String(row.last_updated_at.to_string()),
                "last_updated_by".to_string() => Value::String(row.last_updated_by),
            };

            result.push(map);
        }
        
        Ok(result)
    }
}

#[async_trait]
impl SettingCreator<SettingsData> for LockdownSettingsExecutor {
    async fn create<'a>(
        &self,
        context: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(context,"lockdown_settings.create".into()).await?;

        let Some(Value::Array(member_roles)) = entry.get("member_roles") else {
            return Err("Missing or invalid field: `member_roles`".into());
        };

        let member_roles: Vec<String> = member_roles.iter().map(|v| match v {
            Value::String(s) => Ok(s.clone()),
            _ => Err("Invalid member role".into()),
        }).collect::<Result<Vec<String>, Error>>()?;
        
        let Some(Value::Bool(require_correct_layout)) = entry.get("require_correct_layout") else {
            return Err("Missing or invalid field: `require_correct_layout`".into());
        };

        sqlx::query(
            "INSERT INTO lockdown__guilds (guild_id, member_roles, require_correct_layout, created_at, created_by, last_updated_at, last_updated_by) VALUES ($1, $2, $3, NOW(), $4, NOW(), $5)",
        )
        .bind(context.scope.guild_id()?.to_string())
        .bind(&member_roles)
        .bind(require_correct_layout)
        .bind(context.scope.user_id()?.to_string())
        .bind(context.scope.user_id()?.to_string())
        .execute(&context.data.pool)
        .await
        .map_err(|e| format!("Error while creating lockdown settings: {}", e))?;

        Ok(entry)
    }
}

#[async_trait]
impl SettingUpdater<SettingsData> for LockdownSettingsExecutor {
    async fn update<'a>(
        &self,
        context: &SettingsData,
        entry: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        check_perms(context,"lockdown_settings.uodate".into()).await?;

        let Some(Value::Array(member_roles)) = entry.get("member_roles") else {
            return Err("Missing or invalid field: `member_roles`".into());
        };

        let member_roles: Vec<String> = member_roles.iter().map(|v| match v {
            Value::String(s) => Ok(s.clone()),
            _ => Err("Invalid member role".into()),
        }).collect::<Result<Vec<String>, Error>>()?;
        
        let Some(Value::Bool(require_correct_layout)) = entry.get("require_correct_layout") else {
            return Err("Missing or invalid field: `require_correct_layout`".into());
        };

        let count = sqlx::query(
            "SELECT COUNT(*) FROM lockdown__guilds WHERE guild_id = $1",
        )
        .bind(context.scope.guild_id()?.to_string())
        .fetch_one(&context.data.pool)
        .await
        .map_err(|e| format!("Error while updating lockdown settings: {}", e))?
        .try_get::<Option<i64>, _>(0)
        .map_err(|e| format!("Error while updating lockdown settings: {}", e))?
        .unwrap_or(0);

        if count == 0 {
            return Err("Lockdown settings not found".into());
        }

        sqlx::query(
            "UPDATE lockdown__guilds SET member_roles = $2, require_correct_layout = $3, last_updated_at = NOW(), last_updated_by = $4 WHERE guild_id = $1",
        )
        .bind(context.scope.guild_id()?.to_string())
        .bind(&member_roles)
        .bind(require_correct_layout)
        .bind(context.scope.user_id()?.to_string())
        .execute(&context.data.pool)
        .await
        .map_err(|e| format!("Error while creating lockdown settings: {}", e))?;

        Ok(entry)
    }
}

#[async_trait]
impl SettingDeleter<SettingsData> for LockdownSettingsExecutor {
    async fn delete<'a>(
        &self,
        context: &SettingsData,
        _fields: indexmap::IndexMap<String, Value>,
    ) -> Result<(), Error> {
        check_perms(context,"lockdown_settings.delete".into()).await?;

        sqlx::query("DELETE FROM lockdown__guilds WHERE guild_id = $1")
            .bind(context.scope.guild_id()?.to_string())
            .execute(&context.data.pool)
            .await
            .map_err(|e| format!("Error while deleting lockdown settings: {}", e))?;

        Ok(())
    }
}
