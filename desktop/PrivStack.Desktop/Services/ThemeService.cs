using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Markup.Xaml;
using Avalonia.Media;
using Avalonia.Styling;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Sdk;
using Serilog;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Available themes in PrivStack.
/// </summary>
public enum AppTheme
{
    Dark,
    Light,
    Sage,
    Lavender,
    Azure,
    Slate,
    Ember,
}

/// <summary>
/// Service for managing application themes at runtime.
/// Handles switching between Dark, Light, and High Contrast themes.
/// Uses direct resource dictionary updates for DynamicResource bindings to work.
/// </summary>
public class ThemeService : IThemeService
{
    private static readonly ILogger _log = Log.ForContext<ThemeService>();

    private AppTheme _currentTheme = AppTheme.Dark;

    public event PropertyChangedEventHandler? PropertyChanged;

    private readonly IAppSettingsService _appSettings;
    private readonly IFontScaleService _fontScaleService;
    private readonly IResponsiveLayoutService _responsiveLayoutService;

    /// <summary>
    /// Gets or sets the current theme.
    /// </summary>
    public AppTheme CurrentTheme
    {
        get => _currentTheme;
        set
        {
            if (_currentTheme != value)
            {
                _currentTheme = value;
                ApplyTheme(value);
                SaveThemePreference(value);
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(CurrentTheme)));
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsDarkTheme)));
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsLightTheme)));
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(IsHighContrastTheme)));
            }
        }
    }

    /// <summary>
    /// Gets whether the current theme is Dark.
    /// </summary>
    public bool IsDarkTheme => _currentTheme == AppTheme.Dark;

    /// <summary>
    /// Gets whether the current theme is Light.
    /// </summary>
    public bool IsLightTheme => _currentTheme == AppTheme.Light;

    /// <summary>
    /// Gets whether the current theme is High Contrast.
    /// </summary>
    public bool IsHighContrastTheme => false;

    public ThemeService(IAppSettingsService appSettings, IFontScaleService fontScaleService,
        IResponsiveLayoutService responsiveLayoutService)
    {
        _appSettings = appSettings;
        _fontScaleService = fontScaleService;
        _responsiveLayoutService = responsiveLayoutService;
        // Load saved theme preference
        LoadThemePreference();
    }

    /// <summary>
    /// Initializes the theme service and applies the saved theme.
    /// Call this after the Application has been initialized.
    /// </summary>
    public void Initialize()
    {
        ApplyTheme(_currentTheme);
        _log.Information("Theme service initialized with {Theme} theme", _currentTheme);
    }

    private void ApplyTheme(AppTheme theme)
    {
        var app = Application.Current;
        if (app == null)
        {
            _log.Warning("Cannot apply theme: Application.Current is null");
            return;
        }

        try
        {
            // Load the theme dictionary
            var themeUri = GetThemeUri(theme);
            var themeDictionary = LoadResourceDictionary(themeUri);

            if (themeDictionary == null)
            {
                _log.Error("Failed to load theme dictionary from {Uri}", themeUri);
                return;
            }

            // Directly update each resource in the app's resource dictionary
            // This triggers DynamicResource bindings to update
            var appResources = app.Resources;
            var updatedCount = 0;

            foreach (var kvp in themeDictionary)
            {
                if (kvp.Key is string key)
                {
                    appResources[key] = kvp.Value;
                    updatedCount++;
                }
            }

            // Update the RequestedThemeVariant for FluentTheme compatibility
            app.RequestedThemeVariant = IsLightThemeVariant(theme) ? ThemeVariant.Light : ThemeVariant.Dark;

            // Reapply responsive layout first (sets mode-adjusted base values),
            // then font scaling applies multiplier on top
            _responsiveLayoutService.ReapplyLayout();

            _log.Information("Applied {Theme} theme successfully ({Count} resources updated)", theme, updatedCount);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to apply {Theme} theme", theme);
        }
    }

    private static ResourceDictionary? LoadResourceDictionary(Uri uri)
    {
        try
        {
            var loaded = AvaloniaXamlLoader.Load(uri);
            if (loaded is ResourceDictionary rd)
            {
                return rd;
            }
            _log.Warning("Loaded resource from {Uri} is not a ResourceDictionary: {Type}", uri, loaded?.GetType().Name);
            return null;
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to load ResourceDictionary from {Uri}", uri);
            return null;
        }
    }

    private static Uri GetThemeUri(AppTheme theme)
    {
        var fileName = theme switch
        {
            AppTheme.Dark => "DarkTheme",
            AppTheme.Light => "LightTheme",
            AppTheme.Sage => "SageTheme",
            AppTheme.Lavender => "LavenderTheme",
            AppTheme.Azure => "AzureTheme",
            AppTheme.Slate => "SlateTheme",
            AppTheme.Ember => "EmberTheme",
            _ => "DarkTheme"
        };

        return new Uri($"avares://PrivStack.Desktop/Styles/Themes/{fileName}.axaml");
    }

    private static bool IsLightThemeVariant(AppTheme theme)
    {
        return theme is AppTheme.Light
            or AppTheme.Sage
            or AppTheme.Lavender
            or AppTheme.Azure;
    }

    private void LoadThemePreference()
    {
        var settings = _appSettings.Settings;
        _currentTheme = Enum.TryParse<AppTheme>(settings.Theme, out var parsed) ? parsed : AppTheme.Dark;
        _log.Debug("Loaded theme preference: {Theme}", _currentTheme);
    }

    private void SaveThemePreference(AppTheme theme)
    {
        var settings = _appSettings.Settings;
        settings.Theme = theme.ToString();
        _appSettings.Save();
        _log.Debug("Saved theme preference: {Theme}", theme);
    }
}
