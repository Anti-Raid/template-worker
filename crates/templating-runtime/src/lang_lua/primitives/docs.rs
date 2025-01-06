pub fn document_primitives() -> crate::doclib::PrimitiveListBuilder {
    crate::doclib::PrimitiveListBuilder::default()
        .add("u8", "number", "An unsigned 8-bit integer. **Note: u8 arrays (`{u8}`) are often used to represent an array of bytes in AntiRaid**", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("0-{}", u8::MAX),
            )
        })
        .add("u16", "number", "An unsigned 16-bit integer.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("0-{}", u16::MAX),
            )
        })
        .add("u32", "number", "An unsigned 32-bit integer.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("0-{}", u32::MAX),
            )
        })
        .add("u64", "number", "An unsigned 64-bit integer. **Note that most, if not all, cases of `i64` in the actual API are either `string` or the `I64` custom type from typesext**", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("0-{}", u64::MAX),
            )
        })
        .add("i8", "number", "A signed 8-bit integer.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("{}-{}", i8::MIN, i8::MAX),
            )
        })
        .add("i16", "number", "A signed 16-bit integer.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("{}-{}", i16::MIN, i16::MAX),
            )
        })
        .add("i32", "number", "A signed 32-bit integer.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("{}-{}", i32::MIN, i32::MAX),
            )
        })
        .add("i64", "number", "A signed 64-bit integer. **Note that most, if not all, cases of `i64` in the actual API are either `string` or the `I64` custom type from typesext**", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("{}-{}", i64::MIN, i64::MAX),
            )
        })
        .add("f32", "number", "A 32-bit floating point number.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                "IEEE 754 single-precision floating point",
            )
        })
        .add("f64", "number", "A 64-bit floating point number.", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                "IEEE 754 double-precision floating point",
            )
        })
        .add("byte", "number", "An unsigned 8-bit integer that semantically stores a byte of information", |p| {
            p.add_constraint(
                "range",
                "The range of values this number can take on",
                &format!("0-{}", u8::MAX),
            )
        })
        .add("bool", "boolean", "A boolean value.", |p| p)
        .add("char", "string", "A single Unicode character.", |p| {
            p.add_constraint(
                "length",
                "The length of the string",
                "1",
            )
        })
        .add("string", "string", "A UTF-8 encoded string.", |p| {
            p.add_constraint(
                "encoding",
                "Accepted character encoding",
                "UTF-8 *only*",
            )
        })
        .add("function", "function", "A Lua function.", |p| p)
        .type_mut("Event", "An event that has been dispatched to the template. This is what `args` is in the template.", |mut t| {
            t
            .field("base_name", |f| {
                f
                .typ("string")
                .description("The base name of the event.")
            })
            .field("name", |f| {
                f
                .typ("string")
                .description("The name of the event.")
            })
            .field("data", |f| {
                f
                .typ("unknown")
                .description("The data of the event.")
            })
            .field("can_respond", |f| {
                f
                .typ("boolean")
                .description("Whether the event can be responded to.")
            })
            .field("response", |f| {
                f
                .typ("unknown")
                .description("The current response of the event. This can be overwritten by the template by just setting it to a new value.")
            })
            .field("author", |f| {
                f
                .typ("string?")
                .description("The author of the event, if any. If there is no known author, this field will either be `nil` or `null`.")
            })
        })
        .type_mut(
            "Template",
            "`Template` is a struct that represents the data associated with a template. Fields are still being documented and subject to change.",
            |t| {
                t
                .example(std::sync::Arc::new(crate::Template::default()))
                .field("language", |f| {
                    f.typ("string").description("The language of the template.")
                })
                .field("allowed_caps", |f| {
                    f.typ("{string}").description("The allowed capabilities provided to the template.")
                })
            },
        )
        .type_mut(
            "TemplateContext",
            "`TemplateContext` is a struct that represents the context of a template. Stores data including the templates data, pragma and what capabilities it should have access to. Passing a TemplateContext is often required when using AntiRaid plugins for security purposes.",
            |mut t| {
                t
                .field("template_data", |f| {
                    f
                    .typ("TemplateData")
                    .description("The data associated with the template.")
                })
                .field("guild_id", |m| {
                    m.description("The current guild ID the template is running on.")
                    .typ("string")
                })
                .field("current_user", |m| {
                    m.description("Returns AntiRaid's discord user object [the current discord bot user driving the template].")
                    .typ("Serenity.User")
                })
            },
        )
}
