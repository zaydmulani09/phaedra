#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    U8,
    U16Be,
    U16Le,
    U32Be,
    U32Le,
    U64Be,
    U64Le,
    Bytes,
    Cstring,
    LpBytes8,
    LpBytes16Be,
    LpBytes32Be,
    Repeated,
    Magic,
    Padding,
}

impl FieldType {
    pub fn min_size(&self) -> usize {
        match self {
            FieldType::U8 => 1,
            FieldType::U16Be | FieldType::U16Le => 2,
            FieldType::U32Be | FieldType::U32Le => 4,
            FieldType::U64Be | FieldType::U64Le => 8,
            FieldType::Bytes => 0,
            FieldType::Cstring => 1,
            FieldType::LpBytes8 => 1,
            FieldType::LpBytes16Be => 2,
            FieldType::LpBytes32Be => 4,
            FieldType::Repeated => 0,
            FieldType::Magic => 0,
            FieldType::Padding => 0,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            FieldType::U8
                | FieldType::U16Be
                | FieldType::U16Le
                | FieldType::U32Be
                | FieldType::U32Le
                | FieldType::U64Be
                | FieldType::U64Le
        )
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub length: Option<usize>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default = "default_true")]
    pub mutable: bool,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Schema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub fields: Vec<Field>,
}
