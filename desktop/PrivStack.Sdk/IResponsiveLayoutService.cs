using System.ComponentModel;

namespace PrivStack.Sdk;

/// <summary>
/// Layout modes based on content area width (window minus nav sidebar).
/// </summary>
public enum LayoutMode
{
    /// <summary>Width &gt; 1100px — Full multi-column, all panels visible.</summary>
    Wide,

    /// <summary>700–1100px — Standard 2-column, narrower sidebars.</summary>
    Normal,

    /// <summary>&lt; 700px — Sidebars collapse to icon-only or overlay drawers.</summary>
    Compact
}

/// <summary>
/// Observes the content area width and exposes a <see cref="LayoutMode"/> that plugins
/// use to adapt their layouts. Overrides DynamicResource layout values at runtime
/// (same pattern as font scaling).
/// </summary>
public interface IResponsiveLayoutService : INotifyPropertyChanged
{
    /// <summary>Current layout mode derived from content area width.</summary>
    LayoutMode CurrentMode { get; }

    /// <summary>Current content area width in pixels (window width minus nav sidebar).</summary>
    double ContentAreaWidth { get; }

    /// <summary>
    /// Called by the host window when the content area width changes (resize, sidebar toggle).
    /// Debounced internally to prevent resource thrashing during drag-resize.
    /// </summary>
    void UpdateContentAreaWidth(double width);

    /// <summary>
    /// Initializes the service and applies the initial layout mode.
    /// Call after ThemeService and FontScaleService initialization.
    /// </summary>
    void Initialize();

    /// <summary>
    /// Reapplies layout resource overrides for the current mode.
    /// Called by ThemeService after a theme switch resets resources.
    /// </summary>
    void ReapplyLayout();
}
