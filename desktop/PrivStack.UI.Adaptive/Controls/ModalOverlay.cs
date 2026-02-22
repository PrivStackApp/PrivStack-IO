// ============================================================================
// File: ModalOverlay.cs
// Description: Reusable modal overlay control with animated backdrop, centered
//              card, title bar with close button, scrollable body, and optional
//              footer. Supports Escape and backdrop-click to close. All visual
//              tokens resolve from the active theme.
// ============================================================================

using System.Windows.Input;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Transformation;
using Avalonia.Controls.Primitives;
using Avalonia.Threading;

namespace PrivStack.UI.Adaptive.Controls;

/// <summary>
/// Full-screen modal overlay with a dark backdrop and a centered card.
/// Provides animated entrance (scale + fade), Escape key handling,
/// and backdrop-click-to-close. Compose by setting <see cref="Body"/>
/// to any content and binding <see cref="CloseCommand"/>.
/// </summary>
public sealed class ModalOverlay : Panel
{
    // -------------------------------------------------------------------------
    // Styled properties
    // -------------------------------------------------------------------------

    public static readonly StyledProperty<string> TitleProperty =
        AvaloniaProperty.Register<ModalOverlay, string>(nameof(Title), string.Empty);

    public static readonly StyledProperty<double> ModalWidthProperty =
        AvaloniaProperty.Register<ModalOverlay, double>(nameof(ModalWidth), 560);

    public static readonly StyledProperty<double> ModalMaxHeightProperty =
        AvaloniaProperty.Register<ModalOverlay, double>(nameof(ModalMaxHeight), 800);

    public static readonly StyledProperty<ICommand?> CloseCommandProperty =
        AvaloniaProperty.Register<ModalOverlay, ICommand?>(nameof(CloseCommand));

    public static readonly StyledProperty<object?> BodyProperty =
        AvaloniaProperty.Register<ModalOverlay, object?>(nameof(Body));

    public static readonly StyledProperty<object?> FooterContentProperty =
        AvaloniaProperty.Register<ModalOverlay, object?>(nameof(FooterContent));

    // -------------------------------------------------------------------------
    // CLR accessors
    // -------------------------------------------------------------------------

    public string Title
    {
        get => GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }

    public double ModalWidth
    {
        get => GetValue(ModalWidthProperty);
        set => SetValue(ModalWidthProperty, value);
    }

    public double ModalMaxHeight
    {
        get => GetValue(ModalMaxHeightProperty);
        set => SetValue(ModalMaxHeightProperty, value);
    }

    public ICommand? CloseCommand
    {
        get => GetValue(CloseCommandProperty);
        set => SetValue(CloseCommandProperty, value);
    }

    public object? Body
    {
        get => GetValue(BodyProperty);
        set => SetValue(BodyProperty, value);
    }

    public object? FooterContent
    {
        get => GetValue(FooterContentProperty);
        set => SetValue(FooterContentProperty, value);
    }

    // -------------------------------------------------------------------------
    // Private children
    // -------------------------------------------------------------------------

    private readonly Border _backdrop;
    private readonly Border _card;
    private readonly TextBlock _titleBlock;
    private readonly Button _closeButton;
    private readonly ContentControl _bodyPresenter;
    private readonly Border _footerBorder;
    private readonly ContentControl _footerPresenter;

    // -------------------------------------------------------------------------
    // Constructor
    // -------------------------------------------------------------------------

    public ModalOverlay()
    {
        // Backdrop
        _backdrop = new Border
        {
            Opacity = 0,
        };
        _backdrop.Bind(Border.BackgroundProperty,
            _backdrop.GetResourceObservable("ThemeModalBackdropBrush"));
        _backdrop.Transitions =
        [
            new Avalonia.Animation.DoubleTransition
            {
                Property = Border.OpacityProperty,
                Duration = TimeSpan.FromMilliseconds(150),
            },
        ];
        _backdrop.PointerPressed += OnBackdropPressed;

        // Title
        _titleBlock = new TextBlock
        {
            VerticalAlignment = VerticalAlignment.Center,
            FontWeight = FontWeight.Bold,
        };
        _titleBlock.Bind(TextBlock.FontSizeProperty,
            _titleBlock.GetResourceObservable("ThemeFontSizeLg"));
        _titleBlock.Bind(TextBlock.ForegroundProperty,
            _titleBlock.GetResourceObservable("ThemeTextPrimaryBrush"));

        // Close button
        _closeButton = new Button
        {
            Content = "\u2715",
            Background = Brushes.Transparent,
            Padding = new Thickness(8, 4),
            CornerRadius = new CornerRadius(4),
            VerticalAlignment = VerticalAlignment.Center,
        };
        _closeButton.Bind(Button.ForegroundProperty,
            _closeButton.GetResourceObservable("ThemeTextMutedBrush"));
        _closeButton.Bind(Button.FontSizeProperty,
            _closeButton.GetResourceObservable("ThemeFontSizeLg"));
        _closeButton.Click += (_, _) =>
        {
            if (CloseCommand?.CanExecute(null) == true)
                CloseCommand.Execute(null);
        };

        // Header row
        var headerGrid = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*, Auto"),
        };
        headerGrid.Children.Add(_titleBlock);
        Grid.SetColumn(_closeButton, 1);
        headerGrid.Children.Add(_closeButton);

        var headerBorder = new Border
        {
            Padding = new Thickness(20, 16),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Child = headerGrid,
        };
        headerBorder.Bind(Border.BorderBrushProperty,
            headerBorder.GetResourceObservable("ThemeBorderBrush"));

        // Body
        _bodyPresenter = new ContentControl();
        var bodyScroll = new ScrollViewer
        {
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Content = _bodyPresenter,
        };

        // Footer
        _footerPresenter = new ContentControl();
        _footerBorder = new Border
        {
            Padding = new Thickness(20, 12),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = _footerPresenter,
            IsVisible = false,
        };
        _footerBorder.Bind(Border.BorderBrushProperty,
            _footerBorder.GetResourceObservable("ThemeBorderBrush"));

        // Card layout
        var cardGrid = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto, *, Auto"),
        };
        Grid.SetRow(headerBorder, 0);
        Grid.SetRow(bodyScroll, 1);
        Grid.SetRow(_footerBorder, 2);
        cardGrid.Children.Add(headerBorder);
        cardGrid.Children.Add(bodyScroll);
        cardGrid.Children.Add(_footerBorder);

        // Card
        _card = new Border
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 16),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(12),
            BoxShadow = BoxShadows.Parse("0 8 32 0 #60000000"),
            ClipToBounds = true,
            RenderTransformOrigin = RelativePoint.Center,
            RenderTransform = TransformOperations.Parse("scale(0.95)"),
            Opacity = 0,
            Child = cardGrid,
        };
        _card.Bind(Border.BackgroundProperty,
            _card.GetResourceObservable("ThemeSurfaceBrush"));
        _card.Bind(Border.BorderBrushProperty,
            _card.GetResourceObservable("ThemeBorderBrush"));
        _card.Transitions =
        [
            new Avalonia.Animation.TransformOperationsTransition
            {
                Property = Border.RenderTransformProperty,
                Duration = TimeSpan.FromMilliseconds(150),
                Easing = new Avalonia.Animation.Easings.CubicEaseOut(),
            },
            new Avalonia.Animation.DoubleTransition
            {
                Property = Border.OpacityProperty,
                Duration = TimeSpan.FromMilliseconds(120),
            },
        ];

        // Assemble
        Children.Add(_backdrop);
        Children.Add(_card);

        Focusable = true;
    }

    // -------------------------------------------------------------------------
    // Lifecycle
    // -------------------------------------------------------------------------

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);

        // Post so initial Opacity=0 / scale(0.95) renders first,
        // then the transition animates to final values.
        Dispatcher.UIThread.Post(() =>
        {
            _backdrop.Opacity = 1;
            _card.Opacity = 1;
            _card.RenderTransform = TransformOperations.Parse("scale(1)");
        }, DispatcherPriority.Loaded);
    }

    // -------------------------------------------------------------------------
    // Keyboard
    // -------------------------------------------------------------------------

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            if (CloseCommand?.CanExecute(null) == true)
            {
                CloseCommand.Execute(null);
                e.Handled = true;
                return;
            }
        }

        base.OnKeyDown(e);
    }

    // -------------------------------------------------------------------------
    // Property reactions
    // -------------------------------------------------------------------------

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == TitleProperty)
        {
            _titleBlock.Text = change.GetNewValue<string>();
        }
        else if (change.Property == ModalWidthProperty)
        {
            _card.Width = change.GetNewValue<double>();
        }
        else if (change.Property == ModalMaxHeightProperty)
        {
            _card.MaxHeight = change.GetNewValue<double>();
        }
        else if (change.Property == BodyProperty)
        {
            _bodyPresenter.Content = change.GetNewValue<object?>();
        }
        else if (change.Property == FooterContentProperty)
        {
            var footer = change.GetNewValue<object?>();
            _footerPresenter.Content = footer;
            _footerBorder.IsVisible = footer != null;
        }
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    private void OnBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        if (CloseCommand?.CanExecute(null) == true)
        {
            CloseCommand.Execute(null);
            e.Handled = true;
        }
    }
}
