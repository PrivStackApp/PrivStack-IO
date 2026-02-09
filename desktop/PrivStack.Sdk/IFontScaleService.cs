using System.ComponentModel;

namespace PrivStack.Sdk;

/// <summary>
/// Abstraction over font-scaling for accessibility.
/// </summary>
public interface IFontScaleService : INotifyPropertyChanged
{
    double ScaleMultiplier { get; set; }
    string ScaleDisplayText { get; }
    string CurrentFontFamily { get; set; }
    void Initialize();
    void ReapplyScale();
    double GetScaledSize(double baseSize);
}
