use khronos_runtime::rt::{mlua_scheduler::LuaSchedulerAsyncUserData, mluau::prelude::*};
use sqlx::PgPool;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Uuid;

macro_rules! db_index_map {
    ($($type:ty => { $base:ident, $opt:ident, $list:ident, $typestr:literal }),* $(,)?) => {
        use serde::{Serialize, Deserialize};
        use sqlx::Row;

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "type", content = "value")]
        pub enum OpaqueValue {
            $( 
                $base($type),
                $opt(Option<$type>),
                $list(Vec<$type>),
            )*
        }

        pub struct OpaqueValueTaker(Vec<OpaqueValue>);
        impl FromLua for OpaqueValueTaker {
            fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
                let mut values = Vec::new();
                match value {
                    LuaValue::UserData(ud) => {
                        let Ok(ov) = ud.take::<OpaqueValue>() else {
                            return Err(LuaError::external("Expected OpaqueValue userdata"));
                        };
                        values.push(ov);
                    }
                    _ => return Err(LuaError::external("Expected a table of OpaqueValue userdata")),
                }
                Ok(OpaqueValueTaker(values))
            }
        }

        impl OpaqueValue {
            pub fn from_lua(lua: &Lua, value: LuaValue, type_name: &str) -> LuaResult<Self> {
                match type_name {
                    $(
                        $typestr => {
                            let val: $type = lua.from_value(value)?;
                            Ok(OpaqueValue::$base(val))
                        },
                        concat!($typestr, "?") => {
                            let val: Option<$type> = lua.from_value(value)?;
                            Ok(OpaqueValue::$opt(val))
                        },
                        concat!("{", $typestr, "}") => {
                            let val: Vec<$type> = lua.from_value(value)?;
                            Ok(OpaqueValue::$list(val))
                        },
                    )*
                    _ => Err(LuaError::external(format!("Unknown type for OpaqueValue conversion: {}", type_name))),
                }
            }

            pub fn type_name(&self) -> &'static str {
                match self {
                    $(
                        OpaqueValue::$base(_) => $typestr,
                        OpaqueValue::$opt(_) => concat!($typestr, "?"),
                        OpaqueValue::$list(_) => concat!("{", $typestr, "}"),
                    )*
                }
            }

            pub fn bind(self, query: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>) -> 
                sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>
            {
                match self {
                    $(
                        OpaqueValue::$base(v) => query.bind(v),
                        OpaqueValue::$opt(v) => query.bind(v),
                        OpaqueValue::$list(v) => query.bind(v),
                    )*
                }
            }
        }

        impl LuaUserData for OpaqueValue {
            fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
                methods.add_method("type", |_, this, ()| {
                    Ok(this.type_name())
                });
                methods.add_method("get", |lua, this, ()| {
                    lua.to_value(this)
                });
                methods.add_method("clone", |_lua, this, ()| {
                    Ok(this.clone())
                });
            }
        }

        pub struct DbIndexMapper {
            map: fn(&sqlx::postgres::PgRow, i32) -> Result<OpaqueValue, crate::Error>,
        }

        impl DbIndexMapper {
            fn new(typ: &str) -> Option<Self> {
                match typ {
                    $(
                        $typestr => {
                            let map_fn = |row: &sqlx::postgres::PgRow, idx: i32| {
                                let v = OpaqueValue::$base(row.try_get(idx as usize)?);
                                Ok(v)
                            };

                            Some(Self { map: map_fn })
                        },
                        concat!($typestr, "?") => {
                            let map_fn = |row: &sqlx::postgres::PgRow, idx: i32| {
                                let v = OpaqueValue::$opt(row.try_get(idx as usize)?);
                                Ok(v)
                            };

                            Some(Self { map: map_fn })
                        },
                        concat!("{", $typestr, "}") => {
                            let map_fn = |row: &sqlx::postgres::PgRow, idx: i32| {
                                let v = OpaqueValue::$list(row.try_get(idx as usize)?);
                                Ok(v)
                            };

                            Some(Self { map: map_fn })
                        },
                    )*
                    _ => return None,
                }
            }
        }

        impl LuaUserData for DbIndexMapper {}
    };
}

db_index_map! {
    i32 => { I32, I32Opt, I32List, "i32" },
    String => { String, StringOpt, StringList, "string" },
    bool => { Bool, BoolOpt, BoolList, "boolean" },
    f64 => { F64, F64Opt, F64List, "f64" },
    DateTime<Utc> => { DateTime, DateTimeOpt, DateTimeList, "datetime" },
    JsonValue => { Json, JsonOpt, JsonList, "json" },
    Uuid => { Uuid, UuidOpt, UuidList, "uuid" },
}

#[allow(dead_code)]
pub struct Db {
    pub pool: PgPool,
}

impl LuaUserData for Db {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("indexmap", |_lua, _this, typ: String| {
            match DbIndexMapper::new(&typ) {
                Some(mapper) => Ok(mapper),
                None => Err(LuaError::external(format!("Unsupported type for index mapping: {typ}"))),
            }
        });

        methods.add_method("cast", |lua, _this, (value, typ): (LuaValue, String)| {
            OpaqueValue::from_lua(lua, value, &typ)
        });

        methods.add_scheduler_async_method("fetchall", async |_lua, this, (query, params): (String, OpaqueValueTaker)| {
            let mut q = sqlx::query(&query);
            for param in params.0 {
                q = param.bind(q);
            }
            let rows = q.fetch_all(&this.pool).await.map_err(|e| LuaError::external(format!("Database query failed: {}", e)))?;
            Ok(rows.into_iter().map(|row| PgRow { row }).collect::<Vec<_>>())
        });
    }
}

pub struct PgRow {
    row: sqlx::postgres::PgRow,
}

impl LuaUserData for PgRow {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_lua, this, (idx, mapper): (i32, LuaUserDataRef<DbIndexMapper>)| {
            match (mapper.map)(&this.row, idx) {
                Ok(value) => Ok(value),
                Err(e) => Err(LuaError::external(format!("Failed to get column at index {}: {}", idx, e))),
            }
        });
    }
}