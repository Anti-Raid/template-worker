#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
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
    /// A widget
    Widget {
        /// The inner widget type
        inner: InnerWidget,
    },
}

/// Note: this is merely a hint used for styling the website
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum InnerWidget {
    Info {
        /// The info message to display
        message: String,
    },
    Warning {
        /// The warning message to display
        message: String,
    },
    Button {
        /// The label of the button
        label: String,
        /// The operation to perform when the button is clicked
        op: String,
    },
}

/// Note: this is merely a hint used for styling the website
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum InnerColumnType {
    String {
        min_length: Option<usize>,
        max_length: Option<usize>,
        allowed_values: Vec<String>, // If empty, all values are allowed
        suggestions: Option<Vec<String>>, // If set, these are the suggestions for the input
        kind: String, // e.g. uuid, textarea, channel, user, role, interval, timestamp, password etc.
    },
    Integer {},
    Float {},
    BitFlag {
        /// The bit flag values
        values: indexmap::IndexMap<String, i64>,
    },
    Boolean {},
    Json {
        style: String, // e.g. templateref etc.
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
pub struct Column {
    /// The ID of the column on the database
    pub id: String,

    /// The friendly name of the column
    pub name: String,

    /// The description of the column
    pub description: String,

    /// Placeholder text for the column, used in the UI
    pub placeholder: Option<String>,

    /// The type of the column
    pub column_type: ColumnType,

    /// Whether or not the column is a primary key
    pub primary_key: bool,

    /// Whether or not the column is nullable
    pub nullable: bool,

    /// Whether the field should be hidden for the given operations
    #[serde(default)]
    pub hidden: Vec<String>,

    /// Whether the field is readonly for the given operations. Readonly fields may or may not be sent to the server
    pub readonly: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
pub struct Setting {
    /// The ID of the option
    pub id: String,

    /// The name of the option
    pub name: String,

    /// The description of the option
    pub description: String,

    /// View template, used for the title of the embed but can also be used for handling client side errors
    pub view_template: Option<String>,

    /// Index by
    ///
    /// If set, all options within the setting will be draggable with the provided index field (must be
    /// a integer being set to the position of the item in the list)
    pub index_by: Option<String>,

    /// The columns for this option
    pub columns: Vec<Column>,

    /// The supported operations for this option
    pub operations: Vec<String>,

    /// Footer for the setting
    pub footer: Option<Footer>,

    /// Icon to use for the setting
    pub icon: Option<String>,

    /// Client side validation script
    pub validation_template: Option<String>,

    /// Post-send script to run after the setting has been sent to the server
    pub postsend_template: Option<String>,

    /// DEPRACRATED, but still used in production badgerfang (TO REMOVE)
    pub title_template: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, ts_rs::TS)]
#[ts(export)]
pub struct Footer {
    /// The text to display in the footer of the setting as a whole
    pub end_text: String,
}