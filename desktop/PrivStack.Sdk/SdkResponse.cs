using System.Text.Json.Serialization;

namespace PrivStack.Sdk;

/// <summary>
/// Data payload returned by the count action.
/// </summary>
public record CountResponse
{
    [JsonPropertyName("count")]
    public int Count { get; init; }
}

/// <summary>
/// Response from an SDK operation with no data payload.
/// </summary>
public record SdkResponse
{
    [JsonPropertyName("success")]
    public bool Success { get; init; }

    [JsonPropertyName("error_code")]
    public string? ErrorCode { get; init; }

    [JsonPropertyName("error_message")]
    public string? ErrorMessage { get; init; }

    public static SdkResponse Ok() => new() { Success = true };

    public static SdkResponse Fail(string code, string message) =>
        new() { Success = false, ErrorCode = code, ErrorMessage = message };
}

/// <summary>
/// Response from an SDK operation with a typed data payload.
/// </summary>
public record SdkResponse<T> : SdkResponse
{
    [JsonPropertyName("data")]
    public T? Data { get; init; }

    public static SdkResponse<T> Ok(T data) => new() { Success = true, Data = data };

    public new static SdkResponse<T> Fail(string code, string message) =>
        new() { Success = false, ErrorCode = code, ErrorMessage = message };
}
