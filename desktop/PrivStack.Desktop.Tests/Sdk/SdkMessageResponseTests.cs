namespace PrivStack.Desktop.Tests.Sdk;

using System.Text.Json;
using PrivStack.Sdk;

public class SdkMessageResponseTests
{
    // =========================================================================
    // SdkAction serialization
    // =========================================================================

    [Theory]
    [InlineData(SdkAction.Create, "create")]
    [InlineData(SdkAction.Read, "read")]
    [InlineData(SdkAction.ReadList, "read_list")]
    [InlineData(SdkAction.Count, "count")]
    [InlineData(SdkAction.Update, "update")]
    [InlineData(SdkAction.Delete, "delete")]
    [InlineData(SdkAction.Query, "query")]
    [InlineData(SdkAction.Command, "command")]
    [InlineData(SdkAction.Trash, "trash")]
    [InlineData(SdkAction.Restore, "restore")]
    [InlineData(SdkAction.Link, "link")]
    [InlineData(SdkAction.Unlink, "unlink")]
    [InlineData(SdkAction.GetLinks, "get_links")]
    public void SdkAction_serializes_as_snake_case(SdkAction action, string expected)
    {
        var json = JsonSerializer.Serialize(action);
        json.Should().Be($"\"{expected}\"");
    }

    [Theory]
    [InlineData("\"create\"", SdkAction.Create)]
    [InlineData("\"read_list\"", SdkAction.ReadList)]
    [InlineData("\"get_links\"", SdkAction.GetLinks)]
    public void SdkAction_deserializes_from_snake_case(string json, SdkAction expected)
    {
        var result = JsonSerializer.Deserialize<SdkAction>(json);
        result.Should().Be(expected);
    }

    // =========================================================================
    // SdkMessage serialization
    // =========================================================================

    [Fact]
    public void SdkMessage_serializes_correctly()
    {
        var msg = new SdkMessage
        {
            PluginId = "privstack.notes",
            Action = SdkAction.Create,
            EntityType = "page",
            EntityId = "p-123",
            Payload = "{\"title\":\"Test\"}"
        };

        var json = JsonSerializer.Serialize(msg);
        json.Should().Contain("\"plugin_id\":\"privstack.notes\"");
        json.Should().Contain("\"action\":\"create\"");
        json.Should().Contain("\"entity_type\":\"page\"");
        json.Should().Contain("\"entity_id\":\"p-123\"");
    }

    [Fact]
    public void SdkMessage_suppress_change_notification_not_serialized()
    {
        var msg = new SdkMessage
        {
            PluginId = "test",
            Action = SdkAction.Update,
            EntityType = "page",
            SuppressChangeNotification = true
        };

        var json = JsonSerializer.Serialize(msg);
        json.Should().NotContain("SuppressChangeNotification");
        json.Should().NotContain("suppress");
    }

    [Fact]
    public void SdkMessage_optional_fields_null_when_unset()
    {
        var msg = new SdkMessage
        {
            PluginId = "test",
            Action = SdkAction.ReadList,
            EntityType = "task"
        };
        msg.EntityId.Should().BeNull();
        msg.Payload.Should().BeNull();
        msg.Parameters.Should().BeNull();
    }

    [Fact]
    public void SdkMessage_with_parameters()
    {
        var msg = new SdkMessage
        {
            PluginId = "test",
            Action = SdkAction.ReadList,
            EntityType = "task",
            Parameters = new Dictionary<string, string>
            {
                ["status"] = "active",
                ["limit"] = "50"
            }
        };

        var json = JsonSerializer.Serialize(msg);
        json.Should().Contain("\"parameters\"");
        json.Should().Contain("\"status\":\"active\"");
    }

    // =========================================================================
    // SdkResponse
    // =========================================================================

    [Fact]
    public void SdkResponse_Ok_factory()
    {
        var response = SdkResponse.Ok();
        response.Success.Should().BeTrue();
        response.ErrorCode.Should().BeNull();
        response.ErrorMessage.Should().BeNull();
    }

    [Fact]
    public void SdkResponse_Fail_factory()
    {
        var response = SdkResponse.Fail("NOT_FOUND", "Entity not found");
        response.Success.Should().BeFalse();
        response.ErrorCode.Should().Be("NOT_FOUND");
        response.ErrorMessage.Should().Be("Entity not found");
    }

    [Fact]
    public void SdkResponse_serialization_roundtrip()
    {
        var response = SdkResponse.Fail("ERR", "Something broke");
        var json = JsonSerializer.Serialize(response);
        var deserialized = JsonSerializer.Deserialize<SdkResponse>(json);

        deserialized!.Success.Should().BeFalse();
        deserialized.ErrorCode.Should().Be("ERR");
        deserialized.ErrorMessage.Should().Be("Something broke");
    }

    // =========================================================================
    // SdkResponse<T>
    // =========================================================================

    [Fact]
    public void SdkResponseT_Ok_factory()
    {
        var response = SdkResponse<int>.Ok(42);
        response.Success.Should().BeTrue();
        response.Data.Should().Be(42);
    }

    [Fact]
    public void SdkResponseT_Fail_factory()
    {
        var response = SdkResponse<string>.Fail("ERR", "Failed");
        response.Success.Should().BeFalse();
        response.Data.Should().BeNull();
    }

    [Fact]
    public void SdkResponseT_with_complex_data()
    {
        var data = new List<string> { "a", "b", "c" };
        var response = SdkResponse<List<string>>.Ok(data);
        response.Data.Should().HaveCount(3);
    }

    // =========================================================================
    // CountResponse
    // =========================================================================

    [Fact]
    public void CountResponse_serialization()
    {
        var json = """{"count": 42}""";
        var response = JsonSerializer.Deserialize<CountResponse>(json);
        response!.Count.Should().Be(42);
    }
}
