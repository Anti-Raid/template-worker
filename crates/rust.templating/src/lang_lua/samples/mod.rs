use include_dir::{include_dir, Dir};

pub static DEFAULT_TEMPLATES: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/src/lang_lua/samples/samples");

pub fn load_embedded_template(event: &str) -> Result<String, silverpelt::Error> {
    let template = match DEFAULT_TEMPLATES.get_file(format!("{}.art", event)) {
        Some(template) => template,
        None => return Err("Template not found".into()),
    };

    let template_str = template.contents_utf8().ok_or("Failed to load template")?;

    Ok(template_str.to_string())
}
