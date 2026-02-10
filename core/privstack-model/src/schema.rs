use serde::{Deserialize, Serialize};

/// Describes an entity type's structure for storage indexing and search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub entity_type: String,
    pub indexed_fields: Vec<IndexedField>,
    pub merge_strategy: MergeStrategy,
}

/// A field extracted from entity JSON for indexing/search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedField {
    /// JSON pointer path (e.g., "/title", "/body", "/tags").
    pub field_path: String,
    pub field_type: FieldType,
    pub searchable: bool,
    /// Vector dimension size. Only meaningful when FieldType is Vector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "dimensions")]
    pub vector_dim: Option<u16>,
    /// Allowed enum values. Only meaningful when FieldType is Enum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "options")]
    pub enum_options: Option<Vec<String>>,
}

impl IndexedField {
    fn simple(path: &str, field_type: FieldType, searchable: bool) -> Self {
        Self {
            field_path: path.into(),
            field_type,
            searchable,
            vector_dim: None,
            enum_options: None,
        }
    }

    /// Shorthand for a searchable text field.
    pub fn text(path: &str, searchable: bool) -> Self {
        Self::simple(path, FieldType::Text, searchable)
    }

    /// Shorthand for a tag array field (always searchable).
    pub fn tag(path: &str) -> Self {
        Self::simple(path, FieldType::Tag, true)
    }

    /// Shorthand for a DateTime field.
    pub fn datetime(path: &str) -> Self {
        Self::simple(path, FieldType::DateTime, false)
    }

    /// Shorthand for a numeric field.
    pub fn number(path: &str) -> Self {
        Self::simple(path, FieldType::Number, false)
    }

    /// Shorthand for a boolean field.
    pub fn bool(path: &str) -> Self {
        Self::simple(path, FieldType::Bool, false)
    }

    /// Shorthand for a vector/embedding field.
    pub fn vector(path: &str, dim: u16) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Vector,
            searchable: false,
            vector_dim: Some(dim),
            enum_options: None,
        }
    }

    /// Shorthand for a CRDT counter field.
    pub fn counter(path: &str) -> Self {
        Self::simple(path, FieldType::Counter, false)
    }

    /// Shorthand for a relation (entity link) field.
    pub fn relation(path: &str) -> Self {
        Self::simple(path, FieldType::Relation, false)
    }

    /// Shorthand for a decimal field.
    pub fn decimal(path: &str) -> Self {
        Self::simple(path, FieldType::Decimal, false)
    }

    /// Shorthand for a JSON blob field.
    pub fn json(path: &str) -> Self {
        Self::simple(path, FieldType::Json, false)
    }

    /// Shorthand for an enum field with fixed options.
    pub fn enumeration(path: &str, options: Vec<String>) -> Self {
        Self {
            field_path: path.into(),
            field_type: FieldType::Enum,
            searchable: false,
            vector_dim: None,
            enum_options: Some(options),
        }
    }

    /// Shorthand for a geographic point field.
    pub fn geo_point(path: &str) -> Self {
        Self::simple(path, FieldType::GeoPoint, false)
    }

    /// Shorthand for a duration field.
    pub fn duration(path: &str) -> Self {
        Self::simple(path, FieldType::Duration, false)
    }
}

/// The data type of an indexed field.
///
/// Vector dimensions and enum options are stored on `IndexedField` rather than
/// inside this enum so the JSON representation matches the C# SDK format:
/// `{"field_type": "vector", "vector_dim": 384}` instead of `{"field_type": {"vector": {"dim": 384}}}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Tag,
    DateTime,
    Number,
    Bool,
    Vector,
    Counter,
    Relation,
    Decimal,
    Json,
    Enum,
    GeoPoint,
    Duration,
}

/// How conflicts are resolved when syncing this entity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Last-writer-wins on the whole document (default, simplest).
    LwwDocument,
    /// Last-writer-wins per top-level field (finer granularity).
    LwwPerField,
    /// Plugin provides a custom merge via `PluginDomainHandler::merge`.
    Custom,
}
