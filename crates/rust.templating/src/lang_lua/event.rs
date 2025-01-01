pub use mlua::prelude::*;
use std::{cell::RefCell, sync::Arc};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InnerEvent {
    /// The title name of the event
    title: String,
    /// The name of the base event
    base_name: String,
    /// The name of the event
    name: String,
    /// The inner data of the object
    data: serde_json::Value,
    /// The author, if any, of the event
    author: Option<String>,
}

/// An `Event` is an object that can be passed to a Lua template
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateEvent {
    inner: Arc<InnerEvent>,
}

impl CreateEvent {
    /// Create a new Event
    pub fn new(
        title: String,
        base_name: String,
        name: String,
        data: serde_json::Value,
        author: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(InnerEvent {
                title,
                base_name,
                name,
                data,
                author,
            }),
        }
    }
}

impl CreateEvent {
    /// Returns the base name of the event
    pub fn base_name(&self) -> &str {
        &self.inner.base_name
    }

    /// Returns the name (NOT the base name) of the event
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Converts the `CreateEvent` into an `Event`
    pub(crate) fn to_event(&self) -> Event {
        Event {
            inner: self.inner.clone(),
            cached_data: RefCell::new(None),
        }
    }
}

/// An `Event` is an object that can be passed to a Lua template
pub(crate) struct Event {
    /// The inner data of the object
    inner: Arc<InnerEvent>,
    /// The cached serialized value of the template data
    cached_data: RefCell<Option<LuaValue>>,
}

impl LuaUserData for Event {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("title", |lua, this| {
            let title = lua.to_value(&this.inner.title)?;
            Ok(title)
        });
        fields.add_field_method_get("base_name", |lua, this| {
            let base_name = lua.to_value(&this.inner.base_name)?;
            Ok(base_name)
        });
        fields.add_field_method_get("name", |lua, this| {
            let name = lua.to_value(&this.inner.name)?;
            Ok(name)
        });
        fields.add_field_method_get("data", |lua, this| {
            // Check for cached serialized data
            let mut cached_data = this
                .cached_data
                .try_borrow_mut()
                .map_err(|e| LuaError::external(e.to_string()))?;

            if let Some(v) = cached_data.as_ref() {
                return Ok(v.clone());
            }

            log::trace!("Event: Serializing data");
            let v = lua.to_value(&this.inner.data)?;

            *cached_data = Some(v.clone());

            Ok(v)
        });
        fields.add_field_method_get("author", |lua, this| {
            let author = lua.to_value(&this.inner.author)?;
            Ok(author)
        });
    }
}
