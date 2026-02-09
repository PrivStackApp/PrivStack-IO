using System.ComponentModel;
using System.Timers;
using Avalonia;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Sdk;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Observes content area width and maintains a <see cref="LayoutMode"/>.
/// Overrides DynamicResource layout values at runtime per mode, then delegates
/// to <see cref="FontScaleService"/> to apply the font-scale multiplier on top.
/// </summary>
public class ResponsiveLayoutService : IResponsiveLayoutService
{
    private static readonly ILogger _log = Log.ForContext<ResponsiveLayoutService>();

    // Breakpoints (content area width, not window width)
    private const double CompactThreshold = 700;
    private const double WideThreshold = 1100;

    // Debounce interval to prevent resource thrashing during resize
    private const double DebounceMs = 100;

    private LayoutMode _currentMode = LayoutMode.Wide;
    private double _contentAreaWidth = 1200;
    private System.Timers.Timer? _debounceTimer;
    private double _pendingWidth;

    private readonly IFontScaleService _fontScaleService;

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>
    /// Singleton accessor for code-behind usage in plugins.
    /// Set once during DI construction (registered as singleton).
    /// </summary>
    public static ResponsiveLayoutService Instance { get; private set; } = null!;

    /// <summary>
    /// Base layout dimensions per mode. FontScaleService reads these as its base values.
    /// </summary>
    private static readonly Dictionary<LayoutMode, Dictionary<string, double>> ModeLayoutSizes = new()
    {
        [LayoutMode.Wide] = new()
        {
            ["ThemeSidebarWidth"] = 260,
            ["ThemeDetailPanelWidth"] = 320,
            ["ThemeSidebarNarrowWidth"] = 200,
            ["ThemeInfoPanelWidth"] = 280,
        },
        [LayoutMode.Normal] = new()
        {
            ["ThemeSidebarWidth"] = 220,
            ["ThemeDetailPanelWidth"] = 280,
            ["ThemeSidebarNarrowWidth"] = 180,
            ["ThemeInfoPanelWidth"] = 240,
        },
        [LayoutMode.Compact] = new()
        {
            ["ThemeSidebarWidth"] = 48,
            ["ThemeDetailPanelWidth"] = 260,
            ["ThemeSidebarNarrowWidth"] = 48,
            ["ThemeInfoPanelWidth"] = 0,
        }
    };

    public LayoutMode CurrentMode
    {
        get => _currentMode;
        private set
        {
            if (_currentMode != value)
            {
                var old = _currentMode;
                _currentMode = value;
                ApplyModeResources();
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(CurrentMode)));
                _log.Information("Layout mode changed: {Old} -> {New} (content width: {Width}px)",
                    old, value, _contentAreaWidth);
            }
        }
    }

    public double ContentAreaWidth
    {
        get => _contentAreaWidth;
        private set
        {
            if (Math.Abs(_contentAreaWidth - value) > 0.5)
            {
                _contentAreaWidth = value;
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ContentAreaWidth)));
            }
        }
    }

    public ResponsiveLayoutService(IFontScaleService fontScaleService)
    {
        _fontScaleService = fontScaleService;
        Instance = this;
    }

    public void Initialize()
    {
        ApplyModeResources();
        _log.Information("Responsive layout service initialized in {Mode} mode (content width: {Width}px)",
            _currentMode, _contentAreaWidth);
    }

    public void UpdateContentAreaWidth(double width)
    {
        if (width <= 0) return;

        _pendingWidth = width;

        if (_debounceTimer == null)
        {
            _debounceTimer = new System.Timers.Timer(DebounceMs);
            _debounceTimer.AutoReset = false;
            _debounceTimer.Elapsed += OnDebounceElapsed;
        }

        _debounceTimer.Stop();
        _debounceTimer.Start();
    }

    public void ReapplyLayout()
    {
        ApplyModeResources();
    }

    private void OnDebounceElapsed(object? sender, ElapsedEventArgs e)
    {
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            ContentAreaWidth = _pendingWidth;
            var newMode = ComputeMode(_pendingWidth);
            if (newMode != _currentMode)
            {
                CurrentMode = newMode;
            }
            else
            {
                // Even if mode didn't change, cap sidebar widths if font scale
                // would cause overflow in Compact mode
                ClampLayoutForFontScale();
            }
        });
    }

    private static LayoutMode ComputeMode(double width) => width switch
    {
        < CompactThreshold => LayoutMode.Compact,
        > WideThreshold => LayoutMode.Wide,
        _ => LayoutMode.Normal
    };

    /// <summary>
    /// Sets DynamicResource overrides for the current mode, then asks FontScaleService
    /// to reapply scaling on top of these new base values.
    /// </summary>
    private void ApplyModeResources()
    {
        var app = Application.Current;
        if (app == null) return;

        if (!ModeLayoutSizes.TryGetValue(_currentMode, out var sizes))
            return;

        try
        {
            // Boolean flags for plugin header compact responsiveness
            app.Resources["ThemeIsCompactMode"] = (_currentMode == LayoutMode.Compact);
            app.Resources["ThemeIsNotCompactMode"] = (_currentMode != LayoutMode.Compact);

            // Feed mode-adjusted bases to FontScaleService before it applies scaling
            ((FontScaleService)_fontScaleService).SetBaseLayoutSizes(sizes);

            // FontScaleService.ReapplyScale() applies the multiplier on top
            _fontScaleService.ReapplyScale();

            // Clamp if needed
            ClampLayoutForFontScale();

            _log.Debug("Applied layout resources for {Mode} mode", _currentMode);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to apply layout resources for {Mode}", _currentMode);
        }
    }

    /// <summary>
    /// In Compact mode with high font scale, cap sidebar width at 40% of content area
    /// to prevent overflow.
    /// </summary>
    private void ClampLayoutForFontScale()
    {
        if (_currentMode != LayoutMode.Compact) return;

        var app = Application.Current;
        if (app == null) return;

        var maxSidebarWidth = _contentAreaWidth * 0.4;
        var scale = _fontScaleService.ScaleMultiplier;

        if (scale > 1.0)
        {
            var currentSidebar = ModeLayoutSizes[LayoutMode.Compact]["ThemeSidebarWidth"] * scale;
            if (currentSidebar > maxSidebarWidth && maxSidebarWidth > 0)
            {
                app.Resources["ThemeSidebarWidth"] = Math.Round(maxSidebarWidth);
                app.Resources["ThemeSidebarNarrowWidth"] = Math.Round(maxSidebarWidth);
            }
        }
    }
}
