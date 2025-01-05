use core::fmt;

use mlua::prelude::*;
use rbx_dom_weak::types::NumberSequenceKeypoint as DomNumberSequenceKeypoint;

use crate::lang_lua::plugins::lune::utils::TableBuilder;

use crate::lang_lua::plugins::lune::roblox::exports::LuaExportsTable;

use super::super::*;

/**
    An implementation of the [NumberSequenceKeypoint](https://create.roblox.com/docs/reference/engine/datatypes/NumberSequenceKeypoint) Roblox datatype.

    This implements all documented properties, methods & constructors of the `NumberSequenceKeypoint` class as of March 2023.
*/
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberSequenceKeypoint {
    pub(crate) time: f32,
    pub(crate) value: f32,
    pub(crate) envelope: f32,
}

impl<'lua> LuaExportsTable<'lua> for NumberSequenceKeypoint {
    const EXPORT_NAME: &'static str = "NumberSequenceKeypoint";

    fn create_exports_table(lua: &'lua Lua) -> LuaResult<LuaTable> {
        TableBuilder::new(lua)?
            .with_function(
                "new",
                |_, (time, value, envelope): (f32, f32, Option<f32>)| {
                    Ok(NumberSequenceKeypoint {
                        time,
                        value,
                        envelope: envelope.unwrap_or_default(),
                    })
                },
            )?
            .build_readonly()
    }
}

impl LuaUserData for NumberSequenceKeypoint {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Time", |_, this| Ok(this.time));
        fields.add_field_method_get("Value", |_, this| Ok(this.value));
        fields.add_field_method_get("Envelope", |_, this| Ok(this.envelope));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Eq, userdata_impl_eq);
        methods.add_meta_method(LuaMetaMethod::ToString, userdata_impl_to_string);
    }
}

impl fmt::Display for NumberSequenceKeypoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} > {}", self.time, self.value)
    }
}

impl From<DomNumberSequenceKeypoint> for NumberSequenceKeypoint {
    fn from(v: DomNumberSequenceKeypoint) -> Self {
        Self {
            time: v.time,
            value: v.value,
            envelope: v.envelope,
        }
    }
}

impl From<NumberSequenceKeypoint> for DomNumberSequenceKeypoint {
    fn from(v: NumberSequenceKeypoint) -> Self {
        Self {
            time: v.time,
            value: v.value,
            envelope: v.envelope,
        }
    }
}
