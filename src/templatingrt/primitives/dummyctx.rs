use khronos_runtime::traits::context::{
    CompatibilityFlags, KhronosContext, Limitations, ScriptData,
};
use khronos_runtime::traits::datastoreprovider::{DataStoreImpl, DataStoreProvider};
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::httpclientprovider::HTTPClientProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::ir::ObjectMetadata;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::objectstorageprovider::ObjectStorageProvider;
use khronos_runtime::utils::khronos_value::KhronosValue;
use std::{rc::Rc, sync::Arc};

#[derive(Clone)]
pub struct DummyProvider {
    template_data: Arc<ScriptData>,
    datastores: Vec<Rc<dyn DataStoreImpl>>,
}

impl DummyProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(datastores: Vec<Rc<dyn DataStoreImpl>>) -> Self {
        Self {
            template_data: Arc::new(ScriptData {
                guild_id: Some(serenity::all::GuildId::new(0)),
                name: "DUMMY".to_string(),
                description: None,
                shop_name: None,
                shop_owner: None,
                events: Vec::with_capacity(0),
                error_channel: None,
                lang: "luau-priv".to_string(), // allows template to know its a privileged template with dummy context
                allowed_caps: Vec::with_capacity(0),
                created_by: None,
                created_at: None,
                updated_by: None,
                updated_at: None,
                compatibility_flags: CompatibilityFlags::empty(),
            }),
            datastores,
        }
    }
}

impl KhronosContext for DummyProvider {
    type KVProvider = DummyKVProvider;
    type DiscordProvider = DummyDiscordProvider;
    type DataStoreProvider = DummyDataStoreProvider;
    type ObjectStorageProvider = DummyObjectStorageProvider;
    type HTTPClientProvider = DummyHTTPClientProvider;

    fn data(&self) -> &ScriptData {
        &self.template_data
    }

    fn limitations(&self) -> Limitations {
        Limitations::new(Vec::with_capacity(0))
    }

    fn guild_id(&self) -> Option<serenity::all::GuildId> {
        self.template_data.guild_id
    }

    fn owner_guild_id(&self) -> Option<serenity::all::GuildId> {
        self.template_data.shop_owner
    }

    fn template_name(&self) -> String {
        self.template_data.name.clone()
    }

    fn current_user(&self) -> Option<serenity::all::CurrentUser> {
        None
    }

    fn kv_provider(&self) -> Option<Self::KVProvider> {
        None
    }

    fn discord_provider(&self) -> Option<Self::DiscordProvider> {
        None
    }

    fn datastore_provider(&self) -> Option<Self::DataStoreProvider> {
        Some(DummyDataStoreProvider {
            datastores: self.datastores.clone(),
        })
    }

    fn objectstorage_provider(&self) -> Option<Self::ObjectStorageProvider> {
        None
    }

    fn httpclient_provider(&self) -> Option<Self::HTTPClientProvider> {
        None
    }
}

#[derive(Clone)]
pub struct DummyKVProvider {}

impl KVProvider for DummyKVProvider {
    fn attempt_action(&self, _scope: &[String], _bucket: &str) -> Result<(), crate::Error> {
        Ok(())
    }

    async fn get(
        &self,
        _scopes: &[String],
        _key: String,
    ) -> Result<Option<KvRecord>, crate::Error> {
        unreachable!()
    }

    async fn get_by_id(&self, _id: String) -> Result<Option<KvRecord>, crate::Error> {
        unreachable!()
    }

    async fn list_scopes(&self) -> Result<Vec<String>, crate::Error> {
        unreachable!()
    }

    async fn set(
        &self,
        _scopes: &[String],
        _key: String,
        _data: KhronosValue,
        _expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(bool, String), crate::Error> {
        unreachable!()
    }

    async fn set_expiry(
        &self,
        _scopes: &[String],
        _key: String,
        _expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), khronos_runtime::Error> {
        unreachable!()
    }

    async fn set_expiry_by_id(
        &self,
        _id: String,
        _expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), khronos_runtime::Error> {
        unreachable!()
    }

    async fn set_by_id(
        &self,
        _id: String,
        _data: KhronosValue,
        _expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), khronos_runtime::Error> {
        unreachable!()
    }

    async fn delete(&self, _scopes: &[String], _key: String) -> Result<(), crate::Error> {
        unreachable!()
    }

    async fn delete_by_id(&self, _id: String) -> Result<(), crate::Error> {
        unreachable!()
    }

    async fn find(
        &self,
        _scopes: &[String],
        _query: String,
    ) -> Result<Vec<KvRecord>, crate::Error> {
        unreachable!()
    }

    async fn exists(&self, _scopes: &[String], _key: String) -> Result<bool, crate::Error> {
        unreachable!()
    }

    async fn keys(&self, _scopes: &[String]) -> Result<Vec<String>, crate::Error> {
        unreachable!()
    }
}

#[derive(Clone)]
pub struct DummyDiscordProvider {}

impl DiscordProvider for DummyDiscordProvider {
    fn attempt_action(&self, _bucket: &str) -> serenity::Result<(), crate::Error> {
        Ok(())
    }

    async fn get_channel(
        &self,
        _channel_id: serenity::all::ChannelId,
    ) -> serenity::Result<serenity::all::GuildChannel, crate::Error> {
        unreachable!()
    }

    fn guild_id(&self) -> serenity::all::GuildId {
        unreachable!()
    }

    fn serenity_http(&self) -> &serenity::http::Http {
        unreachable!()
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct DummyDataStoreProvider {
    datastores: Vec<Rc<dyn DataStoreImpl>>,
}

impl DataStoreProvider for DummyDataStoreProvider {
    fn attempt_action(&self, _method: &str, _bucket: &str) -> Result<(), khronos_runtime::Error> {
        Ok(())
    }

    /// Returns a builtin data store given its name
    fn get_builtin_data_store(&self, name: &str) -> Option<Rc<dyn DataStoreImpl>> {
        for ds in self.datastores.iter() {
            if ds.name() == name {
                return Some(ds.clone());
            }
        }

        None
    }

    /// Returns all public builtin data stores
    fn public_builtin_data_stores(&self) -> Vec<String> {
        self.datastores.iter().map(|ds| ds.name()).collect()
    }
}

#[derive(Clone)]
pub struct DummyObjectStorageProvider {}

impl ObjectStorageProvider for DummyObjectStorageProvider {
    fn attempt_action(&self, _bucket: &str) -> Result<(), khronos_runtime::Error> {
        Ok(())
    }

    fn bucket_name(&self) -> String {
        "dummy-bucket".to_string()
    }

    async fn list_files(
        &self,
        _prefix: Option<String>,
    ) -> Result<Vec<ObjectMetadata>, khronos_runtime::Error> {
        unreachable!()
    }

    async fn file_exists(&self, _key: String) -> Result<bool, khronos_runtime::Error> {
        unreachable!()
    }

    async fn download_file(&self, _key: String) -> Result<Vec<u8>, khronos_runtime::Error> {
        unreachable!()
    }

    async fn get_file_url(
        &self,
        _key: String,
        _expiry: std::time::Duration,
    ) -> Result<String, khronos_runtime::Error> {
        unreachable!()
    }

    async fn upload_file(
        &self,
        _key: String,
        _data: Vec<u8>,
    ) -> Result<(), khronos_runtime::Error> {
        unreachable!()
    }

    async fn delete_file(&self, _key: String) -> Result<(), khronos_runtime::Error> {
        unreachable!()
    }
}

#[derive(Clone)]
pub struct DummyHTTPClientProvider {}

impl HTTPClientProvider for DummyHTTPClientProvider {
    fn attempt_action(&self, _bucket: &str, _url: &str) -> Result<(), khronos_runtime::Error> {
        Ok(())
    }
}
