namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class MasterPasswordCacheTests
{
    [Fact]
    public void Initially_has_no_cached_password()
    {
        using var cache = new MasterPasswordCache();
        cache.HasCachedPassword.Should().BeFalse();
        cache.Get().Should().BeNull();
    }

    [Fact]
    public void Set_and_Get_roundtrip()
    {
        using var cache = new MasterPasswordCache();
        cache.Set("MyS3cretP@ss");
        cache.HasCachedPassword.Should().BeTrue();
        cache.Get().Should().Be("MyS3cretP@ss");
    }

    [Fact]
    public void Set_overwrites_previous()
    {
        using var cache = new MasterPasswordCache();
        cache.Set("first");
        cache.Set("second");
        cache.Get().Should().Be("second");
    }

    [Fact]
    public void Clear_removes_cached_password()
    {
        using var cache = new MasterPasswordCache();
        cache.Set("password");
        cache.Clear();
        cache.HasCachedPassword.Should().BeFalse();
        cache.Get().Should().BeNull();
    }

    [Fact]
    public void Clear_is_idempotent()
    {
        using var cache = new MasterPasswordCache();
        cache.Set("password");
        cache.Clear();
        cache.Clear(); // should not throw
        cache.HasCachedPassword.Should().BeFalse();
    }

    [Fact]
    public void Dispose_clears_password()
    {
        var cache = new MasterPasswordCache();
        cache.Set("password");
        cache.Dispose();
        cache.HasCachedPassword.Should().BeFalse();
    }

    [Fact]
    public void Dispose_is_idempotent()
    {
        var cache = new MasterPasswordCache();
        cache.Set("password");
        cache.Dispose();
        cache.Dispose(); // should not throw
    }

    [Fact]
    public void Set_after_clear_works()
    {
        using var cache = new MasterPasswordCache();
        cache.Set("first");
        cache.Clear();
        cache.Set("second");
        cache.Get().Should().Be("second");
    }
}
