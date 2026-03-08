use std::{collections::HashSet, sync::LazyLock};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TenantState {
    pub events: HashSet<String>,
    pub flags: i32,
}

pub static DEFAULT_TENANT_STATE: LazyLock<TenantState> = LazyLock::new(|| TenantState {
    events: {
        let mut set = HashSet::new();
        set.insert("INTERACTION_CREATE".to_string());
        set.insert("WebGetSettings".to_string());
        set.insert("WebExecuteSetting".to_string());
        set
    },
    flags: 0,
});
