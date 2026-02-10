using System.Text.Json;
using PrivStack.Sdk;

namespace PrivStack.Desktop.Tests.Sdk;

public class SdkMessageSerializationTests
{
    [Fact]
    public void SdkMessage_Serializes_WithSnakeCase()
    {
        var msg = new SdkMessage
        {
            PluginId = "privstack.notes",
            Action = SdkAction.Create,
            EntityType = "page",
            EntityId = "abc-123",
            Payload = """{"title":"Hello"}"""
        };

        var json = JsonSerializer.Serialize(msg);

        json.Should().Contain("\"plugin_id\"");
        json.Should().Contain("\"entity_type\"");
        json.Should().Contain("\"entity_id\"");
        json.Should().Contain("\"action\"");
    }

    [Fact]
    public void SdkMessage_RoundTrips()
    {
        var msg = new SdkMessage
        {
            PluginId = "privstack.notes",
            Action = SdkAction.ReadList,
            EntityType = "page",
            Parameters = new Dictionary<string, string> { ["sort"] = "title" }
        };

        var json = JsonSerializer.Serialize(msg);
        var deserialized = JsonSerializer.Deserialize<SdkMessage>(json);

        deserialized.Should().NotBeNull();
        deserialized!.PluginId.Should().Be("privstack.notes");
        deserialized.Action.Should().Be(SdkAction.ReadList);
        deserialized.EntityType.Should().Be("page");
        deserialized.Parameters.Should().ContainKey("sort");
    }

    [Theory]
    [InlineData(SdkAction.Create, "create")]
    [InlineData(SdkAction.ReadList, "read_list")]
    [InlineData(SdkAction.GetLinks, "get_links")]
    [InlineData(SdkAction.Delete, "delete")]
    [InlineData(SdkAction.Query, "query")]
    [InlineData(SdkAction.Trash, "trash")]
    [InlineData(SdkAction.Restore, "restore")]
    [InlineData(SdkAction.Link, "link")]
    [InlineData(SdkAction.Unlink, "unlink")]
    public void SdkAction_SerializesAsSnakeCase(SdkAction action, string expected)
    {
        var json = JsonSerializer.Serialize(action);
        json.Should().Be($"\"{expected}\"");
    }

    [Theory]
    [InlineData("\"read_list\"", SdkAction.ReadList)]
    [InlineData("\"get_links\"", SdkAction.GetLinks)]
    [InlineData("\"create\"", SdkAction.Create)]
    public void SdkAction_DeserializesFromSnakeCase(string json, SdkAction expected)
    {
        var result = JsonSerializer.Deserialize<SdkAction>(json);
        result.Should().Be(expected);
    }

    [Fact]
    public void SdkResponse_Ok_ReturnsSuccess()
    {
        var response = SdkResponse.Ok();

        response.Success.Should().BeTrue();
        response.ErrorCode.Should().BeNull();
        response.ErrorMessage.Should().BeNull();
    }

    [Fact]
    public void SdkResponse_Fail_ReturnsFailure()
    {
        var response = SdkResponse.Fail("NOT_FOUND", "Entity not found");

        response.Success.Should().BeFalse();
        response.ErrorCode.Should().Be("NOT_FOUND");
        response.ErrorMessage.Should().Be("Entity not found");
    }

    [Fact]
    public void SdkResponseT_Ok_ReturnsSuccessWithData()
    {
        var response = SdkResponse<string>.Ok("hello");

        response.Success.Should().BeTrue();
        response.Data.Should().Be("hello");
    }

    [Fact]
    public void SdkResponseT_Fail_ReturnsFailureWithNoData()
    {
        var response = SdkResponse<string>.Fail("ERR", "oops");

        response.Success.Should().BeFalse();
        response.Data.Should().BeNull();
        response.ErrorCode.Should().Be("ERR");
    }

    [Fact]
    public void SdkResponse_RoundTrips()
    {
        var response = SdkResponse.Fail("ERR", "something broke");
        var json = JsonSerializer.Serialize(response);
        var deserialized = JsonSerializer.Deserialize<SdkResponse>(json);

        deserialized!.Success.Should().BeFalse();
        deserialized.ErrorCode.Should().Be("ERR");
        deserialized.ErrorMessage.Should().Be("something broke");
    }
}
