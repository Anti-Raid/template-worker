use mlua::prelude::*;

pub fn plugin_docs() -> templating_docgen::Plugin {
    templating_docgen::Plugin::default()
        .name("@antiraid/interop")
        .description("This plugin allows interoperability with AntiRaid and controlled interaction with the low-levels of AntiRaid templating subsystem.")
        .type_mut(
            "null",
            "`null` is a special value that represents nothing. It is often used in AntiRaid instead of `nil` due to issues regarding existence etc. `null` is not equal to `nil` but is also an opaque type.",
            |t| {
                t
            },
        )
        .field_mut("null", |m| {
            m.description("The null value.")
            .typ("null")
        })
        .type_mut(
            "array_metatable",
            "`array_metatable` is a special metatable that is used to represent arrays across the Lua-AntiRaid templating subsystem boundary. This metatable must be set on all arrays over this boundary and is required to ensure AntiRaid knows the value you're sending it is actually an array and not an arbitrary Luau table.",
            |t| {
                t
            },
        )
        .method_mut("array_metatable", |m| {
            m.description("Returns the array metatable.")
            .return_("array_metatable", |r| {
                r.typ("table").description("The array metatable.")
            })
        })
        .method_mut("memusage", |m| {
            m.description("Returns the current memory usage of the Lua VM.")
            .return_("memory_usage", |r| {
                r.typ("f64").description("The current memory usage, in bytes, of the Lua VM.")
            })
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    // Null
    module.set("null", lua.null())?;

    // Array metatable
    module.set("array_metatable", lua.array_metatable())?;

    module.set(
        "memusage",
        lua.create_function(|lua, _: ()| Ok(lua.used_memory()))?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
