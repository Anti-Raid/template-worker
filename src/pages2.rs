use crate::Error;
use serde_json::{Number, Value};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum ColumnType {
    /// A single valued column (scalar)
    Scalar {
        /// The value type
        inner: InnerColumnType,
    },
    /// An array column
    Array {
        /// The inner type of the array
        inner: InnerColumnType,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum InnerColumnType {
    String {
        min_length: Option<usize>,
        max_length: Option<usize>,
        allowed_values: Vec<String>, // If empty, all values are allowed
        kind: String, // e.g. uuid, textarea, channel, user, role, interval, timestamp etc.
    },
    Integer {},
    Float {},
    BitFlag {
        /// The bit flag values
        values: indexmap::IndexMap<String, i64>,
    },
    Boolean {},
    Json {
        kind: String, // e.g. templateref etc.
        max_bytes: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColumnSuggestion {
    Static { suggestions: Vec<String> },
    None {},
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    /// The ID of the column on the database
    pub id: String,

    /// The friendly name of the column
    pub name: String,

    /// The description of the column
    pub description: String,

    /// The type of the column
    pub column_type: ColumnType,

    /// Whether or not the column is a primary key
    pub primary_key: bool,

    /// Whether or not the column is nullable
    ///
    /// Note that the point where nullability is checked may vary but will occur after pre_checks are executed
    pub nullable: bool,

    /// Suggestions to display
    pub suggestions: ColumnSuggestion,

    /// A secret field that is not shown to the user
    pub secret: bool,

    /// For which operations should the field be ignored for (essentially, read only)
    ///
    /// Semantics are defined by the Executor
    pub ignored_for: Vec<OperationType>,
}

impl PartialEq for Column {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum OperationType {
    View,
    Create,
    Update,
    Delete,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::View => write!(f, "View"),
            OperationType::Create => write!(f, "Create"),
            OperationType::Update => write!(f, "Update"),
            OperationType::Delete => write!(f, "Delete"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Setting {
    /// The ID of the option
    pub id: String,

    /// The name of the option
    pub name: String,

    /// The description of the option
    pub description: String,

    /// Title template, used for the title of the embed
    pub title_template: String,

    /// The columns for this option
    pub columns: Vec<Column>,

    /// The supported operations for this option
    pub operations: Vec<OperationType>,
}

/// Parse a value against the schema's column type
pub fn validate_value(
    v: Value,
    column_type: &ColumnType,
    column_id: &str,
    nullable: bool,
) -> Result<Value, Error> {
    if v == Value::Null {
        if !nullable {
            return Err(format!(
                "Validation error in column {}, expected non-nullable value but got null",
                column_id
            )
            .into());
        } else {
            return Ok(Value::Null);
        }
    }

    match &column_type {
        ColumnType::Scalar { inner } => {
            // Special case: JSON columns can be any type
            if matches!(v, Value::Array(_)) && !matches!(inner, InnerColumnType::Json { .. }) {
                return Err(format!(
                    "Validation error in column {}, expected scalar but got array",
                    column_id
                )
                .into());
            }

            match inner {
                InnerColumnType::String {
                    min_length,
                    max_length,
                    allowed_values,
                    ..
                } => match v {
                    Value::String(s) => {
                        if let Some(min_length) = min_length {
                            if s.len() < *min_length {
                                return Err(format!(
                                    "Validation error in column {}, expected String with min length {} but got String with length {}",
                                    column_id, min_length, s.len()
                                )
                                .into());
                            }
                        }

                        if let Some(max_length) = max_length {
                            if s.len() > *max_length {
                                return Err(format!(
                                    "Validation error in column {}, expected String with max length {} but got String with length {}",
                                    column_id, max_length, s.len()
                                )
                                .into());
                            }
                        }

                        if !allowed_values.is_empty() && !allowed_values.contains(&s) {
                            return Err(format!(
                                "Validation error in column {}, expected String with value in {:?} but got String with value {}",
                                column_id, allowed_values, s
                            )
                            .into());
                        }

                        Ok(Value::String(s))
                    }
                    _ => Err(format!(
                        "Validation error in column {}, expected String but got {:?}",
                        column_id, v
                    )
                    .into()),
                },
                InnerColumnType::Integer {} => match v {
                    Value::String(s) => {
                        if s.is_empty() {
                            Err(format!(
                                "Validation error in column {}, expected Integer but got empty String",
                                column_id
                            ).into())
                        } else {
                            let value = match s.parse::<i64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    return Err(format!(
                                        "Validation error in column {}, expected Integer but got String that cannot be parsed: {}",
                                        column_id, e
                                    )
                                    .into());
                                }
                            };

                            Ok(Value::Number(value.into()))
                        }
                    }
                    Value::Number(v) => {
                        if v.is_i64() {
                            Ok(Value::Number(v))
                        } else {
                            Err(format!(
                                "Validation error in column {}, expected Integer but got Float",
                                column_id
                            )
                            .into())
                        }
                    }
                    _ => Err(format!(
                        "Validation error in column {}, expected Integer but got {:?}",
                        column_id, v
                    )
                    .into()),
                },
                InnerColumnType::Float {} => match v {
                    Value::String(s) => {
                        let value = match s.parse::<f64>() {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(format!(
                                    "Validation error in column {}, expected Float but got String that cannot be parsed: {}",
                                    column_id, e
                                )
                                .into());
                            }
                        };

                        let number = match Number::from_f64(value) {
                            Some(n) => n,
                            None => {
                                return Err(format!(
                                    "Validation error in column {}, expected Float but got Float that cannot be converted to JSON Number",
                                    column_id
                                )
                                .into());
                            }
                        };

                        Ok(Value::Number(number))
                    }
                    Value::Number(v) => {
                        if v.is_f64() {
                            Ok(Value::Number(v))
                        } else {
                            Err(format!(
                                "Validation error in column {}, expected Float but got Integer",
                                column_id
                            )
                            .into())
                        }
                    }
                    _ => Err(format!(
                        "Validation error in column {}, expected Float but got {:?}",
                        column_id, v
                    )
                    .into()),
                },
                InnerColumnType::BitFlag { values } => {
                    let v = match v {
                        Value::String(s) => match s.parse::<i64>() {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(format!(
                                        "Validation error in column {}, expected BitFlag but got String that cannot be parsed: {}",
                                        column_id, e
                                    )
                                    .into());
                            }
                        },
                        Value::Number(v) => {
                            if v.is_i64() {
                                v.as_i64().unwrap()
                            } else {
                                return Err(format!(
                                    "Validation error in column {}, expected BitFlag but got Float",
                                    column_id
                                )
                                .into());
                            }
                        }
                        _ => {
                            return Err(format!(
                                "Validation error in column {}, expected BitFlag but got {:?}",
                                column_id, v
                            )
                            .into())
                        }
                    };

                    let mut final_value = 0;

                    // Set all the valid bits in final_value to ensure no unknown bits are being set
                    for (_, bit) in values.iter() {
                        if *bit & v == *bit {
                            final_value |= *bit;
                        }
                    }

                    if final_value == 0 {
                        // Set the first value as the default value
                        let Some(fv) = values.values().next() else {
                            return Err(
                                format!(
                                    "Validation error in column {}, expected BitFlag but no default value found",
                                    column_id
                                )
                                .into()
                            );
                        };

                        final_value = *fv;
                    }

                    Ok(Value::Number(final_value.into()))
                }
                InnerColumnType::Boolean {} => match v {
                    Value::String(s) => {
                        let value = match s.parse::<bool>() {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(format!(
                                    "Validation error in column {}, expected Boolean but got String that cannot be parsed: {}",
                                    column_id, e
                                )
                                .into());
                            }
                        };

                        Ok(Value::Bool(value))
                    }
                    Value::Bool(v) => Ok(Value::Bool(v)),
                    _ => Err(format!(
                        "Validation error in column {}, expected Boolean but got {:?}",
                        column_id, v
                    )
                    .into()),
                },
                InnerColumnType::Json { max_bytes, .. } => {
                    // Convert back to json to get bytes
                    match v {
                        Value::String(s) => {
                            if s.len() > max_bytes.unwrap_or(0) {
                                return Err(
                                    format!(
                                        "Validation error in column {}, expected JSON with max bytes {} but got JSON with bytes {}",
                                        column_id, max_bytes.unwrap_or(0), s.len()
                                    )
                                    .into()
                                );
                            }

                            let v: serde_json::Value = {
                                if !s.starts_with("[") && !s.starts_with("{") {
                                    serde_json::Value::String(s)
                                } else {
                                    match serde_json::from_str(&s) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            return Err(
                                                format!(
                                                    "Validation error in column {}, expected JSON but got String that cannot be parsed: {}",
                                                    column_id, e
                                                )
                                                .into()
                                            );
                                        }
                                    }
                                }
                            };

                            Ok(v)
                        }
                        _ => {
                            let bytes = match serde_json::to_string(&v) {
                                Ok(b) => b,
                                Err(e) => {
                                    return Err(
                                        format!(
                                            "Validation error in column {}, expected JSON but got value that cannot be converted to JSON: {}",
                                            column_id, e
                                        )
                                        .into()
                                    );
                                }
                            };

                            if let Some(max_bytes) = max_bytes {
                                if bytes.len() > *max_bytes {
                                    return Err(
                                        format!(
                                            "Validation error in column {}, expected JSON with max bytes {} but got JSON with bytes {}",
                                            column_id, max_bytes, bytes.len()
                                        )
                                        .into()
                                    );
                                }
                            }

                            Ok(v)
                        }
                    }
                }
            }
        }
        ColumnType::Array { inner } => match v {
            Value::Array(l) => {
                let mut values: Vec<Value> = Vec::new();

                let column_type = ColumnType::Scalar {
                    inner: inner.clone(),
                };
                for v in l {
                    let new_v = validate_value(v, &column_type, column_id, nullable)?;

                    values.push(new_v);
                }

                Ok(Value::Array(values))
            }
            _ => Err(format!(
                "Validation error in column {}, expected Array but got {:?}",
                column_id, v
            )
            .into()),
        },
    }
}
