using System.Text.Json.Serialization;

namespace PrivStack.Sdk;

/// <summary>
/// Describes an entity type's structure for storage indexing and search.
/// Plugins declare these to register their entity types with the core engine.
/// </summary>
public sealed record EntitySchema
{
    [JsonPropertyName("entity_type")]
    public required string EntityType { get; init; }

    [JsonPropertyName("indexed_fields")]
    public required IReadOnlyList<IndexedField> IndexedFields { get; init; }

    [JsonPropertyName("merge_strategy")]
    public required MergeStrategy MergeStrategy { get; init; }
}

/// <summary>
/// A field extracted from entity JSON for indexing and search.
/// </summary>
public sealed record IndexedField
{
    /// <summary>JSON pointer path (e.g., "/title", "/body", "/tags").</summary>
    [JsonPropertyName("field_path")]
    public required string FieldPath { get; init; }

    [JsonPropertyName("field_type")]
    public required FieldType FieldType { get; init; }

    [JsonPropertyName("searchable")]
    public required bool Searchable { get; init; }

    /// <summary>Vector dimension size. Only meaningful when FieldType is Vector.</summary>
    [JsonPropertyName("dimensions")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? Dimensions { get; init; }

    /// <summary>Allowed values. Only meaningful when FieldType is Enum.</summary>
    [JsonPropertyName("options")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public IReadOnlyList<string>? Options { get; init; }

    public static IndexedField Text(string path, bool searchable = true) =>
        new() { FieldPath = path, FieldType = FieldType.Text, Searchable = searchable };

    public static IndexedField Tag(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Tag, Searchable = true };

    public static IndexedField DateTime(string path) =>
        new() { FieldPath = path, FieldType = FieldType.DateTime, Searchable = false };

    public static IndexedField Number(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Number, Searchable = false };

    public static IndexedField Bool(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Bool, Searchable = false };

    /// <summary>A high-dimensional vector for AI similarity search.</summary>
    public static IndexedField Vector(string path, int dim) =>
        new() { FieldPath = path, FieldType = FieldType.Vector, Searchable = false, Dimensions = dim };

    /// <summary>A fixed-point decimal for currency/finance.</summary>
    public static IndexedField Decimal(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Decimal, Searchable = false };

    /// <summary>A directed edge to another entity. Auto-populates the graph index.</summary>
    public static IndexedField Relation(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Relation, Searchable = false };

    /// <summary>A distributed counter (PNCounter) for accurate aggregation.</summary>
    public static IndexedField Counter(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Counter, Searchable = false };

    /// <summary>A raw JSON blob field.</summary>
    public static IndexedField Json(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Json, Searchable = false };

    /// <summary>An enum field with fixed allowed values.</summary>
    public static IndexedField Enum(string path, IReadOnlyList<string> options) =>
        new() { FieldPath = path, FieldType = FieldType.Enum, Searchable = false, Options = options };

    /// <summary>A geographic coordinate (lat/lon).</summary>
    public static IndexedField GeoPoint(string path) =>
        new() { FieldPath = path, FieldType = FieldType.GeoPoint, Searchable = false };

    /// <summary>A time duration field.</summary>
    public static IndexedField Duration(string path) =>
        new() { FieldPath = path, FieldType = FieldType.Duration, Searchable = false };
}

/// <summary>The data type of an indexed field.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<FieldType>))]
public enum FieldType
{
    [JsonStringEnumMemberName("text")]
    Text,

    [JsonStringEnumMemberName("tag")]
    Tag,

    [JsonStringEnumMemberName("date_time")]
    DateTime,

    [JsonStringEnumMemberName("number")]
    Number,

    [JsonStringEnumMemberName("bool")]
    Bool,

    [JsonStringEnumMemberName("vector")]
    Vector,

    [JsonStringEnumMemberName("decimal")]
    Decimal,

    [JsonStringEnumMemberName("relation")]
    Relation,

    [JsonStringEnumMemberName("counter")]
    Counter,

    [JsonStringEnumMemberName("json")]
    Json,

    [JsonStringEnumMemberName("enum")]
    Enum,

    [JsonStringEnumMemberName("geo_point")]
    GeoPoint,

    [JsonStringEnumMemberName("duration")]
    Duration
}

/// <summary>How conflicts are resolved when syncing this entity type.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<MergeStrategy>))]
public enum MergeStrategy
{
    [JsonStringEnumMemberName("lww_document")]
    LwwDocument,

    [JsonStringEnumMemberName("lww_per_field")]
    LwwPerField,

    [JsonStringEnumMemberName("custom")]
    Custom
}
