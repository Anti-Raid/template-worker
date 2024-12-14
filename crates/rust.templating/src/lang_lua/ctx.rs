use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Serialize, Deserialize)]
pub struct TemplateContext {
    pub template_data: Rc<super::state::TemplateData>,

    #[serde(skip)]
    #[serde(default = "std::sync::Mutex::default")]
    /// The cached serialized value of the template data
    cached_template_data: std::sync::Mutex<Option<LuaValue>>,
}

impl TemplateContext {
    pub fn new(template_data: super::state::TemplateData) -> Self {
        Self {
            template_data: Rc::new(template_data),
            cached_template_data: std::sync::Mutex::default(),
        }
    }
}

pub type TemplateContextRef = LuaUserDataRef<TemplateContext>;

impl LuaUserData for TemplateContext {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("template_data", |lua, this| {
            // Check for cached serialized data
            let mut cached_data = this
                .cached_template_data
                .lock()
                .map_err(|e| LuaError::external(e.to_string()))?;

            if let Some(v) = cached_data.as_ref() {
                return Ok(v.clone());
            }

            log::trace!("TemplateContext: Serializing data");
            let v = lua.to_value(&this.template_data)?;

            *cached_data = Some(v.clone());

            Ok(v)
        });

        fields.add_field_method_get("guild_id", |lua, _| {
            let Some(data) = lua.app_data_ref::<crate::lang_lua::state::LuaUserData>() else {
                return Err(LuaError::external("No app data found"));
            };

            Ok(data.guild_id.to_string())
        });

        fields.add_field_method_get("current_user", |lua, _| {
            let Some(data) = lua.app_data_ref::<crate::lang_lua::state::LuaUserData>() else {
                return Err(LuaError::external("No app data found"));
            };

            let v = lua.to_value(&data.serenity_context.cache.current_user().clone())?;
            Ok(v)
        });
    }
}
