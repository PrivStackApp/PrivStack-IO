namespace PrivStack.Services.AI;

/// <summary>
/// GPU acceleration backend detected on the system.
/// </summary>
public enum GpuBackend
{
    None,
    Cuda,
    Rocm,
    Metal,
    Unknown
}

/// <summary>
/// Fitness tier for local AI inference viability.
/// </summary>
public enum FitnessTier
{
    /// <summary>70+ score — local inference runs well.</summary>
    Green,

    /// <summary>40-69 score — local inference possible but slow; cloud recommended.</summary>
    Yellow,

    /// <summary>&lt;40 score — local inference impractical; cloud strongly recommended.</summary>
    Red
}

/// <summary>
/// Detected GPU capabilities.
/// </summary>
public sealed record GpuInfo(GpuBackend Backend, string Description, bool IsAccelerated);

/// <summary>
/// Detected CPU capabilities.
/// </summary>
public sealed record CpuInfo(int LogicalCoreCount, bool SupportsAvx2, bool SupportsAvx512, string Description);

/// <summary>
/// Detected memory information.
/// </summary>
public sealed record MemoryInfo(int TotalGb, int AvailableGb, string Description);

/// <summary>
/// Composite hardware assessment for local AI fitness.
/// </summary>
public sealed record HardwareReport(
    MemoryInfo Memory,
    GpuInfo Gpu,
    CpuInfo Cpu,
    int FitnessScore,
    FitnessTier FitnessTier);

/// <summary>
/// Full AI recommendation combining hardware assessment with model/provider suggestions.
/// </summary>
public sealed record AiRecommendation(
    HardwareReport Hardware,
    string? RecommendedLocalModelId,
    string? RecommendedLocalReason,
    string? RecommendedCloudProviderId,
    string? RecommendedCloudReason,
    string Summary);
