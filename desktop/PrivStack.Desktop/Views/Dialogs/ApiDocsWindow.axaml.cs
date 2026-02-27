using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Media;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class ApiDocsWindow : Window
{
    public ApiDocsWindow()
    {
        InitializeComponent();
    }

    public void LoadRoutes(IPluginRegistry pluginRegistry)
    {
        var sections = new List<ApiPluginSection>();

        var providers = pluginRegistry.GetCapabilityProviders<IApiProvider>();
        foreach (var provider in providers)
        {
            var plugin = pluginRegistry.ActivePlugins
                .FirstOrDefault(p => p.Metadata.Id.Contains(provider.ApiSlug, StringComparison.OrdinalIgnoreCase));
            var displayName = plugin?.Metadata.Name ?? provider.ApiSlug;

            var routes = provider.GetRoutes();
            var items = routes.Select(r => new ApiRouteItem
            {
                Method = r.Method.ToString().ToUpperInvariant(),
                Path = string.IsNullOrEmpty(r.Path)
                    ? $"/api/v1/{provider.ApiSlug}"
                    : $"/api/v1/{provider.ApiSlug}/{r.Path}",
                Description = r.Description ?? "",
                MethodBrush = ResolveMethodBrush(r.Method),
            }).ToList();

            sections.Add(new ApiPluginSection
            {
                PluginName = displayName,
                Routes = items,
            });
        }

        PluginSections.ItemsSource = sections;
        NoPluginEndpoints.IsVisible = sections.Count == 0;
    }

    private IBrush ResolveMethodBrush(ApiMethod method)
    {
        var key = method switch
        {
            ApiMethod.Get => "ThemeSuccessBrush",
            ApiMethod.Post => "ThemePrimaryBrush",
            ApiMethod.Put => "ThemeWarningBrush",
            ApiMethod.Patch => "ThemeWarningBrush",
            ApiMethod.Delete => "ThemeDangerBrush",
            _ => "ThemeTextSecondaryBrush",
        };

        if (Application.Current!.TryFindResource(key, ActualThemeVariant, out var resource) && resource is IBrush brush)
            return brush;

        return Brushes.Gray;
    }

    private void OnClose(object? sender, RoutedEventArgs e)
    {
        Close();
    }
}

public class ApiPluginSection
{
    public string PluginName { get; init; } = "";
    public List<ApiRouteItem> Routes { get; init; } = [];
    public string Header => $"{PluginName}  ({Routes.Count} endpoint{(Routes.Count != 1 ? "s" : "")})";
}

public class ApiRouteItem
{
    public string Method { get; init; } = "";
    public string Path { get; init; } = "";
    public string Description { get; init; } = "";
    public IBrush MethodBrush { get; init; } = Brushes.Gray;
    public string DescriptionSuffix => string.IsNullOrEmpty(Description) ? "" : $"  {Description}";
}
