use std::str::FromStr;

use chrono::{Datelike, TimeZone, Timelike};
use chrono_tz::OffsetComponents;
use mlua::prelude::*;

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/timezone")
        .description("This plugin allows for the managing timezones.")
        .type_mut(
            "Timezone",
            "A timezone object. Can be used for formatting and/or converting between timezones",
            |t| t
            .description("A timezone object.")
            .method_mut("utcToTz", |m| {
                m.description("Translates a timestamp in UTC time to a datetime in the said specific timezone.")
                    .parameter("year", |p| {
                        p.typ("number")
                            .description("The year to translate.")
                    })
                    .parameter("month", |p| {
                        p.typ("number")
                            .description("The month to translate.")
                    })
                    .parameter("day", |p| {
                        p.typ("number")
                            .description("The day to translate.")
                    })
                    .parameter("hours", |p| {
                        p.typ("number")
                            .description("The hours to translate.")
                    })
                    .parameter("minutes", |p| {
                        p.typ("number")
                            .description("The minutes to translate.")
                    })
                    .parameter("secs", |p| {
                        p.typ("number")
                            .description("The seconds to translate.")
                    })
                    .parameter("all", |p| {
                        p.typ("boolean?")
                            .description("Whether to return both offsets if the time is ambiguous.")
                    })
                    .return_("date", |r| {
                        r.typ("DateTime")
                            .description("The translated datetime.")
                    })
                    .return_("date2", |r| {
                        r.typ("DateTime?")
                            .description("The second translated datetime if the time is ambiguous.")
                    })
            })
            .method_mut("tzToUtc", |m| {
                m.description("Translates a timestamp in the specified timezone to a datetime in UTC.")
                    .parameter("year", |p| {
                        p.typ("number")
                            .description("The year to translate.")
                    })
                    .parameter("month", |p| {
                        p.typ("number")
                            .description("The month to translate.")
                    })
                    .parameter("day", |p| {
                        p.typ("number")
                            .description("The day to translate.")
                    })
                    .parameter("hours", |p| {
                        p.typ("number")
                            .description("The hours to translate.")
                    })
                    .parameter("minutes", |p| {
                        p.typ("number")
                            .description("The minutes to translate.")
                    })
                    .parameter("secs", |p| {
                        p.typ("number")
                            .description("The seconds to translate.")
                    })
                    .parameter("all", |p| {
                        p.typ("boolean?")
                            .description("Whether to return both offsets if the time is ambiguous.")
                    })
                    .return_("date", |r| {
                        r.typ("DateTime")
                            .description("The translated datetime.")
                    })
                    .return_("date2", |r| {
                        r.typ("DateTime?")
                            .description("The second translated datetime if the time is ambiguous.")
                    })
                    
            })
            .method_mut("timeUtcToTz", |m| {
                m.description("Translates a time of the current day in UTC time to a datetime in the said specific timezone.")
                    .parameter("hours", |p| {
                        p.typ("number")
                            .description("The hours to translate.")
                    })
                    .parameter("minutes", |p| {
                        p.typ("number")
                            .description("The minutes to translate.")
                    })
                    .parameter("secs", |p| {
                        p.typ("number")
                            .description("The seconds to translate.")
                    })
                    .return_("date", |r| {
                        r.typ("DateTime")
                            .description("The translated datetime.")
                    })
            })
            .method_mut("timeTzToUtc", |m| {
                m.description("Translates a time of the current day in the said specific timezone to a datetime in UTC.")
                    .parameter("hours", |p| {
                        p.typ("number")
                            .description("The hours to translate.")
                    })
                    .parameter("minutes", |p| {
                        p.typ("number")
                            .description("The minutes to translate.")
                    })
                    .parameter("secs", |p| {
                        p.typ("number")
                            .description("The seconds to translate.")
                    })
                    .return_("date", |r| {
                        r.typ("DateTime")
                            .description("The translated datetime.")
                    })
            })
            .method_mut("date", |m| {
                m.description("Translates the current timestamp to a datetime in the said specific timezone.")
                    .return_("date", |r| {
                        r.typ("DateTime")
                            .description("The translated datetime.")
                    })
            })
        )
        .type_mut("TimeDelta", "A time delta object. Represents a difference in time.", |t| {
            t.description("A time delta object. Supports addition/subtraction with another TimeDelta object as well as comparisons with them.")
                .method_mut("offset_string", |m| {
                    m.description("Returns the offset as a string.")
                        .return_("offset", |r| {
                            r.typ("string")
                                .description("The offset as a string.")
                        })
                })
                .field("nanos", |f| {
                    f.typ("number")
                        .description("The number of nanoseconds in the time delta.")
                })
                .field("micros", |f| {
                    f.typ("number")
                        .description("The number of microseconds in the time delta.")
                })
                .field("millis", |f| {
                    f.typ("number")
                        .description("The number of milliseconds in the time delta.")
                })
                .field("seconds", |f| {
                    f.typ("number")
                        .description("The number of seconds in the time delta.")
                })
                .field("minutes", |f| {
                    f.typ("number")
                        .description("The number of minutes in the time delta.")
                })
                .field("hours", |f| {
                    f.typ("number")
                        .description("The number of hours in the time delta.")
                })
                .field("days", |f| {
                    f.typ("number")
                        .description("The number of days in the time delta.")
                })
                .field("weeks", |f| {
                    f.typ("number")
                        .description("The number of weeks in the time delta.")
                })

        })
        .type_mut("DateTime", "A datetime object. Represents a specific point in time.", |t| {
            t.description("A datetime object. Supports addition/subtraction with TimeDelta objects as well as comparisons with other DateTime objects.")
                .method_mut("with_timezone", |m| {
                    m.description("Converts the datetime to the specified timezone.")
                        .parameter("tz", |p| {
                            p.typ("Timezone")
                                .description("The timezone to convert to.")
                        })
                        .return_("dt", |r| {
                            r.typ("DateTime")
                                .description("The converted datetime.")
                        })
                })
                .method_mut("format", |m| {
                    m.description("Formats the datetime using the specified format string.")
                        .parameter("format", |p| {
                            p.typ("string")
                                .description("The format string to use.")
                        })
                        .return_("formatted", |r| {
                            r.typ("string")
                                .description("The formatted datetime.")
                        })
                })
                .field("year", |f| {
                    f.typ("number")
                        .description("The year of the datetime.")
                })
                .field("month", |f| {
                    f.typ("number")
                        .description("The month of the datetime.")
                })
                .field("day", |f| {
                    f.typ("number")
                        .description("The day of the datetime.")
                })
                .field("hour", |f| {
                    f.typ("number")
                        .description("The hour of the datetime.")
                })
                .field("minute", |f| {
                    f.typ("number")
                        .description("The minute of the datetime.")
                })
                .field("second", |f| {
                    f.typ("number")
                        .description("The second of the datetime.")
                })
                .field("timestamp_seconds", |f| {
                    f.typ("number")
                        .description("The timestamp in seconds of the datetime from the Unix epoch.")
                })
                .field("timestamp_millis", |f| {
                    f.typ("number")
                        .description("The timestamp in milliseconds of the datetime from the Unix epoch.")
                })
                .field("timestamp_micros", |f| {
                    f.typ("number")
                        .description("The timestamp in microseconds of the datetime from the Unix epoch.")
                })
                .field("timestamp_nanos", |f| {
                    f.typ("number")
                        .description("The timestamp in nanoseconds of the datetime from the Unix epoch.")
                })
                .field("tz", |f| {
                    f.typ("Timezone")
                        .description("The timezone of the datetime.")
                })
                .field("offset", |f| {
                    f.typ("TimeDelta")
                        .description("The offset of the datetime.")
                })
        })
        .method_mut("new", |m| {
            m.description("Returns a new Timezone object if the timezone is recognized/supported.")
                .parameter("timezone", |p| {
                    p.typ("string")
                        .description("The timezone to get the offset for.")
                })
                .return_("tzobj", |r| {
                    r.typ("Timezone")
                        .description("The timezone userdata object.")
                })
        })
        .field_mut("UTC", |f| {
            f.typ("Timezone")
                .description("The UTC timezone object for simple access to UTC datetime conversions.")
        })
        .method_mut("timedelta_weeks", |m| {
            m.description("Creates a new TimeDelta object with the specified number of weeks.")
                .parameter("weeks", |p| {
                    p.typ("number")
                        .description("The number of weeks.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_days", |m| {
            m.description("Creates a new TimeDelta object with the specified number of days.")
                .parameter("days", |p| {
                    p.typ("number")
                        .description("The number of days.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_hours", |m| {
            m.description("Creates a new TimeDelta object with the specified number of hours.")
                .parameter("hours", |p| {
                    p.typ("number")
                        .description("The number of hours.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_minutes", |m| {
            m.description("Creates a new TimeDelta object with the specified number of minutes.")
                .parameter("minutes", |p| {
                    p.typ("number")
                        .description("The number of minutes.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_seconds", |m| {
            m.description("Creates a new TimeDelta object with the specified number of seconds.")
                .parameter("seconds", |p| {
                    p.typ("number")
                        .description("The number of seconds.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_millis", |m| {
            m.description("Creates a new TimeDelta object with the specified number of milliseconds.")
                .parameter("millis", |p| {
                    p.typ("number")
                        .description("The number of milliseconds.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_micros", |m| {
            m.description("Creates a new TimeDelta object with the specified number of microseconds.")
                .parameter("micros", |p| {
                    p.typ("number")
                        .description("The number of microseconds.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
        .method_mut("timedelta_nanos", |m| {
            m.description("Creates a new TimeDelta object with the specified number of nanoseconds.")
                .parameter("nanos", |p| {
                    p.typ("number")
                        .description("The number of nanoseconds.")
                })
                .return_("td", |r| {
                    r.typ("TimeDelta")
                        .description("The TimeDelta object.")
                })
        })
}

pub struct TimeDelta {
    pub timedelta: chrono::TimeDelta,
}

impl LuaUserData for TimeDelta {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(this.timedelta.to_string())
        });

        methods.add_meta_method(
            LuaMetaMethod::Eq,
            |_, this, other: LuaUserDataRef<TimeDelta>| Ok(this.timedelta == other.timedelta),
        );

        methods.add_meta_method(
            LuaMetaMethod::Add,
            |_, this, other: LuaUserDataRef<TimeDelta>| {
                Ok(TimeDelta {
                    timedelta: this.timedelta + other.timedelta,
                })
            },
        );

        methods.add_meta_method(
            LuaMetaMethod::Sub,
            |_, this, other: LuaUserDataRef<TimeDelta>| {
                Ok(TimeDelta {
                    timedelta: this.timedelta - other.timedelta,
                })
            },
        );

        methods.add_meta_method(
            LuaMetaMethod::Le,
            |_, this, other: LuaUserDataRef<TimeDelta>| Ok(this.timedelta <= other.timedelta),
        );

        methods.add_meta_method(
            LuaMetaMethod::Lt,
            |_, this, other: LuaUserDataRef<TimeDelta>| Ok(this.timedelta < other.timedelta),
        );

        methods.add_meta_method(LuaMetaMethod::Type, |_, _, _: ()| {
            Ok("TimeDelta".to_string())
        });

        methods.add_method("offset_string", |_, this, ()| {
            Ok(format!(
                "{}{:02}:{:02}",
                if this.timedelta.num_seconds() < 0 {
                    "-"
                } else {
                    "+"
                },
                this.timedelta.num_hours(),
                this.timedelta.num_minutes() % 60
            ))
        });
    }

    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("nanos", |_, this| Ok(this.timedelta.num_nanoseconds()));
        fields.add_field_method_get("micros", |_, this| Ok(this.timedelta.num_microseconds()));
        fields.add_field_method_get("millis", |_, this| Ok(this.timedelta.num_milliseconds()));
        fields.add_field_method_get("seconds", |_, this| Ok(this.timedelta.num_seconds()));
        fields.add_field_method_get("minutes", |_, this| Ok(this.timedelta.num_minutes()));
        fields.add_field_method_get("hours", |_, this| Ok(this.timedelta.num_hours()));
        fields.add_field_method_get("days", |_, this| Ok(this.timedelta.num_days()));
        fields.add_field_method_get("weeks", |_, this| Ok(this.timedelta.num_weeks()));
    }
}

pub struct DateTime<Tz>
where
    Tz: chrono::TimeZone + 'static + From<chrono_tz::Tz>,
    chrono_tz::Tz: From<Tz>,
    Tz::Offset: std::fmt::Display,
{
    pub dt: chrono::DateTime<Tz>,
}

impl<Tz> LuaUserData for DateTime<Tz>
where
    Tz: chrono::TimeZone + 'static + From<chrono_tz::Tz>,
    chrono_tz::Tz: From<Tz>,
    Tz::Offset: std::fmt::Display,
{
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(this.dt.to_rfc3339())
        });

        methods.add_meta_method(
            LuaMetaMethod::Eq,
            |_, this, other: LuaUserDataRef<DateTime<Tz>>| Ok(this.dt == other.dt),
        );

        methods.add_meta_method(LuaMetaMethod::Add, |_, this, other: LuaValue| match other {
            LuaValue::Integer(i) => Ok(DateTime {
                dt: this.dt.clone() + chrono::Duration::seconds(i as i64),
            }),
            LuaValue::UserData(ud) => {
                if let Ok(td) = ud.borrow::<TimeDelta>() {
                    Ok(DateTime {
                        dt: this.dt.clone() + td.timedelta,
                    })
                } else {
                    Err(mlua::Error::RuntimeError("Invalid userdata type. DateTime can only be added to TimeDelta's or integers".to_string()))
                }
            }
            _ => Err(mlua::Error::RuntimeError("Invalid type".to_string())),
        });

        methods.add_meta_method(LuaMetaMethod::Sub, |_, this, other: LuaValue| match other {
            LuaValue::Integer(i) => Ok(DateTime {
                dt: this.dt.clone() - chrono::Duration::seconds(i as i64),
            }),
            LuaValue::UserData(ud) => {
                if let Ok(td) = ud.borrow::<TimeDelta>() {
                    Ok(DateTime {
                        dt: this.dt.clone() - td.timedelta,
                    })
                } else {
                    Err(mlua::Error::RuntimeError(
                        "Invalid userdata type. DateTime can only subtract TimeDelta's or integers"
                            .to_string(),
                    ))
                }
            }
            _ => Err(mlua::Error::RuntimeError("Invalid type".to_string())),
        });

        methods.add_meta_method(
            LuaMetaMethod::Le,
            |_, this, other: LuaUserDataRef<DateTime<Tz>>| Ok(this.dt <= other.dt),
        );

        methods.add_meta_method(
            LuaMetaMethod::Lt,
            |_, this, other: LuaUserDataRef<DateTime<Tz>>| Ok(this.dt < other.dt),
        );

        methods.add_meta_method(
            LuaMetaMethod::Type,
            |_, _, _: ()| Ok("DateTime".to_string()),
        );

        methods.add_method("with_timezone", |_, this, tz: LuaUserDataRef<Timezone>| {
            Ok(DateTime {
                dt: this.dt.with_timezone(&tz.tz.into()),
            })
        });

        methods.add_method("format", |_, this, format: String| {
            Ok(this.dt.format(&format).to_string())
        });
    }

    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("year", |_, this| Ok(this.dt.year()));
        fields.add_field_method_get("month", |_, this| Ok(this.dt.month()));
        fields.add_field_method_get("day", |_, this| Ok(this.dt.day()));
        fields.add_field_method_get("hour", |_, this| Ok(this.dt.hour()));
        fields.add_field_method_get("minute", |_, this| Ok(this.dt.minute()));
        fields.add_field_method_get("second", |_, this| Ok(this.dt.second()));
        fields.add_field_method_get("timestamp_seconds", |_, this| Ok(this.dt.timestamp()));
        fields.add_field_method_get("timestamp_millis", |_, this| {
            Ok(this.dt.timestamp_millis())
        });
        fields.add_field_method_get("timestamp_micros", |_, this| {
            Ok(this.dt.timestamp_subsec_micros())
        });
        fields.add_field_method_get("timestamp_nanos", |_, this| {
            Ok(this.dt.timestamp_subsec_nanos())
        });
        fields.add_field_method_get("tz", |_, this| {
            Ok(Timezone {
                tz: this.dt.timezone().into(),
            })
        });
        fields.add_field_method_get("offset", |_, this| {
            let tz: chrono_tz::Tz = this.dt.timezone().into();

            let td = tz
                .offset_from_utc_datetime(&this.dt.naive_utc())
                .base_utc_offset();

            Ok(TimeDelta { timedelta: td })
        });
    }
}

pub struct Timezone {
    pub tz: chrono_tz::Tz,
}

impl LuaUserData for Timezone {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(this.tz.name().to_string())
        });

        methods.add_meta_method(
            LuaMetaMethod::Eq,
            |_, this, other: LuaUserDataRef<Timezone>| Ok(this.tz == other.tz),
        );

        // Translates a timestamp in UTC time to a datetime in the said specific timezone
        methods.add_method(
            "utcToTz",
            |_,
             this,
             (year, month, day, hours, minutes, secs, all): (
                i32,
                u32,
                u32,
                u32,
                u32,
                u32,
                Option<bool>,
            )| {
                match chrono_tz::Tz::UTC.with_ymd_and_hms(year, month, day, hours, minutes, secs) {
                    chrono::LocalResult::Single(ymd_hms) => {
                        let ymd_hms = ymd_hms.with_timezone(&this.tz);
                        Ok((Some(DateTime { dt: ymd_hms }), None))
                    }
                    chrono::LocalResult::Ambiguous(tz, t2) => {
                        if all.unwrap_or(false) {
                            let tz = tz.with_timezone(&this.tz);
                            let t2 = t2.with_timezone(&this.tz);
                            Ok((Some(DateTime { dt: tz }), Some(DateTime { dt: t2 })))
                        } else {
                            let tz = tz.with_timezone(&this.tz);
                            Ok((Some(DateTime { dt: tz }), None))
                        }
                    }
                    chrono::LocalResult::None => {
                        Err(mlua::Error::RuntimeError("Invalid date".to_string()))
                    }
                }
            },
        );

        // Translates a timestamp in the specified timezone to a datetime in UTC
        //
        // Returns a tuple of the first offset and second offset (if the time is ambiguous)
        methods.add_method(
            "tzToUtc",
            |_,
             this,
             (year, month, day, hours, minutes, secs, all): (
                i32,
                u32,
                u32,
                u32,
                u32,
                u32,
                Option<bool>,
            )| {
                match this
                    .tz
                    .with_ymd_and_hms(year, month, day, hours, minutes, secs)
                {
                    chrono::LocalResult::Single(ymd_hms) => {
                        let ymd_hms = ymd_hms.with_timezone(&chrono_tz::Tz::UTC);
                        Ok((Some(DateTime { dt: ymd_hms }), None))
                    }
                    chrono::LocalResult::Ambiguous(tz, t2) => {
                        if all.unwrap_or(false) {
                            let tz = tz.with_timezone(&chrono_tz::Tz::UTC);
                            let t2 = t2.with_timezone(&chrono_tz::Tz::UTC);
                            Ok((Some(DateTime { dt: tz }), Some(DateTime { dt: t2 })))
                        } else {
                            let tz = tz.with_timezone(&chrono_tz::Tz::UTC);
                            Ok((Some(DateTime { dt: tz }), None))
                        }
                    }
                    chrono::LocalResult::None => {
                        Err(mlua::Error::RuntimeError("Invalid date".to_string()))
                    }
                }
            },
        );

        // Translates a time of the current day in UTC time to a datetime in the said specific timezone
        //
        // Returns the first offset
        methods.add_method(
            "timeUtcToTz",
            |_, this, (hours, minutes, secs): (u32, u32, u32)| {
                let now = chrono::Utc::now();
                let now = now
                    .with_hour(hours)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_minute(minutes)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_second(secs)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_timezone(&this.tz);
                Ok(DateTime { dt: now })
            },
        );

        // Translates a time of the current day in UTC time to a datetime in the said specific timezone
        //
        // Returns the first offset
        methods.add_method(
            "timeTzToUtc",
            |_, this, (hours, minutes, secs): (u32, u32, u32)| {
                let now = this.tz.from_utc_datetime(&chrono::Utc::now().naive_utc());
                let now = now
                    .with_hour(hours)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_minute(minutes)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_second(secs)
                    .ok_or(mlua::Error::RuntimeError("Invalid time".to_string()))?
                    .with_timezone(&chrono_tz::Tz::UTC);

                Ok(DateTime { dt: now })
            },
        );

        // Translates the current timestamp to a datetime in the said specific timezone
        //
        // Returns the first offset
        methods.add_method("date", |_, this, (): ()| {
            let now = chrono::Utc::now();
            let now = now.with_timezone(&this.tz);
            Ok(DateTime { dt: now })
        });
    }
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "new",
        lua.create_function(|_, tz: String| {
            // Map some common timezones automatically
            match tz.as_str() {
                "UTC" => Ok(Timezone { tz: chrono_tz::UTC }),
                "IST" => Ok(Timezone {
                    tz: chrono_tz::Asia::Kolkata, // Most people in India call it IST though...
                }),
                "PST" | "PDT" => Ok(Timezone {
                    tz: chrono_tz::America::Los_Angeles, // Somehow not included in from_str here?
                }),
                _ => {
                    if let Ok(tz) = chrono_tz::Tz::from_str(&tz) {
                        Ok(Timezone { tz })
                    } else {
                        Err(mlua::Error::RuntimeError("Invalid timezone".to_string()))
                    }
                }
            }
        })?,
    )?;

    // The standard UTC timezone
    module.set("UTC", lua.create_userdata(Timezone { tz: chrono_tz::UTC })?)?;

    // Creates a new TimeDelta object
    module.set(
        "timedelta_weeks",
        lua.create_function(|_, weeks: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_weeks(weeks)
                .ok_or(mlua::Error::RuntimeError("Invalid number of weeks".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_days",
        lua.create_function(|_, days: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_days(days)
                .ok_or(mlua::Error::RuntimeError("Invalid number of days".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_hours",
        lua.create_function(|_, hours: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_hours(hours)
                .ok_or(mlua::Error::RuntimeError("Invalid number of hours".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_minutes",
        lua.create_function(|_, minutes: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_minutes(minutes)
                .ok_or(mlua::Error::RuntimeError("Invalid number of minutes".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_seconds",
        lua.create_function(|_, seconds: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_seconds(seconds)
                .ok_or(mlua::Error::RuntimeError("Invalid number of seconds".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_millis",
        lua.create_function(|_, millis: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::try_milliseconds(millis)
                .ok_or(mlua::Error::RuntimeError("Invalid number of milliseconds".to_string()))?,
            })
        })?,
    )?;

    module.set(
        "timedelta_micros",
        lua.create_function(|_, micros: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::microseconds(micros),
            })
        })?,
    )?;

    module.set(
        "timedelta_nanos",
        lua.create_function(|_, nanos: i64| {
            Ok(TimeDelta {
                timedelta: chrono::Duration::nanoseconds(nanos),
            })
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timezone() {
        let lua = Lua::new();
        let module = init_plugin(&lua).unwrap();

        lua.load(
            r#"
            local tz = ...
            local myTz = tz.new("IST")
            assert(myTz:utcToTz(2021, 1, 1, 8, 0, 0):format("%I:%M %p") == "01:30 PM", "0: Basic sanity test failed") -- Should be 1:30 PM IST
            
            -- Should be 1:30 PM IST
            local date = myTz:utcToTz(2021, 1, 1, 8, 0, 0):format("%I%M%S")
            assert(tonumber(date) == 013000, "1: Expected 013000, got " .. date)

            -- The same thing should work with timeUtcToTz
            local time = myTz:timeUtcToTz(8, 0, 0):format("%I%M%S")
            assert(tonumber(time) == 13000, "2: Expected 13000, got " .. time)

            -- Test the other way around (timeUtcToTz)
            local time = myTz:timeTzToUtc(13, 30, 0):format("%I%M%S")
            assert(tonumber(time) == 80000, "3: Expected 80000, got " .. time)

            -- Test the other way around (tzToUtc)
            local date = myTz:tzToUtc(2021, 1, 1, 13, 30, 0):format("%I:%M %p")
            assert(date == "08:00 AM", "4: Expected 08:00 AM, got " .. date)

            -- Test datetime methods
            local date = myTz:utcToTz(2021, 1, 1, 8, 0, 0)
            assert(date.year == 2021, "5: Expected 2021, got " .. date.year)
            assert(date.month == 1, "6: Expected 1, got " .. date.month)
            assert(date.day == 1, "7: Expected 1, got " .. date.day)
            assert(date.hour == 13, "8: Expected 13, got " .. date.hour)
            assert(date.minute == 30, "9: Expected 30, got " .. date.minute)
            assert(date.second == 0, "10: Expected 0, got " .. date.second)
            assert(date.offset:offset_string() == "+05:30", "11: Expected +05:30, got " .. date.offset:offset_string())
            assert(date.offset.seconds == 19800, "11: Expected 19800, got " .. date.offset.seconds)
            assert(date.offset.millis == 19800000, "12: Expected 19800000, got " .. date.offset.millis)
            assert(date.tz == myTz, "12: Expected myTz, got " .. tostring(date.tz))
            
            -- Make a new timedelta object
            local td = tz.timedelta_seconds(10)
            assert(td.seconds == 10, "13: Expected 10, got " .. td.seconds)
            assert(td.millis == 10000, "14: Expected 10000, got " .. td.millis)
            assert(td.minutes == 0, "15: Expected 0, got " .. td.minutes)
            assert(td.hours == 0, "16: Expected 0, got " .. td.hours)
            assert(td.days == 0, "17: Expected 0, got " .. td.days)
            assert(td:offset_string() == "+00:00", "18: Expected +00:00, got " .. td:offset_string())
            assert(date + td == myTz:utcToTz(2021, 1, 1, 8, 0, 10), "19: Expected 2021-01-01T13:30:10+05:30, got " .. tostring(date + td))
            
            local td2 = tz.timedelta_weeks(10)
            assert(td2.weeks == 10, "20: Expected 10, got " .. td2.weeks)
            local date33 = myTz:utcToTz(2021, 1, 1, 8, 0, 0) + td2
            assert(date33:format("%Y-%m-%dT%H:%M:%S%z") == "2021-03-12T13:30:00+0530", "21: Expected 2021-03-12T13:30:00+0530, got " .. date33:format("%Y-%m-%dT%H:%M:%S%z"))

            -- Test with_timezone
            local newDate = date:with_timezone(tz.new("UTC"))
            assert(newDate:format("%Y-%m-%dT%H:%M:%S%z") == "2021-01-01T08:00:00+0000", "20: Expected 2021-01-01T08:00:00+0000, got " .. newDate:format("%Y-%m-%dT%H:%M:%S%z"))
            local newDateInEST = date:with_timezone(tz.new("EST"))
            -- In EST, its 3:00 AM
            assert(newDateInEST:format("%Y-%m-%dT%H:%M:%S%z") == "2021-01-01T03:00:00-0500", "21: Expected 2021-01-01T03:00:00-0500, got " .. newDateInEST:format("%Y-%m-%dT%H:%M:%S%z"))
        "#,
        )
        .call::<()>(module)
        .unwrap();
    }
}
