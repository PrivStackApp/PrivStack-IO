namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services.Ipc;

public class IpcProtocolTests
{
    [Fact]
    public void Constants_are_correct()
    {
        IpcProtocol.MaxMessageSize.Should().Be(16 * 1024 * 1024);
        IpcProtocol.LengthPrefixSize.Should().Be(4);
        IpcProtocol.PipePrefix.Should().Be("privstack-ipc-");
    }

    [Fact]
    public void GetPipeName_starts_with_prefix()
    {
        var name = IpcProtocol.GetPipeName("/some/workspace/path");
        name.Should().StartWith("privstack-ipc-");
    }

    [Fact]
    public void GetPipeName_is_deterministic()
    {
        var a = IpcProtocol.GetPipeName("/workspace/data");
        var b = IpcProtocol.GetPipeName("/workspace/data");
        a.Should().Be(b);
    }

    [Fact]
    public void GetPipeName_different_paths_different_names()
    {
        var a = IpcProtocol.GetPipeName("/workspace/one");
        var b = IpcProtocol.GetPipeName("/workspace/two");
        a.Should().NotBe(b);
    }

    [Fact]
    public void GetPipeName_uses_lowercase_hex()
    {
        var name = IpcProtocol.GetPipeName("/test");
        var suffix = name["privstack-ipc-".Length..];
        suffix.Should().MatchRegex("^[0-9a-f]{12}$");
    }

    [Fact]
    public void GetPipeName_hash_suffix_is_12_chars()
    {
        var name = IpcProtocol.GetPipeName("/any/path");
        var suffix = name["privstack-ipc-".Length..];
        suffix.Should().HaveLength(12);
    }
}
