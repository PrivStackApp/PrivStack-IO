using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class ApiRouteDetailWindow : Window
{
    public ApiRouteDetailWindow() { InitializeComponent(); }

    public ApiRouteDetailWindow(ApiRouteItem route)
    {
        InitializeComponent();

        MethodText.Text = route.Method;
        PathText.Text = route.Path;
        DescriptionText.Text = route.Description;
        DescriptionText.IsVisible = !string.IsNullOrEmpty(route.Description);

        // Color the method badge
        var bgKey = route.Method switch
        {
            "GET" => "ThemeSuccessBrush",
            "POST" => "ThemePrimaryBrush",
            "PATCH" or "PUT" => "ThemeWarningBrush",
            "DELETE" => "ThemeDangerBrush",
            _ => "ThemeTextMutedBrush",
        };
        if (Application.Current!.TryFindResource(bgKey, ActualThemeVariant, out var res) && res is IBrush brush)
            MethodBadge.Background = brush;

        BuildDetailContent(route);
    }

    private void BuildDetailContent(ApiRouteItem route)
    {
        // cURL command
        var curl = BuildCurlCommand(route);
        DetailContent.Children.Add(BuildSection("cURL"));
        DetailContent.Children.Add(BuildCodeBlock(curl));

        // Query parameters
        if (route.QueryParamDocs is { Count: > 0 })
        {
            DetailContent.Children.Add(BuildSection("Query Parameters"));
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
            DetailContent.Children.Add(paramPanel);
        }

        // Request body
        if (!string.IsNullOrEmpty(route.RequestExample))
        {
            DetailContent.Children.Add(BuildSection("Request Body"));
            DetailContent.Children.Add(BuildCodeBlock(route.RequestExample));
        }

        // Response body
        if (!string.IsNullOrEmpty(route.ResponseExample))
        {
            DetailContent.Children.Add(BuildSection("Response"));
            DetailContent.Children.Add(BuildCodeBlock(route.ResponseExample));
        }

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
            // Compact the JSON to a single line for the -d flag
            var body = route.RequestExample
                .Replace("\n", "").Replace("\r", "");
            // Collapse runs of whitespace between JSON tokens
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

    private void OnClose(object? sender, RoutedEventArgs e) => Close();
}
