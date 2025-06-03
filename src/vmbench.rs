use crate::templatingrt::*;
use crate::templatingrt::template::Template;
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::GuildId;
use vfs::FileSystem;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FireBenchmark {
    pub hashmap_insert_time: u128,
    pub get_lua_vm: u128,
    pub exec_simple: u128,
    pub exec_no_wait: u128,
    pub exec_error: u128,
}

/// Benchmark the Lua VM
pub async fn benchmark_vm(
    guild_id: GuildId,
    cgs: CreateGuildState
) -> Result<FireBenchmark, silverpelt::Error> {
    return Err("Being rewritten".into())

    // Get_lua_vm
    /*let cgs_a = cgs.clone();
    let guild_id_a = guild_id;

    let start = std::time::Instant::now();
    let _ = get_lua_vm(guild_id_a, cgs_a).await?;
    let get_lua_vm = start.elapsed().as_micros();

    let new_map = scc::HashMap::new();
    let start = std::time::Instant::now();
    let _ = new_map.insert_async(1, 1).await;
    let hashmap_insert_time = start.elapsed().as_micros();

    // Exec simple with wait
    fn str_to_map(s: &str) -> vfs::MemoryFS {
        let fs = vfs::MemoryFS::new();
        fs.create_file("/init.luau")
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
        fs
    }    

    let pt = {
        let mut tmpl = Template {
            content: str_to_map("return 1"),
            name: "benchmark1".to_string(),
            ..Default::default()
        };

        tmpl.prepare_ready_fs();

        tmpl
    };

    let cgs_a = cgs.clone();
    let guild_id_a = guild_id;

    let start = std::time::Instant::now();
    let n = execute(
        guild_id_a,
        cgs_a,
        LuaVmAction::DispatchInlineEvent {
            event: CreateEvent::new(
                "Benchmark".to_string(),
                "Benchmark".to_string(),
                serde_json::Value::Null,
                None,
            ),
            template: pt.into(),
        },
    )
    .await?
    .wait()
    .await?
    .into_response_first::<i32>()?;

    let exec_simple = start.elapsed().as_micros();

    if n != 1 {
        return Err(format!("Expected 1, got {}", n).into());
    }

    // Exec simple with no wait
    let pt = {
        let mut tmpl = Template {
            content: str_to_map("return 1"),
            name: "benchmark2".to_string(),
            ..Default::default()
        };

        tmpl.prepare_ready_fs();

        tmpl
    };

    let cgs_a = cgs.clone();
    let guild_id_a = guild_id;

    let start = std::time::Instant::now();
    execute(
        guild_id_a,
        cgs_a,
        LuaVmAction::DispatchInlineEvent {
            event: CreateEvent::new(
                "Benchmark".to_string(),
                "Benchmark".to_string(),
                serde_json::Value::Null,
                None,
            ),
            template: pt.into(),
        },
    )
    .await?;

    let exec_no_wait = start.elapsed().as_micros();

    // Exec simple with wait
    let pt = {
        let mut tmpl = Template {
            content: str_to_map("error('MyError')\nreturn 1"),
            name: "benchmark3".to_string(),
            ..Default::default()
        };

        tmpl.prepare_ready_fs();

        tmpl
    };

    let cgs_a = cgs.clone();
    let guild_id_a = guild_id;

    let start = std::time::Instant::now();
    let err = execute(
        guild_id_a,
        cgs_a,
        LuaVmAction::DispatchInlineEvent {
            event: CreateEvent::new(
                "Benchmark".to_string(),
                "Benchmark".to_string(),
                serde_json::Value::Null,
                None,
            ),
            template: pt.into(),
        },
    )
    .await?
    .wait()
    .await?;

    let exec_error = start.elapsed().as_micros();

    let first = err.results.into_iter().next().ok_or("No results")?;

    let Some(err) = first.lua_error() else {
        return Err("Expected error, got success".into());
    };

    if !err.contains("MyError") {
        return Err(format!("Expected MyError, got {}", err).into());
    }

    Ok(FireBenchmark {
        get_lua_vm,
        hashmap_insert_time,
        exec_simple,
        exec_no_wait,
        exec_error,
    })*/
}

