// ============================================================================
// File: InfiniteCanvasContextMenu.cs
// Description: Right-click context menus for canvas elements, connectors,
//              and empty canvas background. Provides Delete, Duplicate,
//              Z-order, color, connector style, and arrow mode options.
// ============================================================================

using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using PrivStack.UI.Adaptive.Models;

namespace PrivStack.UI.Adaptive.Controls.InfiniteCanvas;

public sealed partial class InfiniteCanvasControl
{
    private static readonly (string Name, string Hex)[] ColorPresets =
    [
        ("Gray", "#9CA3AF"),
        ("Brown", "#A8763E"),
        ("Orange", "#F97316"),
        ("Yellow", "#EAB308"),
        ("Green", "#22C55E"),
        ("Blue", "#3B82F6"),
        ("Purple", "#A855F7"),
        ("Pink", "#EC4899"),
        ("Red", "#EF4444"),
    ];

    // ================================================================
    // Right-Click Dispatch
    // ================================================================

    internal void HandleRightClick(Point pos, CanvasData? data)
    {
        if (IsReadOnly || data == null) return;

        // Hit-test element first
        var hitElement = HitTestElement(pos, data);
        if (hitElement != null)
        {
            if (!IsSelected(hitElement.Id))
            {
                ClearSelection();
                ClearConnectorSelection();
                Select(hitElement.Id);
                ElementSelected?.Invoke(hitElement);
            }

            ShowElementContextMenu(pos);
            return;
        }

        // Hit-test connector
        var hitConnector = HitTestConnector(pos, data);
        if (hitConnector != null)
        {
            ClearSelection();
            SelectConnector(hitConnector.Id);
            ShowConnectorContextMenu(pos, hitConnector);
            return;
        }

        // Empty canvas
        ShowCanvasContextMenu(pos);
    }

    // ================================================================
    // Element Context Menu
    // ================================================================

    private void ShowElementContextMenu(Point pos)
    {
        var menu = new ContextMenu();

        var deleteItem = new MenuItem { Header = "Delete" };
        deleteItem.Click += (_, _) => DeleteSelectedElements();
        menu.Items.Add(deleteItem);

        var duplicateItem = new MenuItem { Header = "Duplicate" };
        duplicateItem.Click += (_, _) => DuplicateSelected();
        menu.Items.Add(duplicateItem);

        menu.Items.Add(new Separator());

        var bringFrontItem = new MenuItem { Header = "Bring to Front" };
        bringFrontItem.Click += (_, _) => BringSelectedToFront();
        menu.Items.Add(bringFrontItem);

        var sendBackItem = new MenuItem { Header = "Send to Back" };
        sendBackItem.Click += (_, _) => SendSelectedToBack();
        menu.Items.Add(sendBackItem);

        menu.Items.Add(new Separator());

        menu.Items.Add(BuildColorSubmenu(color => SetSelectedElementColor(color)));

        menu.Open(this);
    }

    // ================================================================
    // Connector Context Menu
    // ================================================================

    private void ShowConnectorContextMenu(Point pos, CanvasConnector connector)
    {
        var menu = new ContextMenu();

        var deleteItem = new MenuItem { Header = "Delete" };
        deleteItem.Click += (_, _) => DeleteSelectedElements();
        menu.Items.Add(deleteItem);

        menu.Items.Add(new Separator());

        // Style submenu
        var styleMenu = new MenuItem { Header = "Style" };
        foreach (var style in new[] { ConnectorStyle.Straight, ConnectorStyle.Curved, ConnectorStyle.Elbow })
        {
            var label = style switch
            {
                ConnectorStyle.Straight => "Straight",
                ConnectorStyle.Curved => "Curved",
                ConnectorStyle.Elbow => "Elbow",
                _ => style.ToString(),
            };

            var prefix = connector.Style == style ? "\u2713 " : "   ";
            var item = new MenuItem { Header = prefix + label };
            var capturedStyle = style;
            item.Click += (_, _) => SetSelectedConnectorStyle(capturedStyle);
            styleMenu.Items.Add(item);
        }
        menu.Items.Add(styleMenu);

        // Arrow submenu
        var arrowMenu = new MenuItem { Header = "Arrow" };
        foreach (var mode in new[] { ArrowMode.Forward, ArrowMode.None, ArrowMode.Backward, ArrowMode.Both })
        {
            var label = mode switch
            {
                ArrowMode.Forward => "Forward (\u2192)",
                ArrowMode.None => "None (\u2014)",
                ArrowMode.Backward => "Backward (\u2190)",
                ArrowMode.Both => "Both (\u2194)",
                _ => mode.ToString(),
            };

            var prefix = connector.ArrowMode == mode ? "\u2713 " : "   ";
            var item = new MenuItem { Header = prefix + label };
            var capturedMode = mode;
            item.Click += (_, _) => SetSelectedArrowMode(capturedMode);
            arrowMenu.Items.Add(item);
        }
        menu.Items.Add(arrowMenu);

        menu.Items.Add(new Separator());

        menu.Items.Add(BuildColorSubmenu(color => SetSelectedConnectorColor(color)));

        menu.Open(this);
    }

    // ================================================================
    // Canvas Background Context Menu
    // ================================================================

    private void ShowCanvasContextMenu(Point pos)
    {
        var menu = new ContextMenu();

        var selectAllItem = new MenuItem { Header = "Select All" };
        selectAllItem.Click += (_, _) => SelectAll();
        menu.Items.Add(selectAllItem);

        var fitItem = new MenuItem { Header = "Fit to View" };
        fitItem.Click += (_, _) => FitToView();
        menu.Items.Add(fitItem);

        menu.Open(this);
    }

    // ================================================================
    // Color Submenu Builder
    // ================================================================

    private static MenuItem BuildColorSubmenu(Action<string?> applyColor)
    {
        var colorMenu = new MenuItem { Header = "Color" };

        foreach (var (name, hex) in ColorPresets)
        {
            var item = new MenuItem();
            var panel = new StackPanel { Orientation = Avalonia.Layout.Orientation.Horizontal, Spacing = 6 };
            var swatch = new Border
            {
                Width = 14,
                Height = 14,
                CornerRadius = new CornerRadius(3),
                Background = new SolidColorBrush(Color.Parse(hex)),
            };
            panel.Children.Add(swatch);
            panel.Children.Add(new TextBlock { Text = name, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center });
            item.Header = panel;

            var capturedHex = hex;
            item.Click += (_, _) => applyColor(capturedHex);
            colorMenu.Items.Add(item);
        }

        colorMenu.Items.Add(new Separator());

        var defaultItem = new MenuItem { Header = "Default (remove)" };
        defaultItem.Click += (_, _) => applyColor(null);
        colorMenu.Items.Add(defaultItem);

        return colorMenu;
    }
}
