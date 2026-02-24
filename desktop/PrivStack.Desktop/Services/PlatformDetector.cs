using System.Runtime.InteropServices;
using System.Runtime.Intrinsics.X86;
using PrivStack.Desktop.Services.AI;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Detects the current platform, architecture, install format, and hardware capabilities.
/// </summary>
public static class PlatformDetector
{
    public static string GetPlatform()
    {
        if (OperatingSystem.IsWindows()) return "windows";
        if (OperatingSystem.IsLinux()) return "linux";
        if (OperatingSystem.IsMacOS()) return "macos";
        return "unknown";
    }

    public static string GetArch() => RuntimeInformation.OSArchitecture switch
    {
        Architecture.X64 => "x64",
        Architecture.Arm64 => "arm64",
        _ => RuntimeInformation.OSArchitecture.ToString().ToLowerInvariant()
    };

    /// <summary>
    /// Attempts to detect how the app was installed based on process path and environment.
    /// </summary>
    public static string DetectCurrentInstallFormat()
    {
        var processPath = Environment.ProcessPath ?? "";
        var baseDir = AppContext.BaseDirectory;

        if (OperatingSystem.IsWindows())
        {
            // MSIX packages run from WindowsApps
            if (baseDir.Contains("WindowsApps", StringComparison.OrdinalIgnoreCase))
                return "msix";
            return "exe";
        }

        if (OperatingSystem.IsLinux())
        {
            return "deb";
        }

        if (OperatingSystem.IsMacOS())
        {
            return "dmg";
        }

        return "unknown";
    }

    /// <summary>
    /// Returns total physical memory in bytes, or 0 if detection fails.
    /// Uses GC memory info on all platforms (no P/Invoke needed).
    /// </summary>
    public static long GetTotalPhysicalMemoryBytes()
    {
        try
        {
            var gcInfo = GC.GetGCMemoryInfo();
            return gcInfo.TotalAvailableMemoryBytes;
        }
        catch
        {
            return 0;
        }
    }

    /// <summary>
    /// Returns total physical memory in GB (rounded).
    /// </summary>
    public static int GetTotalPhysicalMemoryGb()
    {
        var bytes = GetTotalPhysicalMemoryBytes();
        return bytes > 0 ? (int)Math.Round(bytes / 1_073_741_824.0) : 0;
    }

    /// <summary>
    /// Recommends a local LLM model based on available system RAM.
    /// </summary>
    public static (string ModelId, string Reason) RecommendLocalModel()
    {
        var ramGb = GetTotalPhysicalMemoryGb();

        if (ramGb >= 32)
            return ("qwen-2.5-32b", $"Your system has {ramGb} GB RAM — Qwen 2.5 32B provides the best local quality.");

        if (ramGb >= 16)
            return ("qwen-2.5-14b", $"Your system has {ramGb} GB RAM — Qwen 2.5 14B offers high quality responses.");

        if (ramGb >= 12)
            return ("qwen-2.5-7b", $"Your system has {ramGb} GB RAM — Qwen 2.5 7B offers a good balance of quality and speed.");

        return ("llama-3.2-3b", $"Your system has {ramGb} GB RAM — Llama 3.2 3B is lightweight and runs well on smaller systems.");
    }

    // ── Hardware Assessment ───────────────────────────────────────────

    /// <summary>
    /// Detects available GPU acceleration backend.
    /// </summary>
    public static GpuInfo DetectGpu()
    {
        try
        {
            // Apple Silicon always has Metal
            if (OperatingSystem.IsMacOS() && RuntimeInformation.OSArchitecture == Architecture.Arm64)
                return new GpuInfo(GpuBackend.Metal, "Apple Silicon (Metal)", true);

            if (OperatingSystem.IsLinux())
            {
                // NVIDIA CUDA
                if (File.Exists("/dev/nvidia0"))
                    return new GpuInfo(GpuBackend.Cuda, "NVIDIA GPU (CUDA)", true);

                // AMD ROCm — check DRM vendor ID 0x1002
                if (Directory.Exists("/sys/class/drm"))
                {
                    foreach (var card in Directory.GetDirectories("/sys/class/drm", "card*"))
                    {
                        var vendorPath = Path.Combine(card, "device", "vendor");
                        if (File.Exists(vendorPath))
                        {
                            var vendor = File.ReadAllText(vendorPath).Trim();
                            if (vendor == "0x1002")
                                return new GpuInfo(GpuBackend.Rocm, "AMD GPU (ROCm)", true);
                        }
                    }
                }
            }

            if (OperatingSystem.IsWindows())
            {
                var sys32 = Environment.GetFolderPath(Environment.SpecialFolder.System);
                if (File.Exists(Path.Combine(sys32, "nvcuda.dll")))
                    return new GpuInfo(GpuBackend.Cuda, "NVIDIA GPU (CUDA)", true);
                if (File.Exists(Path.Combine(sys32, "amdhip64.dll")))
                    return new GpuInfo(GpuBackend.Rocm, "AMD GPU (ROCm)", true);
            }

            // Intel Arc / unknown integrated
            return new GpuInfo(GpuBackend.None, "No dedicated GPU detected", false);
        }
        catch
        {
            return new GpuInfo(GpuBackend.Unknown, "GPU detection failed", false);
        }
    }

    /// <summary>
    /// Detects CPU capabilities (core count, SIMD support).
    /// </summary>
    public static CpuInfo DetectCpu()
    {
        var cores = Environment.ProcessorCount;
        bool avx2, avx512;

        if (RuntimeInformation.OSArchitecture == Architecture.Arm64)
        {
            return new CpuInfo(cores, false, false,
                $"ARM64 NEON — {cores} cores");
        }

        avx2 = Avx2.IsSupported;
        avx512 = Avx512F.IsSupported;

        var simd = avx512 ? "AVX-512" : avx2 ? "AVX2" : "SSE";
        return new CpuInfo(cores, avx2, avx512,
            $"x64 {simd} — {cores} cores");
    }

    /// <summary>
    /// Returns available (free) system memory in GB.
    /// </summary>
    public static int GetAvailableMemoryGb()
    {
        try
        {
            // Linux: parse /proc/meminfo for MemAvailable
            if (OperatingSystem.IsLinux() && File.Exists("/proc/meminfo"))
            {
                foreach (var line in File.ReadLines("/proc/meminfo"))
                {
                    if (line.StartsWith("MemAvailable:", StringComparison.Ordinal))
                    {
                        var parts = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
                        if (parts.Length >= 2 && long.TryParse(parts[1], out var kb))
                            return (int)Math.Round(kb / 1_048_576.0);
                    }
                }
            }

            // Fallback: GC memory info
            var gcInfo = GC.GetGCMemoryInfo();
            return (int)Math.Round(gcInfo.TotalAvailableMemoryBytes / 1_073_741_824.0);
        }
        catch
        {
            return GetTotalPhysicalMemoryGb(); // worst-case fallback
        }
    }

    /// <summary>
    /// Produces a composite hardware fitness assessment for local AI inference.
    /// Score 0-100, mapped to Green (70+), Yellow (40-69), Red (&lt;40).
    /// </summary>
    public static HardwareReport AssessHardware()
    {
        var totalRam = GetTotalPhysicalMemoryGb();
        var availRam = GetAvailableMemoryGb();
        var gpu = DetectGpu();
        var cpu = DetectCpu();

        var memory = new MemoryInfo(totalRam, availRam,
            $"{totalRam} GB total, {availRam} GB available");

        // Composite scoring
        int ramScore = totalRam switch
        {
            >= 32 => 40,
            >= 16 => 30,
            >= 12 => 20,
            >= 8 => 10,
            _ => 0
        };

        int gpuScore = gpu.Backend switch
        {
            GpuBackend.Cuda or GpuBackend.Metal => 35,
            GpuBackend.Rocm => 30,
            GpuBackend.Unknown => 5,
            _ => 0
        };

        int cpuScore;
        if (cpu.LogicalCoreCount >= 8 && cpu.SupportsAvx2) cpuScore = 25;
        else if (cpu.LogicalCoreCount >= 8) cpuScore = 20;
        else if (cpu.LogicalCoreCount >= 4 && cpu.SupportsAvx2) cpuScore = 15;
        else cpuScore = 5;

        var total = ramScore + gpuScore + cpuScore;
        var tier = total >= 70 ? FitnessTier.Green
                 : total >= 40 ? FitnessTier.Yellow
                 : FitnessTier.Red;

        return new HardwareReport(memory, gpu, cpu, total, tier);
    }

    /// <summary>
    /// Full AI recommendation combining hardware assessment with model and provider suggestions.
    /// </summary>
    public static AiRecommendation GetFullRecommendation()
    {
        var hw = AssessHardware();
        var (modelId, modelReason) = RecommendLocalModel();

        string? recModelId;
        string? recModelReason;
        string? recCloudId;
        string? recCloudReason;
        string summary;

        switch (hw.FitnessTier)
        {
            case FitnessTier.Green:
                recModelId = modelId;
                recModelReason = modelReason;
                recCloudId = null;
                recCloudReason = null;
                summary = $"Your hardware scores {hw.FitnessScore}/100 — local AI runs well on this system.";
                break;

            case FitnessTier.Yellow:
                recModelId = modelId;
                recModelReason = $"{modelReason} Expect slower responses without GPU acceleration.";
                recCloudId = "anthropic";
                recCloudReason = "Anthropic offers zero data retention — a fast, privacy-first alternative to slow local inference.";
                summary = $"Your hardware scores {hw.FitnessScore}/100 — local AI is possible but may be slow. Consider a privacy-first cloud provider for better performance.";
                break;

            default: // Red
                recModelId = null;
                recModelReason = "Local inference is not recommended on this hardware — it would be extremely slow.";
                recCloudId = "anthropic";
                recCloudReason = "Anthropic (zero data retention) or Mistral (EU-based, GDPR-native) provide fast, privacy-respecting cloud inference.";
                summary = $"Your hardware scores {hw.FitnessScore}/100 — a cloud provider is strongly recommended for usable AI performance.";
                break;
        }

        return new AiRecommendation(hw, recModelId, recModelReason, recCloudId, recCloudReason, summary);
    }
}
