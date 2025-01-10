/// Validates a set of components
pub fn validate_components(rows: &[serenity::all::ActionRow]) -> Result<(), crate::Error> {
    const MAX_BUTTONS_PER_ACTION_ROW: usize = 5;
    const MAX_SELECTS_PER_ACTION_ROW: usize = 1;
    const MAX_POSSIBLE_COMPONENTS: usize = 5; // 5 action rows, each with 5 components

    if rows.len() > MAX_POSSIBLE_COMPONENTS {
        return Err(format!("Too many components, limit is {}", MAX_POSSIBLE_COMPONENTS).into());
    }

    for row in rows.iter() {
        if row.kind != serenity::all::ComponentType::ActionRow {
            return Err("Invalid component type, must be an action row".into());
        }

        // Validate the action row
        let mut num_buttons = 0;
        let mut num_selects = 0;

        for component in row.components.iter() {
            match component {
                serenity::all::ActionRowComponent::Button(_) => {
                    if num_buttons >= MAX_BUTTONS_PER_ACTION_ROW {
                        return Err(format!(
                            "Too many buttons in action row, limit is {}",
                            MAX_BUTTONS_PER_ACTION_ROW
                        )
                        .into());
                    }
                    if num_selects > 0 {
                        return Err("Cannot have buttons and a select menu in action row".into());
                    }
                    num_buttons += 1;
                }
                serenity::all::ActionRowComponent::SelectMenu(_) => {
                    if num_selects >= MAX_SELECTS_PER_ACTION_ROW {
                        return Err(format!(
                            "Too many select menus in action row, limit is {}",
                            MAX_SELECTS_PER_ACTION_ROW
                        )
                        .into());
                    }

                    if num_buttons > 0 {
                        return Err("Cannot have buttons and a select menu in action row".into());
                    }

                    num_selects += 1;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Validates an embed, returning total number of characters used
pub fn validate_embed(embed: &super::builders::CreateEmbed) -> Result<usize, crate::Error> {
    const EMBED_TITLE_LIMIT: usize = 256;
    const EMBED_DESCRIPTION_LIMIT: usize = 4096;
    const EMBED_FOOTER_TEXT_LIMIT: usize = 2048;
    const EMBED_AUTHOR_NAME_LIMIT: usize = 256;
    const EMBED_FIELD_NAME_LIMIT: usize = 256;
    const EMBED_FIELD_VALUE_LIMIT: usize = 1024;

    let mut total_chars = 0;

    // Validate title
    if let Some(title) = &embed.title {
        if title.is_empty() {
            return Err("Embed title cannot be empty".into());
        }

        if title.len() > EMBED_TITLE_LIMIT {
            return Err(format!("Embed title is too long, limit is {}", EMBED_TITLE_LIMIT).into());
        }

        total_chars += title.len();
    }

    // Validate description
    if let Some(description) = &embed.description {
        if description.is_empty() {
            return Err("Embed description cannot be empty".into());
        }

        if description.len() > EMBED_DESCRIPTION_LIMIT {
            return Err(format!(
                "Embed description is too long, limit is {}",
                EMBED_DESCRIPTION_LIMIT
            )
            .into());
        }

        total_chars += description.len();
    }

    // Validate footer
    if let Some(footer) = &embed.footer {
        if footer.text.is_empty() {
            return Err("Embed footer text cannot be empty".into());
        }

        if footer.text.len() > EMBED_FOOTER_TEXT_LIMIT {
            return Err(format!(
                "Embed footer text is too long, limit is {}",
                EMBED_FOOTER_TEXT_LIMIT
            )
            .into());
        }

        total_chars += footer.text.len();
    }

    // Validate author
    if let Some(author) = &embed.author {
        if author.name.is_empty() {
            return Err("Embed author name cannot be empty".into());
        }

        if author.name.len() > EMBED_AUTHOR_NAME_LIMIT {
            return Err(format!(
                "Embed author name is too long, limit is {}",
                EMBED_AUTHOR_NAME_LIMIT
            )
            .into());
        }

        total_chars += author.name.len();
    }

    // Validate fields
    for field in embed.fields.iter() {
        if field.name.is_empty() {
            return Err("Embed field name cannot be empty".into());
        }

        if field.name.len() > EMBED_FIELD_NAME_LIMIT {
            return Err(format!(
                "Embed field name is too long, limit is {}",
                EMBED_FIELD_NAME_LIMIT
            )
            .into());
        }

        total_chars += field.name.len();

        if field.value.is_empty() {
            return Err("Embed field value cannot be empty".into());
        }

        if field.value.len() > EMBED_FIELD_VALUE_LIMIT {
            return Err(format!(
                "Embed field value is too long, limit is {}",
                EMBED_FIELD_VALUE_LIMIT
            )
            .into());
        }

        total_chars += field.value.len();
    }

    Ok(total_chars)
}

/// Validates all messages
pub fn validate_message<'a>(message: &super::builders::CreateMessage) -> Result<(), crate::Error> {
    pub const MESSAGE_CONTENT_LIMIT: usize = 2000;
    pub const MAX_EMBED_CHARACTERS_LIMIT: usize = 6000;

    let has_content = message.content.is_some();
    let has_embed = !message.embeds.is_empty();
    let has_attachments = message.attachments.is_some()
        && !message
            .attachments
            .as_ref()
            .unwrap()
            .new_and_existing_attachments
            .is_empty();
    let has_poll = message.poll.is_some();
    let has_sticker_ids = !message.sticker_ids.is_empty();
    let has_components =
        message.components.is_some() && !message.components.as_ref().unwrap().is_empty();

    if !has_content
        && !has_embed
        && !has_attachments
        && !has_poll
        && !has_sticker_ids
        && !has_components
    {
        return Err("No content/embeds/attachments/poll/sticker_ids/components set".into());
    }

    if let Some(content) = message.content.as_ref() {
        if content.is_empty() {
            return Err("Message content cannot be empty".into());
        }

        if content.len() > MESSAGE_CONTENT_LIMIT {
            return Err(format!(
                "Message content is too long, limit is {}",
                MESSAGE_CONTENT_LIMIT
            )
            .into());
        }
    }

    // Validate all embeds
    let mut total_embed_chars = 0;

    for embed in message.embeds.iter() {
        total_embed_chars += validate_embed(embed)?;

        if total_embed_chars > MAX_EMBED_CHARACTERS_LIMIT {
            return Err(format!(
                "Total embed characters is too long, limit is {}",
                MAX_EMBED_CHARACTERS_LIMIT
            )
            .into());
        }
    }

    // Validate components
    if let Some(components) = message.components.as_ref() {
        validate_components(components)?
    }

    Ok(())
}
