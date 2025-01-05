use mlua::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct TemplateContext {
    pub guild_state: Rc<crate::lang_lua::state::GuildState>,

    /// The template data
    pub template_data: Arc<crate::Template>,

    /// The cached serialized value of the template data
    cached_template_data: RefCell<Option<LuaValue>>,
}

impl TemplateContext {
    pub fn new(
        guild_state: Rc<crate::lang_lua::state::GuildState>,
        template_data: Arc<crate::Template>,
    ) -> Self {
        Self {
            guild_state,
            template_data,
            cached_template_data: RefCell::default(),
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
                .try_borrow_mut()
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
