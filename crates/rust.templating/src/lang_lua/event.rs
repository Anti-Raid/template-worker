pub use mlua::prelude::*;
use std::{ops::Deref, sync::Arc};

pub enum ArcOrNormal<T: Sized> {
    Arc(Arc<T>),
    Normal(T),
}

impl<T: Sized> Deref for ArcOrNormal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            ArcOrNormal::Arc(a) => a.as_ref(),
            ArcOrNormal::Normal(b) => b,
        }
    }
}

impl<T: serde::Serialize> serde::Serialize for ArcOrNormal<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ArcOrNormal::Arc(a) => serde::Serialize::serialize(a, serializer),
            ArcOrNormal::Normal(b) => serde::Serialize::serialize(b, serializer),
        }
    }
}

impl<'de, T: serde::de::Deserialize<'de>> serde::de::Deserialize<'de> for ArcOrNormal<T> {
    fn deserialize<D>(deserializer: D) -> Result<ArcOrNormal<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = T::deserialize(deserializer)?;
        Ok(ArcOrNormal::Normal(v))
    }
}

impl<T: Clone> Clone for ArcOrNormal<T> {
    fn clone(&self) -> Self {
        match self {
            ArcOrNormal::Arc(a) => ArcOrNormal::Arc(a.clone()),
            ArcOrNormal::Normal(b) => ArcOrNormal::Normal(b.clone()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InnerEvent {
    /// The title name of the event
    title: String,
    /// The name of the base event
    base_name: String,
    /// The name of the event
    name: String,
    /// The inner data of the object
    data: ArcOrNormal<serde_json::Value>,
    /// The random identifier of the event
    uid: sqlx::types::Uuid,
    /// The author, if any, of the event
    author: Option<String>,
}

/// An `Event` is an object that can be passed to a Lua template
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    inner: Arc<InnerEvent>,
}

impl Event {
    /// Create a new Event
    pub fn new(
        title: String,
        base_name: String,
        name: String,
        data: ArcOrNormal<serde_json::Value>,
        author: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(InnerEvent {
                title,
                base_name,
                name,
                data,
                uid: sqlx::types::Uuid::new_v4(),
                author,
            }),
        }
    }
}

impl Event {
    /// Returns the base name of the event
    pub fn base_name(&self) -> &str {
        &self.inner.base_name
    }

    /// Returns the name (NOT the base name) of the event
    pub fn name(&self) -> &str {
        &self.inner.name
    }
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
            log::trace!("Event: Serializing data");
            let v = lua.to_value(&this.inner.data)?;
            Ok(v)
        });
        fields.add_field_method_get("uid", |lua, this| {
            let uid = lua.to_value(&this.inner.uid)?;
            Ok(uid)
        });
        fields.add_field_method_get("author", |lua, this| {
            let author = lua.to_value(&this.inner.author)?;
            Ok(author)
        });
    }
}
