using System.Text.Json.Serialization;

namespace PrivStack.Desktop.Models;

/// <summary>
/// Response from GET /api/releases/latest (public, no auth required).
/// </summary>
public record LatestReleaseInfo
{
    [JsonPropertyName("version")]
    public string Version { get; init; } = string.Empty;

    [JsonPropertyName("published_at")]
    public string? PublishedAt { get; init; }

    [JsonPropertyName("platforms")]
    public List<ReleasePlatformInfo> Platforms { get; init; } = [];
}

/// <summary>
/// Per-platform artifact metadata within a release.
/// </summary>
public record ReleasePlatformInfo
{
    [JsonPropertyName("platform")]
    public string Platform { get; init; } = string.Empty;

    [JsonPropertyName("arch")]
    public string Arch { get; init; } = string.Empty;

    [JsonPropertyName("format")]
    public string Format { get; init; } = string.Empty;

    [JsonPropertyName("size_bytes")]
    public long SizeBytes { get; init; }
}

/// <summary>
/// Response from GET /api/account/releases (authenticated).
/// </summary>
public record AccountReleasesResponse
{
    [JsonPropertyName("version")]
    public string Version { get; init; } = string.Empty;

    [JsonPropertyName("releases")]
    public List<AccountReleaseInfo> Releases { get; init; } = [];
}

/// <summary>
/// Individual downloadable release artifact with checksum.
/// </summary>
public record AccountReleaseInfo
{
    [JsonPropertyName("id")]
    public int Id { get; init; }

    [JsonPropertyName("platform")]
    public string Platform { get; init; } = string.Empty;

    [JsonPropertyName("arch")]
    public string Arch { get; init; } = string.Empty;

    [JsonPropertyName("format")]
    public string Format { get; init; } = string.Empty;

    [JsonPropertyName("filename")]
    public string Filename { get; init; } = string.Empty;

    [JsonPropertyName("size_bytes")]
    public long SizeBytes { get; init; }

    [JsonPropertyName("checksum_sha256")]
    public string? ChecksumSha256 { get; init; }

    [JsonPropertyName("download_url")]
    public string DownloadUrl { get; init; } = string.Empty;
}
