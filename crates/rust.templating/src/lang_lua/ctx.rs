use mlua::prelude::*;
use std::rc::Rc;

pub struct TemplateContext {
    pub guild_state: Rc<super::state::GuildState>,

    /// The template data
    pub template_data: Rc<super::state::TemplateData>,

    /// The cached serialized value of the template data
    cached_template_data: std::sync::Mutex<Option<LuaValue>>,
}

impl TemplateContext {
    pub fn new(
        guild_state: Rc<super::state::GuildState>,
        template_data: super::state::TemplateData,
    ) -> Self {
        Self {
            guild_state,
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

        fields.add_field_method_get("guild_id", |_, this| {
            Ok(this.guild_state.guild_id.to_string())
        });

        fields.add_field_method_get("current_user", |lua, this| {
            let v = lua.to_value(
                &this
                    .guild_state
                    .serenity_context
                    .cache
                    .current_user()
                    .clone(),
            )?;
            Ok(v)
        });
    }
}
