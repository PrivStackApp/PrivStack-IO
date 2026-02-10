using System.Text.Json;
using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public class EntitySchemaTests
{
    [Fact]
    public void IndexedField_Text_CreatesCorrectField()
    {
        var field = IndexedField.Text("/title");

        field.FieldPath.Should().Be("/title");
        field.FieldType.Should().Be(FieldType.Text);
        field.Searchable.Should().BeTrue();
    }

    [Fact]
    public void IndexedField_Text_NonSearchable()
    {
        var field = IndexedField.Text("/body", searchable: false);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Tag_IsSearchable()
    {
        var field = IndexedField.Tag("/tags");

        field.FieldType.Should().Be(FieldType.Tag);
        field.Searchable.Should().BeTrue();
    }

    [Fact]
    public void IndexedField_DateTime_NotSearchable()
    {
        var field = IndexedField.DateTime("/created_at");

        field.FieldType.Should().Be(FieldType.DateTime);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Number_NotSearchable()
    {
        var field = IndexedField.Number("/priority");

        field.FieldType.Should().Be(FieldType.Number);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Bool_NotSearchable()
    {
        var field = IndexedField.Bool("/is_completed");

        field.FieldType.Should().Be(FieldType.Bool);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Vector_StoresDimensions()
    {
        var field = IndexedField.Vector("/embedding", 384);

        field.FieldType.Should().Be(FieldType.Vector);
        field.Dimensions.Should().Be(384);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Relation_NotSearchable()
    {
        var field = IndexedField.Relation("/parent_id");

        field.FieldType.Should().Be(FieldType.Relation);
        field.Searchable.Should().BeFalse();
    }

    [Fact]
    public void IndexedField_Enum_StoresOptions()
    {
        var field = IndexedField.Enum("/status", ["open", "closed", "archived"]);

        field.FieldType.Should().Be(FieldType.Enum);
        field.Options.Should().BeEquivalentTo(new[] { "open", "closed", "archived" });
    }

    [Fact]
    public void IndexedField_Counter_NotSearchable()
    {
        var field = IndexedField.Counter("/view_count");

        field.FieldType.Should().Be(FieldType.Counter);
        field.Searchable.Should().BeFalse();
    }

    [Theory]
    [InlineData(FieldType.Text, "\"text\"")]
    [InlineData(FieldType.Tag, "\"tag\"")]
    [InlineData(FieldType.DateTime, "\"date_time\"")]
    [InlineData(FieldType.Number, "\"number\"")]
    [InlineData(FieldType.Bool, "\"bool\"")]
    [InlineData(FieldType.Vector, "\"vector\"")]
    [InlineData(FieldType.Decimal, "\"decimal\"")]
    [InlineData(FieldType.Relation, "\"relation\"")]
    [InlineData(FieldType.Counter, "\"counter\"")]
    [InlineData(FieldType.Json, "\"json\"")]
    [InlineData(FieldType.Enum, "\"enum\"")]
    [InlineData(FieldType.GeoPoint, "\"geo_point\"")]
    [InlineData(FieldType.Duration, "\"duration\"")]
    public void FieldType_SerializesCorrectly(FieldType fieldType, string expectedJson)
    {
        var json = JsonSerializer.Serialize(fieldType);
        json.Should().Be(expectedJson);
    }

    [Theory]
    [InlineData(MergeStrategy.LwwDocument, "\"lww_document\"")]
    [InlineData(MergeStrategy.LwwPerField, "\"lww_per_field\"")]
    [InlineData(MergeStrategy.Custom, "\"custom\"")]
    public void MergeStrategy_SerializesCorrectly(MergeStrategy strategy, string expectedJson)
    {
        var json = JsonSerializer.Serialize(strategy);
        json.Should().Be(expectedJson);
    }

    [Fact]
    public void EntitySchema_RoundTrips()
    {
        var schema = new EntitySchema
        {
            EntityType = "page",
            IndexedFields = [
                IndexedField.Text("/title"),
                IndexedField.Tag("/tags"),
                IndexedField.DateTime("/updated_at")
            ],
            MergeStrategy = MergeStrategy.LwwPerField
        };

        var json = JsonSerializer.Serialize(schema);
        var deserialized = JsonSerializer.Deserialize<EntitySchema>(json);

        deserialized.Should().NotBeNull();
        deserialized!.EntityType.Should().Be("page");
        deserialized.IndexedFields.Should().HaveCount(3);
        deserialized.MergeStrategy.Should().Be(MergeStrategy.LwwPerField);
    }

    [Fact]
    public void EntitySchema_RoundTrips_WithNewTypes()
    {
        var schema = new EntitySchema
        {
            EntityType = "contact",
            IndexedFields = [
                IndexedField.Text("/name"),
                IndexedField.Relation("/company_id"),
                IndexedField.Vector("/embedding", 384),
                IndexedField.Enum("/status", ["active", "inactive"]),
                IndexedField.Counter("/visit_count"),
                IndexedField.GeoPoint("/location"),
                IndexedField.Duration("/avg_response_time"),
            ],
            MergeStrategy = MergeStrategy.LwwPerField
        };

        var json = JsonSerializer.Serialize(schema);
        var deserialized = JsonSerializer.Deserialize<EntitySchema>(json);

        deserialized.Should().NotBeNull();
        deserialized!.IndexedFields.Should().HaveCount(7);
        deserialized.IndexedFields[2].Dimensions.Should().Be(384);
        deserialized.IndexedFields[3].Options.Should().BeEquivalentTo(new[] { "active", "inactive" });
        // Dimensions/Options should be null when not applicable
        deserialized.IndexedFields[0].Dimensions.Should().BeNull();
        deserialized.IndexedFields[0].Options.Should().BeNull();
    }

    [Fact]
    public void FieldType_DeserializesFromSnakeCase()
    {
        var result = JsonSerializer.Deserialize<FieldType>("\"date_time\"");
        result.Should().Be(FieldType.DateTime);
    }

    [Fact]
    public void MergeStrategy_DeserializesFromSnakeCase()
    {
        var result = JsonSerializer.Deserialize<MergeStrategy>("\"lww_per_field\"");
        result.Should().Be(MergeStrategy.LwwPerField);
    }
}
