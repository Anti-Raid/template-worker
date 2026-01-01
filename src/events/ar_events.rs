use strum::{IntoStaticStr, VariantNames};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StartupEvent {
    pub reason: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, IntoStaticStr, VariantNames)]
#[must_use]
pub enum AntiraidEvent {
    /// Fired when a key is resumed
    /// 
    /// This occurs if a resumable key is set and the template is reloaded or the worker process restarted
    OnStartup(StartupEvent),
}

impl std::fmt::Display for AntiraidEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: &'static str = self.into();
        write!(f, "{}", s)
    }
}

impl AntiraidEvent {
    /// Returns the variant names
    pub fn variant_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Convert the event's inner data to a JSON value
    pub fn to_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            AntiraidEvent::OnStartup(templates) => serde_json::to_value(templates),
        }
    }
}
