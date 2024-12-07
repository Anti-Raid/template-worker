pub use silverpelt::templates::LuaKVConstraints;

pub mod captcha {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Captcha {
        pub text: String,
        pub content: Option<String>, // Message content
        pub image: Option<Vec<u8>>,  // Image data
    }
}

pub mod templating_core {
    const MAX_CAPS: usize = 50;
    const MAX_PRAGMA_SIZE: usize = 2048;

    use std::str::FromStr;

    pub use silverpelt::templates::{create_shop_template, parse_shop_template};
    use silverpelt::Error;

    #[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
    pub struct TemplatePragma {
        pub lang: TemplateLanguage,

        #[serde(default)]
        pub allowed_caps: Vec<String>,

        #[serde(flatten)]
        pub extra_info: indexmap::IndexMap<String, serde_json::Value>,
    }

    impl TemplatePragma {
        pub fn parse(template: &str) -> Result<(&str, Self), Error> {
            let (first_line, rest) = match template.find('\n') {
                Some(i) => template.split_at(i),
                None => return Ok((template, Self::default())),
            };

            // Unravel any comments before the @pragma
            let first_line = first_line.trim_start_matches("--").trim();

            if !first_line.contains("@pragma ") {
                return Ok((template, Self::default()));
            }

            // Remove out the @pragma and serde parse it
            let first_line = first_line.replace("@pragma ", "");

            if first_line.len() > MAX_PRAGMA_SIZE {
                return Err("Pragma too large".into());
            }

            let pragma: TemplatePragma = serde_json::from_str(&first_line)?;

            if pragma.allowed_caps.len() > MAX_CAPS {
                return Err("Too many allowed capabilities specified".into());
            }

            Ok((rest, pragma))
        }
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
    pub enum TemplateLanguage {
        #[serde(rename = "lua")]
        #[default]
        Lua,
    }

    impl FromStr for TemplateLanguage {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "lang_lua" => Ok(Self::Lua),
                _ => Err(()),
            }
        }
    }

    impl std::fmt::Display for TemplateLanguage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Lua => write!(f, "lang_lua"),
            }
        }
    }

    #[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
    pub struct GuildTemplate {
        pub name: String,
        pub description: Option<String>,
        pub shop_name: Option<String>,
        pub events: Option<Vec<String>>,
        pub error_channel: Option<serenity::all::ChannelId>,
        pub content: String,
        pub created_by: String,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_by: String,
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub enum Template {
        Raw(String),
        Named(String),
    }
}

pub mod page {
    pub const MAX_PAGE_ID_LENGTH: usize = 128;

    pub struct Page {
        pub page_id: String,
        pub title: String,
        pub description: String,
        pub template: crate::Template,
        pub settings: Vec<ar_settings::types::Setting>,
    }
}
