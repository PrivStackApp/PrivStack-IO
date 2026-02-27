using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
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
                RequestExample = NormalizeJson(r.RequestExample),
                ResponseExample = NormalizeJson(r.ResponseExample),
                QueryParamDocs = r.QueryParamDocs,
                HasDetail = r.RequestExample != null || r.ResponseExample != null || r.QueryParamDocs is { Count: > 0 },
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

    private static string? NormalizeJson(string? json)
    {
        if (string.IsNullOrWhiteSpace(json)) return null;
        // Trim uniform leading whitespace from multi-line const strings
        var lines = json.Split('\n');
        var trimmed = lines.Select(l => l.TrimStart()).Where(l => l.Length > 0);
        return string.Join("\n", trimmed);
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

    private void OnRouteClick(object? sender, PointerPressedEventArgs e)
    {
        if (sender is Control { DataContext: ApiRouteItem route })
        {
            var detail = new ApiRouteDetailWindow(route);
            detail.ShowDialog(this);
        }
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
    public string? RequestExample { get; init; }
    public string? ResponseExample { get; init; }
    public IReadOnlyList<string>? QueryParamDocs { get; init; }
    public bool HasDetail { get; init; }
    public string DescriptionSuffix => string.IsNullOrEmpty(Description) ? "" : $"  {Description}";
}
