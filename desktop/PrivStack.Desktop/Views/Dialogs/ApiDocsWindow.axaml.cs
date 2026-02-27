using System.Windows.Input;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class ApiDocsWindow : Window
{
    public ApiDocsWindow()
    {
        InitializeComponent();
        RouteDetailOverlay.CloseCommand = new RelayCommand(() => RouteDetailOverlay.IsVisible = false);
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
            ShowRouteDetail(route);
    }

    // ── Route detail overlay ─────────────────────────────────────

    private void ShowRouteDetail(ApiRouteItem route)
    {
        // Build header: METHOD /path
        var methodBgKey = route.Method switch
        {
            "GET" => "ThemeSuccessBrush",
            "POST" => "ThemePrimaryBrush",
            "PATCH" or "PUT" => "ThemeWarningBrush",
            "DELETE" => "ThemeDangerBrush",
            _ => "ThemeTextMutedBrush",
        };

        var methodBadge = new Border
        {
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(8, 4),
            Child = new TextBlock
            {
                Text = route.Method,
                FontFamily = new FontFamily("Consolas, Menlo, monospace"),
                FontSize = 13,
                FontWeight = FontWeight.Bold,
                Foreground = Brushes.White,
            },
        };
        if (Application.Current!.TryFindResource(methodBgKey, ActualThemeVariant, out var bgRes) && bgRes is IBrush bgBrush)
            methodBadge.Background = bgBrush;

        var headerRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        headerRow.Children.Add(methodBadge);
        headerRow.Children.Add(new TextBlock
        {
            Text = route.Path,
            FontFamily = new FontFamily("Consolas, Menlo, monospace"),
            FontSize = 14,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = GetBrush("ThemeTextPrimaryBrush"),
        });

        var body = new StackPanel { Spacing = 16, Margin = new Thickness(4) };
        body.Children.Add(headerRow);

        if (!string.IsNullOrEmpty(route.Description))
        {
            body.Children.Add(new TextBlock
            {
                Text = route.Description,
                FontSize = 13,
                Foreground = GetBrush("ThemeTextMutedBrush"),
            });
        }

        // cURL
        body.Children.Add(BuildSection("cURL"));
        body.Children.Add(BuildCodeBlock(BuildCurlCommand(route)));

        // Query parameters
        if (route.QueryParamDocs is { Count: > 0 })
        {
            body.Children.Add(BuildSection("Query Parameters"));
            var paramPanel = new StackPanel { Spacing = 4, Margin = new Thickness(0, 4, 0, 0) };
            foreach (var doc in route.QueryParamDocs)
            {
                var parts = doc.Split(':', 2);
                var name = parts[0].Trim();
                var desc = parts.Length > 1 ? parts[1].Trim() : "";

                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                row.Children.Add(new Border
                {
                    CornerRadius = new CornerRadius(3),
                    Padding = new Thickness(6, 2),
                    Background = GetBrush("ThemeSurfaceElevatedBrush"),
                    Child = new TextBlock
                    {
                        Text = name,
                        FontFamily = new FontFamily("Consolas, Menlo, monospace"),
                        FontSize = 12,
                        Foreground = GetBrush("ThemePrimaryBrush"),
                    }
                });
                row.Children.Add(new TextBlock
                {
                    Text = desc,
                    FontSize = 12,
                    Foreground = GetBrush("ThemeTextSecondaryBrush"),
                    VerticalAlignment = VerticalAlignment.Center,
                });
                paramPanel.Children.Add(row);
            }
            body.Children.Add(paramPanel);
        }

        // Request body
        if (!string.IsNullOrEmpty(route.RequestExample))
        {
            body.Children.Add(BuildSection("Request Body"));
            body.Children.Add(BuildCodeBlock(route.RequestExample));
        }

        // Response body
        if (!string.IsNullOrEmpty(route.ResponseExample))
        {
            body.Children.Add(BuildSection("Response"));
            body.Children.Add(BuildCodeBlock(route.ResponseExample));
        }

        RouteDetailOverlay.Title = "Endpoint Detail";
        RouteDetailOverlay.Body = body;
        RouteDetailOverlay.IsVisible = true;
        RouteDetailOverlay.Focus();
    }

    private TextBlock BuildSection(string title) => new()
    {
        Text = title,
        FontWeight = FontWeight.SemiBold,
        FontSize = 14,
        Foreground = GetBrush("ThemeTextPrimaryBrush"),
    };

    private Border BuildCodeBlock(string code)
    {
        var codeBlock = new TextBlock
        {
            Text = code,
            FontFamily = new FontFamily("Consolas, Menlo, monospace"),
            FontSize = 12,
            Foreground = GetBrush("ThemeTextSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            LineHeight = 18,
        };

        var copyButton = new Button
        {
            Content = "Copy",
            FontSize = 11,
            Padding = new Thickness(10, 4),
            CornerRadius = new CornerRadius(4),
            Background = GetBrush("ThemeSurfaceBrush"),
            Foreground = GetBrush("ThemeTextSecondaryBrush"),
            BorderThickness = new Thickness(1),
            BorderBrush = GetBrush("ThemeBorderSubtleBrush"),
            HorizontalAlignment = HorizontalAlignment.Right,
            Cursor = new Cursor(StandardCursorType.Hand),
        };
        copyButton.Click += async (_, _) =>
        {
            var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
            if (clipboard != null)
            {
                await clipboard.SetTextAsync(code);
                ((Button)copyButton).Content = "Copied!";
                await Task.Delay(1500);
                ((Button)copyButton).Content = "Copy";
            }
        };

        var header = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            Margin = new Thickness(0, 0, 0, 8),
        };
        Grid.SetColumn(copyButton, 1);
        header.Children.Add(copyButton);

        var wrapper = new StackPanel { Spacing = 0 };
        wrapper.Children.Add(header);
        wrapper.Children.Add(codeBlock);

        return new Border
        {
            Background = GetBrush("ThemeSurfaceElevatedBrush"),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(14, 10),
            Margin = new Thickness(0, 4, 0, 0),
            Child = wrapper,
        };
    }

    private static string BuildCurlCommand(ApiRouteItem route)
    {
        var parts = new List<string> { "curl" };

        if (route.Method != "GET")
            parts.Add($"-X {route.Method}");

        parts.Add("-H \"X-API-Key: YOUR_KEY\"");

        if (!string.IsNullOrEmpty(route.RequestExample))
        {
            parts.Add("-H \"Content-Type: application/json\"");
            var body = route.RequestExample
                .Replace("\n", "").Replace("\r", "");
            while (body.Contains("  "))
                body = body.Replace("  ", " ");
            parts.Add($"-d '{body.Trim()}'");
        }

        parts.Add($"http://127.0.0.1:9720{route.Path}");

        return string.Join(" \\\n     ", parts);
    }

    private IBrush? GetBrush(string key)
    {
        if (Application.Current!.TryFindResource(key, ActualThemeVariant, out var res) && res is IBrush brush)
            return brush;
        return null;
    }

    /// <summary>Minimal ICommand for the overlay close button.</summary>
    private sealed class RelayCommand(Action execute) : ICommand
    {
#pragma warning disable CS0067 // Required by ICommand interface
        public event EventHandler? CanExecuteChanged;
#pragma warning restore CS0067
        public bool CanExecute(object? parameter) => true;
        public void Execute(object? parameter) => execute();
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
