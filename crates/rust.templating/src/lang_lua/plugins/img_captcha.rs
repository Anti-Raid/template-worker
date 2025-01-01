use super::promise::lua_promise;
use captcha::filters::Filter;
use mlua::prelude::*;

pub const CREATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
pub const MAX_CHAR_COUNT: u8 = 10;
pub const MAX_FILTERS: usize = 12;
pub const MAX_VIEWBOX_X: u32 = 512;
pub const MAX_VIEWBOX_Y: u32 = 512;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CaptchaConfig {
    pub char_count: u8,
    pub filters: Vec<Box<dyn Filter>>,
    pub viewbox_size: (u32, u32),
    pub set_viewbox_at_idx: Option<usize>,
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            char_count: 5,
            filters: vec![
                Box::new(captcha::filters::Noise::new(0.1)),
                Box::new(captcha::filters::Wave::new(4.0, 2.0)),
                Box::new(captcha::filters::Line::new(
                    (1.0, 0.0),
                    (20.0, 20.0),
                    2.0,
                    captcha::filters::SerdeColor::new(0, 30, 100),
                )),
                Box::new(captcha::filters::RandomLine::new()),
                Box::new(captcha::filters::Grid::new(10, 30)),
                Box::new(captcha::filters::ColorInvert::new()),
            ],
            viewbox_size: (512, 512),
            set_viewbox_at_idx: None,
        }
    }
}

impl CaptchaConfig {
    pub fn is_valid(&self) -> Result<(), silverpelt::Error> {
        if self.char_count == 0 {
            return Err("char_count must be greater than 0".into());
        }

        if self.char_count > MAX_CHAR_COUNT {
            return Err(format!(
                "char_count must be less than or equal to {}",
                MAX_CHAR_COUNT
            )
            .into());
        }

        if self.filters.len() > MAX_FILTERS {
            return Err(format!("filters must be less than or equal to {}", MAX_FILTERS).into());
        }

        if self.viewbox_size.0 == 0 || self.viewbox_size.0 >= MAX_VIEWBOX_X {
            return Err(format!(
                "viewbox_size.0 must be greater than 0 and less than {}",
                MAX_VIEWBOX_X
            )
            .into());
        }

        if self.viewbox_size.1 == 0 || self.viewbox_size.1 >= MAX_VIEWBOX_Y {
            return Err(format!(
                "viewbox_size.1 must be greater than 0 and less than {}",
                MAX_VIEWBOX_Y
            )
            .into());
        }

        if let Some(set_viewbox_at_idx) = self.set_viewbox_at_idx {
            if set_viewbox_at_idx >= self.filters.len() {
                return Err("set_viewbox_at_idx must be less than the length of filters".into());
            }
        }

        for f in self.filters.iter() {
            f.validate(self.viewbox_size)?;
        }

        Ok(())
    }

    pub async fn create_captcha(
        self,
        timeout: std::time::Duration,
    ) -> Result<(String, Vec<u8>), silverpelt::Error> {
        self.is_valid()?;

        let start_time = std::time::Instant::now();

        tokio::task::spawn_blocking(move || {
            let mut c = captcha::Captcha::new();
            c.add_random_chars(self.char_count as u32);

            if let Some(set_viewbox_at_idx) = self.set_viewbox_at_idx {
                // Do two separate for loops, one for 0..set_viewbox_at_idx and one for set_viewbox_at_idx..filters.len()
                for f in self.filters.iter().take(set_viewbox_at_idx) {
                    // Check if we've exceeded the timeout
                    if start_time - std::time::Instant::now() > timeout {
                        return Err(format!(
                            "Timeout exceeded when rendering captcha: {:?}",
                            timeout
                        )
                        .into());
                    }

                    c.apply_filter_dyn(f)?;
                }

                c.view(self.viewbox_size.0, self.viewbox_size.1);

                // Check if we've exceeded the timeout
                if start_time - std::time::Instant::now() > timeout {
                    return Err(
                        format!("Timeout exceeded when rendering captcha: {:?}", timeout).into(),
                    );
                }

                for f in self.filters.iter().skip(set_viewbox_at_idx) {
                    // Check if we've exceeded the timeout
                    if start_time - std::time::Instant::now() > timeout {
                        return Err(format!(
                            "Timeout exceeded when rendering captcha: {:?}",
                            timeout
                        )
                        .into());
                    }

                    c.apply_filter_dyn(f)?;
                }
            } else {
                c.view(self.viewbox_size.0, self.viewbox_size.1);

                for f in self.filters.iter() {
                    // Check if we've exceeded the timeout
                    if start_time - std::time::Instant::now() > timeout {
                        return Err(format!(
                            "Timeout exceeded when rendering captcha: {:?}",
                            timeout
                        )
                        .into());
                    }

                    c.apply_filter_dyn(f)?;
                }
            }

            Ok(c.as_tuple().ok_or("Failed to create captcha")?)
        })
        .await?
    }
}

pub fn plugin_docs() -> templating_docgen::Plugin {
    templating_docgen::Plugin::default()
        .name("@antiraid/img_captcha")
        .description("This plugin allows for the creation of text/image CAPTCHA's with customizable filters which can be useful in protecting against bots.")
        .type_mut(
            "CaptchaConfig",
            "Captcha configuration. See examples for the arguments",
            |t| {
                t
                .example(std::sync::Arc::new(CaptchaConfig::default()))
                .field("char_count", |f| {
                    f.typ("u8").description("The number of characters the CAPTCHA should have.")
                })
                .field("filters", |f| {
                    f.typ("{any}").description("See example for the parameters to pass for the filter as well as https://github.com/Anti-Raid/captcha")
                })
                .field("viewbox_size", |f| {
                    f.typ("(u32, u32)").description("The size of the viewbox to render the CAPTCHA in.")
                })
                .field("set_viewbox_at_idx", |f| {
                    f.typ("Option<usize>").description("At what index of CAPTCHA generation should a viewbox be created at.")
                })
            },
        )
        .method_mut("new", |m| {
            m.description("Creates a new CAPTCHA with the given configuration.")
            .parameter("config", |p| {
                p.typ("CaptchaConfig").description("The configuration to use for the CAPTCHA.")
            })
            .return_("captcha", |r| {
                r.typ("{u8}").description("The created CAPTCHA object.")
            })
            .is_promise(true)
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "new",
        lua.create_function(|_, (config,): (LuaValue,)| {
            Ok(lua_promise!(config, |lua, config|, {
                let config: CaptchaConfig = lua.from_value(config)?;

                let (text, image) = config
                    .create_captcha(CREATE_TIMEOUT)
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                let captcha = crate::core::captcha::Captcha {
                    text,
                    image: Some(image),
                    content: Some("Please enter the text from the image".to_string()),
                };

                lua.to_value(&captcha) // Return the captcha object
            }))
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table
    Ok(module)
}
