use crate::dispatch::{dispatch_and_wait, parse_event};
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use ar_settings::types::{Setting, SettingView};
use ar_settings::types::{SettingCreator, SettingDeleter, SettingUpdater};
use scc::HashMap;
use serde::Serialize;
use serde_json::Value;
use serenity::all::{GuildId, UserId};
use silverpelt::ar_event::TemplateSettingExecuteEventDataAction;
use silverpelt::data::Data;
use silverpelt::Error;
use std::collections::HashMap as StdHashMap;
use std::sync::{Arc, LazyLock};

#[derive(Serialize)]
pub struct Page {
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub settings: Vec<Setting<SettingExecutionData>>,
}

#[derive(Clone)]
pub struct SettingExecutionData {
    pub data: Arc<Data>,
    pub serenity_context: serenity::all::Context,
    pub author: UserId,
}

impl serde::Serialize for SettingExecutionData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.author.serialize(serializer) // This is a dummy impl to satisfy the compiler
    }
}

impl SettingExecutionData {
    pub fn new(data: Arc<Data>, serenity_context: serenity::all::Context, author: UserId) -> Self {
        Self {
            data,
            serenity_context,
            author,
        }
    }
}

#[derive(Clone, Default)]
pub struct TemplateSettingExecutor {
    pub guild_id: GuildId,
    pub template_id: String,
    pub setting_id: String,
}

impl TemplateSettingExecutor {
    fn find_result(
        correlation_id: uuid::Uuid,
        results: Vec<Value>,
    ) -> Option<indexmap::IndexMap<String, Value>> {
        for result in results {
            if let Value::Object(mut map) = result {
                if let Some(Value::String(id)) = map.get("correlation_id") {
                    if *id == correlation_id.to_string() {
                        // We found the result that should correspond to our request
                        if let Some(Value::Object(result)) = map.remove("result") {
                            let mut values = indexmap::IndexMap::new();
                            for (key, value) in result {
                                values.insert(key, value);
                            }

                            return Some(values);
                        }
                    }
                }
            }
        }

        None
    }

    fn find_results(
        correlation_id: uuid::Uuid,
        results: Vec<Value>,
    ) -> Option<Vec<indexmap::IndexMap<String, Value>>> {
        for result in results {
            if let Value::Object(mut map) = result {
                if let Some(Value::String(id)) = map.get("correlation_id") {
                    if *id == correlation_id.to_string() {
                        // We found the result that should correspond to our request
                        if let Some(Value::Array(results)) = map.remove("results") {
                            let mut values_list = Vec::new();
                            for value in results {
                                if let serde_json::Value::Object(result) = value {
                                    let mut values = indexmap::IndexMap::new();
                                    for (key, value) in result {
                                        values.insert(key, value);
                                    }
                                    values_list.push(values);
                                }
                            }

                            return Some(values_list);
                        }
                    }
                }
            }
        }

        None
    }

    fn find_correlation(correlation_id: uuid::Uuid, results: Vec<Value>) -> Option<()> {
        for result in results {
            if let Value::Object(map) = result {
                if let Some(Value::String(id)) = map.get("correlation_id") {
                    if *id == correlation_id.to_string() {
                        return Some(());
                    }
                }
            }
        }

        None
    }
}

#[async_trait::async_trait]
impl SettingView<SettingExecutionData> for TemplateSettingExecutor {
    async fn view<'a>(
        &self,
        context: &SettingExecutionData,
        filters: indexmap::IndexMap<String, Value>,
    ) -> Result<Vec<indexmap::IndexMap<String, Value>>, Error> {
        let correlation_id = uuid::Uuid::new_v4();
        let event = silverpelt::ar_event::AntiraidEvent::TemplateSettingExecute(
            silverpelt::ar_event::TemplateSettingExecuteEventData {
                template_id: self.template_id.clone(),
                setting_id: self.setting_id.clone(),
                action: TemplateSettingExecuteEventDataAction::View { filters },
                author: context.author,
                correlation_id,
            },
        );

        let create_event =
            parse_event(&event).map_err(|e| format!("Failed to send event to template: {}", e))?;

        let result = dispatch_and_wait(
            &context.serenity_context,
            &context.data,
            create_event,
            self.guild_id,
            MAX_TEMPLATES_RETURN_WAIT_TIME,
        )
        .await
        .map_err(|e| format!("Failed to wait for template: {}", e))?;

        if let Some(value) = TemplateSettingExecutor::find_results(correlation_id, result) {
            Ok(value)
        } else {
            Ok(vec![])
        }
    }
}

#[async_trait::async_trait]
impl SettingCreator<SettingExecutionData> for TemplateSettingExecutor {
    async fn create<'a>(
        &self,
        context: &SettingExecutionData,
        fields: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        let correlation_id = uuid::Uuid::new_v4();
        let event = silverpelt::ar_event::AntiraidEvent::TemplateSettingExecute(
            silverpelt::ar_event::TemplateSettingExecuteEventData {
                template_id: self.template_id.clone(),
                setting_id: self.setting_id.clone(),
                action: TemplateSettingExecuteEventDataAction::Create { fields },
                author: context.author,
                correlation_id,
            },
        );

        let create_event =
            parse_event(&event).map_err(|e| format!("Failed to send event to template: {}", e))?;

        let result = dispatch_and_wait(
            &context.serenity_context,
            &context.data,
            create_event,
            self.guild_id,
            MAX_TEMPLATES_RETURN_WAIT_TIME,
        )
        .await
        .map_err(|e| format!("Failed to wait for template: {}", e))?;

        if let Some(value) = TemplateSettingExecutor::find_result(correlation_id, result) {
            Ok(value)
        } else {
            Ok(indexmap::IndexMap::new())
        }
    }
}

#[async_trait::async_trait]
impl SettingUpdater<SettingExecutionData> for TemplateSettingExecutor {
    async fn update<'a>(
        &self,
        context: &SettingExecutionData,
        fields: indexmap::IndexMap<String, Value>,
    ) -> Result<indexmap::IndexMap<String, Value>, Error> {
        let correlation_id = uuid::Uuid::new_v4();
        let event = silverpelt::ar_event::AntiraidEvent::TemplateSettingExecute(
            silverpelt::ar_event::TemplateSettingExecuteEventData {
                template_id: self.template_id.clone(),
                setting_id: self.setting_id.clone(),
                action: TemplateSettingExecuteEventDataAction::Update { fields },
                author: context.author,
                correlation_id,
            },
        );

        let create_event =
            parse_event(&event).map_err(|e| format!("Failed to send event to template: {}", e))?;

        let result = dispatch_and_wait(
            &context.serenity_context,
            &context.data,
            create_event,
            self.guild_id,
            MAX_TEMPLATES_RETURN_WAIT_TIME,
        )
        .await
        .map_err(|e| format!("Failed to wait for template: {}", e))?;

        if let Some(value) = TemplateSettingExecutor::find_result(correlation_id, result) {
            Ok(value)
        } else {
            Ok(indexmap::IndexMap::new())
        }
    }
}

#[async_trait::async_trait]
impl SettingDeleter<SettingExecutionData> for TemplateSettingExecutor {
    async fn delete<'a>(
        &self,
        context: &SettingExecutionData,
        primary_key: Value,
    ) -> Result<(), Error> {
        let correlation_id = uuid::Uuid::new_v4();
        let event = silverpelt::ar_event::AntiraidEvent::TemplateSettingExecute(
            silverpelt::ar_event::TemplateSettingExecuteEventData {
                template_id: self.template_id.clone(),
                setting_id: self.setting_id.clone(),
                action: TemplateSettingExecuteEventDataAction::Delete { primary_key },
                author: context.author,
                correlation_id,
            },
        );

        let create_event =
            parse_event(&event).map_err(|e| format!("Failed to send event to template: {}", e))?;

        let result = dispatch_and_wait(
            &context.serenity_context,
            &context.data,
            create_event,
            self.guild_id,
            MAX_TEMPLATES_RETURN_WAIT_TIME,
        )
        .await
        .map_err(|e| format!("Failed to wait for template: {}", e))?;

        if let Some(()) = TemplateSettingExecutor::find_correlation(correlation_id, result) {
            Ok(())
        } else {
            Err("Failed to delete result? (no correlation found!)".into())
        }
    }
}

#[allow(clippy::type_complexity)]
static PAGES: LazyLock<HashMap<GuildId, Arc<HashMap<String, Arc<Page>>>>> =
    LazyLock::new(HashMap::new);

pub async fn get_all_pages(guild_id: GuildId) -> Option<Vec<Arc<Page>>> {
    let pages = PAGES.read_async(&guild_id, |_, v| v.clone()).await?;

    let mut pages_set = StdHashMap::new();

    pages
        .scan_async(|_, page| {
            pages_set.insert(page.template_id.clone(), page.clone());
        })
        .await;

    let mut pages_list = Vec::with_capacity(pages_set.len());

    for (_, page) in pages_set {
        pages_list.push(page);
    }

    Some(pages_list)
}

pub async fn get_page_by_id(guild_id: GuildId, template_id: &str) -> Option<Arc<Page>> {
    if let Some(page) = PAGES.read_async(&guild_id, |_, v| v.clone()).await {
        if let Some(page) = page.read_async(template_id, |_, v| v.clone()).await {
            return Some(page.clone());
        }
    }

    None
}

pub async fn set_page(guild_id: GuildId, template_id: String, page: Arc<Page>) {
    match PAGES.get_async(&guild_id).await {
        Some(v) => {
            v.upsert_async(template_id.clone(), page).await;
        }
        None => {
            let map = HashMap::new();
            map.upsert_async(template_id.clone(), page).await;
            PAGES.upsert_async(guild_id, Arc::new(map)).await;
        }
    };
}

pub async fn remove_page(guild_id: GuildId, template_id: &str) {
    if let Some(v) = PAGES.get_async(&guild_id).await {
        v.remove_async(template_id).await;
    };
}

/// Given a khronos settings IR, the guild and template id, convert it to an AntiRaid ArSetting
fn create_setting(
    setting: khronos_runtime::traits::ir::Setting,
    guild_id: GuildId,
    template_id: String,
) -> ar_settings::types::Setting<SettingExecutionData> {
    fn _convert_inner_column_type(
        ict: khronos_runtime::traits::ir::InnerColumnType,
    ) -> ar_settings::types::InnerColumnType {
        match ict {
            khronos_runtime::traits::ir::InnerColumnType::String {
                min_length,
                max_length,
                allowed_values,
                kind,
            } => ar_settings::types::InnerColumnType::String {
                min_length,
                max_length,
                allowed_values,
                kind,
            },
            khronos_runtime::traits::ir::InnerColumnType::Integer {} => {
                ar_settings::types::InnerColumnType::Integer {}
            }
            khronos_runtime::traits::ir::InnerColumnType::Float {} => {
                ar_settings::types::InnerColumnType::Float {}
            }
            khronos_runtime::traits::ir::InnerColumnType::BitFlag { values } => {
                ar_settings::types::InnerColumnType::BitFlag { values }
            }
            khronos_runtime::traits::ir::InnerColumnType::Boolean {} => {
                ar_settings::types::InnerColumnType::Boolean {}
            }
            khronos_runtime::traits::ir::InnerColumnType::Json { max_bytes } => {
                ar_settings::types::InnerColumnType::Json { max_bytes }
            }
        }
    }

    let columns = setting
        .columns
        .into_iter()
        .map(|column| {
            let suggestions = match column.suggestions {
                khronos_runtime::traits::ir::ColumnSuggestion::None {} => {
                    ar_settings::types::ColumnSuggestion::None {}
                }
                khronos_runtime::traits::ir::ColumnSuggestion::Static { suggestions } => {
                    ar_settings::types::ColumnSuggestion::Static { suggestions }
                }
            };

            ar_settings::types::Column {
                id: column.id,
                name: column.name,
                description: column.description,
                column_type: match column.column_type {
                    khronos_runtime::traits::ir::ColumnType::Array { inner } => {
                        ar_settings::types::ColumnType::Array {
                            inner: _convert_inner_column_type(inner),
                        }
                    }
                    khronos_runtime::traits::ir::ColumnType::Scalar { inner } => {
                        ar_settings::types::ColumnType::Scalar {
                            inner: _convert_inner_column_type(inner),
                        }
                    }
                },
                nullable: column.nullable,
                suggestions,
                secret: column.secret,
                ignored_for: column
                    .ignored_for
                    .into_iter()
                    .map(|v| match v {
                        khronos_runtime::traits::ir::OperationType::View => {
                            ar_settings::types::OperationType::View
                        }
                        khronos_runtime::traits::ir::OperationType::Create => {
                            ar_settings::types::OperationType::Create
                        }
                        khronos_runtime::traits::ir::OperationType::Update => {
                            ar_settings::types::OperationType::Update
                        }
                        khronos_runtime::traits::ir::OperationType::Delete => {
                            ar_settings::types::OperationType::Delete
                        }
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    let operations = ar_settings::types::SettingOperations {
        view: {
            if setting.supported_operations.view {
                Some(Arc::new(TemplateSettingExecutor {
                    guild_id,
                    template_id: template_id.clone(),
                    setting_id: setting.id.clone(),
                }))
            } else {
                None
            }
        },
        create: {
            if setting.supported_operations.create {
                Some(Arc::new(TemplateSettingExecutor {
                    guild_id,
                    template_id: template_id.clone(),
                    setting_id: setting.id.clone(),
                }))
            } else {
                None
            }
        },
        update: {
            if setting.supported_operations.update {
                Some(Arc::new(TemplateSettingExecutor {
                    guild_id,
                    template_id: template_id.clone(),
                    setting_id: setting.id.clone(),
                }))
            } else {
                None
            }
        },
        delete: {
            if setting.supported_operations.delete {
                Some(Arc::new(TemplateSettingExecutor {
                    guild_id,
                    template_id: template_id.clone(),
                    setting_id: setting.id.clone(),
                }))
            } else {
                None
            }
        },
    };

    ar_settings::types::Setting {
        id: setting.id,
        name: setting.name,
        description: setting.description,
        primary_key: setting.primary_key,
        title_template: setting.title_template,
        columns: Arc::new(columns),
        operations,
    }
}

fn unravel_setting(
    setting: &ar_settings::types::Setting<SettingExecutionData>,
) -> khronos_runtime::traits::ir::Setting {
    fn _convert_inner_column_type(
        ict: ar_settings::types::InnerColumnType,
    ) -> khronos_runtime::traits::ir::InnerColumnType {
        match ict {
            ar_settings::types::InnerColumnType::String {
                min_length,
                max_length,
                allowed_values,
                kind,
            } => khronos_runtime::traits::ir::InnerColumnType::String {
                min_length,
                max_length,
                allowed_values,
                kind,
            },
            ar_settings::types::InnerColumnType::Integer {} => {
                khronos_runtime::traits::ir::InnerColumnType::Integer {}
            }
            ar_settings::types::InnerColumnType::Float {} => {
                khronos_runtime::traits::ir::InnerColumnType::Float {}
            }
            ar_settings::types::InnerColumnType::BitFlag { values } => {
                khronos_runtime::traits::ir::InnerColumnType::BitFlag { values }
            }
            ar_settings::types::InnerColumnType::Boolean {} => {
                khronos_runtime::traits::ir::InnerColumnType::Boolean {}
            }
            ar_settings::types::InnerColumnType::Json { max_bytes } => {
                khronos_runtime::traits::ir::InnerColumnType::Json { max_bytes }
            }
        }
    }

    let columns = setting
        .columns
        .iter()
        .map(|column| {
            let suggestions = match &column.suggestions {
                ar_settings::types::ColumnSuggestion::None {} => {
                    khronos_runtime::traits::ir::ColumnSuggestion::None {}
                }
                ar_settings::types::ColumnSuggestion::Static { suggestions } => {
                    khronos_runtime::traits::ir::ColumnSuggestion::Static {
                        suggestions: suggestions.clone(),
                    }
                }
            };

            khronos_runtime::traits::ir::Column {
                id: column.id.clone(),
                name: column.name.clone(),
                description: column.description.clone(),
                column_type: match &column.column_type {
                    ar_settings::types::ColumnType::Array { inner } => {
                        khronos_runtime::traits::ir::ColumnType::Array {
                            inner: _convert_inner_column_type(inner.clone()),
                        }
                    }
                    ar_settings::types::ColumnType::Scalar { inner } => {
                        khronos_runtime::traits::ir::ColumnType::Scalar {
                            inner: _convert_inner_column_type(inner.clone()),
                        }
                    }
                },
                nullable: column.nullable,
                suggestions,
                secret: column.secret,
                ignored_for: column
                    .ignored_for
                    .iter()
                    .map(|v| match v {
                        ar_settings::types::OperationType::View => {
                            khronos_runtime::traits::ir::OperationType::View
                        }
                        ar_settings::types::OperationType::Create => {
                            khronos_runtime::traits::ir::OperationType::Create
                        }
                        ar_settings::types::OperationType::Update => {
                            khronos_runtime::traits::ir::OperationType::Update
                        }
                        ar_settings::types::OperationType::Delete => {
                            khronos_runtime::traits::ir::OperationType::Delete
                        }
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    let supported_operations = khronos_runtime::traits::ir::SettingOperations {
        view: setting.operations.view.is_some(),
        create: setting.operations.create.is_some(),
        update: setting.operations.update.is_some(),
        delete: setting.operations.delete.is_some(),
    };

    khronos_runtime::traits::ir::Setting {
        id: setting.id.clone(),
        name: setting.name.clone(),
        description: setting.description.clone(),
        primary_key: setting.primary_key.clone(),
        title_template: setting.title_template.clone(),
        columns,
        supported_operations,
    }
}

/// Given a khronos page IR, the guild and template id, convert it to an template worker page struct
pub fn create_page(
    page: khronos_runtime::traits::ir::Page,
    guild_id: GuildId,
    template_id: String,
) -> Arc<Page> {
    let settings = page
        .settings
        .into_iter()
        .map(|setting| create_setting(setting, guild_id, template_id.clone()))
        .collect::<Vec<_>>();

    Arc::new(Page {
        template_id,
        title: page.title,
        description: page.description,
        settings,
    })
}

/// Given a template worker page struct, convert it to a khronos page IR
pub fn unravel_page(page: Arc<Page>) -> khronos_runtime::traits::ir::Page {
    let settings = page
        .settings
        .iter()
        .map(unravel_setting)
        .collect::<Vec<_>>();

    khronos_runtime::traits::ir::Page {
        title: page.title.clone(),
        description: page.description.clone(),
        settings,
    }
}
