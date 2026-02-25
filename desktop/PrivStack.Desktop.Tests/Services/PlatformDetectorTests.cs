namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.AI;

public class PlatformDetectorTests
{
    [Fact]
    public void GetPlatform_returns_known_platform()
    {
        var platform = PlatformDetector.GetPlatform();
        platform.Should().BeOneOf("windows", "linux", "macos", "unknown");
    }

    [Fact]
    public void GetArch_returns_known_architecture()
    {
        var arch = PlatformDetector.GetArch();
        arch.Should().NotBeNullOrWhiteSpace();
        // Should be one of the common architectures
        new[] { "x64", "arm64", "x86", "arm" }.Should().Contain(arch);
    }

    [Fact]
    public void DetectCurrentInstallFormat_returns_non_empty_string()
    {
        var format = PlatformDetector.DetectCurrentInstallFormat();
        format.Should().NotBeNullOrWhiteSpace();
    }

    [Fact]
    public void GetTotalPhysicalMemoryBytes_returns_positive_value()
    {
        var bytes = PlatformDetector.GetTotalPhysicalMemoryBytes();
        bytes.Should().BeGreaterThan(0);
    }

    [Fact]
    public void GetTotalPhysicalMemoryGb_returns_reasonable_value()
    {
        var gb = PlatformDetector.GetTotalPhysicalMemoryGb();
        gb.Should().BeGreaterOrEqualTo(1);
        gb.Should().BeLessThan(4096); // Sanity upper bound
    }

    [Fact]
    public void RecommendLocalModel_returns_valid_recommendation()
    {
        var (modelId, reason) = PlatformDetector.RecommendLocalModel();
        modelId.Should().NotBeNullOrWhiteSpace();
        reason.Should().NotBeNullOrWhiteSpace();
        reason.Should().Contain("GB RAM");
    }

    [Fact]
    public void DetectGpu_returns_valid_gpu_info()
    {
        var gpu = PlatformDetector.DetectGpu();
        gpu.Should().NotBeNull();
        gpu.Description.Should().NotBeNullOrWhiteSpace();
    }

    [Fact]
    public void DetectCpu_returns_valid_cpu_info()
    {
        var cpu = PlatformDetector.DetectCpu();
        cpu.Should().NotBeNull();
        cpu.LogicalCoreCount.Should().BeGreaterOrEqualTo(1);
        cpu.Description.Should().NotBeNullOrWhiteSpace();
    }

    [Fact]
    public void GetAvailableMemoryGb_returns_positive_value()
    {
        var gb = PlatformDetector.GetAvailableMemoryGb();
        gb.Should().BeGreaterOrEqualTo(1);
    }

    [Fact]
    public void AssessHardware_returns_valid_report()
    {
        var report = PlatformDetector.AssessHardware();
        report.Should().NotBeNull();
        report.FitnessScore.Should().BeGreaterOrEqualTo(0);
        report.FitnessScore.Should().BeLessOrEqualTo(100);
        report.Memory.Should().NotBeNull();
        report.Gpu.Should().NotBeNull();
        report.Cpu.Should().NotBeNull();
    }

    [Fact]
    public void AssessHardware_fitness_tier_matches_score()
    {
        var report = PlatformDetector.AssessHardware();
        if (report.FitnessScore >= 70)
            report.FitnessTier.Should().Be(FitnessTier.Green);
        else if (report.FitnessScore >= 40)
            report.FitnessTier.Should().Be(FitnessTier.Yellow);
        else
            report.FitnessTier.Should().Be(FitnessTier.Red);
    }

    [Fact]
    public void GetFullRecommendation_returns_valid_recommendation()
    {
        var rec = PlatformDetector.GetFullRecommendation();
        rec.Should().NotBeNull();
        rec.Hardware.Should().NotBeNull();
        rec.Summary.Should().NotBeNullOrWhiteSpace();
        rec.Summary.Should().Contain("/100");
    }
}
