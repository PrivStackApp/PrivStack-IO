using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.VisualTree;
using AvaloniaEdit;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Controls;
using PrivStack.Services.Models;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Desktop.ViewModels;
using PrivStack.Sdk;
using RichTextEditorControl = PrivStack.UI.Adaptive.Controls.RichTextEditor.RichTextEditor;

namespace PrivStack.Desktop.Views;

public partial class MainWindow : Window
{
    private readonly IAppSettingsService _settings = App.Services.GetRequiredService<IAppSettingsService>();
    private readonly IWindowSettingsService _windowSettings = App.Services.GetRequiredService<IWindowSettingsService>();
    private readonly IResponsiveLayoutService _responsiveLayout = App.Services.GetRequiredService<IResponsiveLayoutService>();
    private UniversalSearchService? _universalSearch;
    private bool _isInitialized;
    private Control? _speechTargetControl;
    private bool _chordPrefixActive;
    private System.Timers.Timer? _chordTimer;
    private bool _isResizingInfoPanel;
    private double _infoPanelResizeStartX;
    private double _infoPanelResizeStartWidth;
    private bool _isResizingAiTray;
    private double _aiTrayResizeStartX;
    private double _aiTrayResizeStartWidth;
    private AiTrayWindow? _floatingAiWindow;
    private bool _isReattaching;

    public MainWindow()
    {
        InitializeComponent();

        // Tunnel routing: intercept global shortcuts before child controls consume them
        AddHandler(KeyDownEvent, OnGlobalKeyDown, RoutingStrategies.Tunnel);

        // Enable window dragging from the title bar spacer
        TitleBarSpacer.PointerPressed += OnTitleBarPointerPressed;

        // Info panel drag-to-resize
        var dragHandle = this.FindControl<Border>("InfoPanelDragHandle");
        if (dragHandle != null)
        {
            dragHandle.PointerPressed += OnInfoPanelDragStart;
            dragHandle.PointerMoved += OnInfoPanelDragMove;
            dragHandle.PointerReleased += OnInfoPanelDragEnd;
            dragHandle.PointerCaptureLost += (_, _) => _isResizingInfoPanel = false;
        }

        // AI tray drag-to-resize
        var aiTrayHandle = this.FindControl<Border>("AiTrayDragHandle");
        if (aiTrayHandle != null)
        {
            aiTrayHandle.PointerPressed += OnAiTrayDragStart;
            aiTrayHandle.PointerMoved += OnAiTrayDragMove;
            aiTrayHandle.PointerReleased += OnAiTrayDragEnd;
            aiTrayHandle.PointerCaptureLost += (_, _) => _isResizingAiTray = false;
        }

        // Dynamically position the AI balloon over the star icon
        SetupBalloonPositioning();

        // Apply saved window settings
        _windowSettings.ApplyToWindow(this);

        // Hook up window events for saving state
        this.Opened += OnWindowOpened;
        this.PositionChanged += OnPositionChanged;
        this.PropertyChanged += OnWindowPropertyChanged;

        // Set up speech recording event handlers
        SetupSpeechRecording();
    }

    private void SetupSpeechRecording()
    {
        if (DataContext is MainWindowViewModel vm)
        {
            vm.SpeechRecordingVM.TranscriptionReady += OnTranscriptionReady;
        }

        this.DataContextChanged += (_, _) =>
        {
            if (DataContext is MainWindowViewModel newVm)
            {
                newVm.SpeechRecordingVM.TranscriptionReady += OnTranscriptionReady;
            }
        };
    }

    private void OnTranscriptionReady(object? sender, string transcription)
    {
        if (string.IsNullOrWhiteSpace(transcription))
        {
            _speechTargetControl = null;
            return;
        }

        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            if (_speechTargetControl != null)
            {
                switch (_speechTargetControl)
                {
                    case TextBox textBox:
                        InsertTextIntoTextBox(textBox, transcription);
                        break;
                    case TextEditor editor:
                        InsertTextIntoEditor(editor, transcription);
                        break;
                    case RichTextEditorControl rte:
                        rte.InsertTranscription(transcription);
                        rte.Focus();
                        break;
                }
            }
            else if (DataContext is MainWindowViewModel vm)
            {
                // No text input focused — route voice transcription to Duncan AI tray
                _ = vm.AiTrayVM.SendVoiceMessageAsync(transcription);
                vm.IsAiTrayOpen = true;
            }

            _speechTargetControl = null;
        });
    }

    private static void InsertTextIntoTextBox(TextBox textBox, string text)
    {
        var caretIndex = textBox.CaretIndex;
        var currentText = textBox.Text ?? "";
        textBox.Text = currentText.Insert(caretIndex, text);
        textBox.CaretIndex = caretIndex + text.Length;
        textBox.Focus();
    }

    private static void InsertTextIntoEditor(TextEditor editor, string text)
    {
        var offset = editor.CaretOffset;
        editor.Document.Insert(offset, text);
        editor.CaretOffset = offset + text.Length;
        editor.Focus();
    }

    // ========================================================================
    // Window Lifecycle
    // ========================================================================

    private void OnWindowOpened(object? sender, EventArgs e)
    {
        _isInitialized = true;

        // Sync the title bar spacer row to the actual platform decoration height.
        // On macOS with notch displays, the system title bar can be taller than our
        // hardcoded 28px hint, causing content to overflow the window bottom.
        var decorTop = WindowDecorationMargin.Top;
        var offScreen = OffScreenMargin;
        Log.Information("[MainWindow] Decorations: Top={DecorTop} OffScreen={OffScreen}", decorTop, offScreen);
        Log.Information("[MainWindow] Sizes: ClientSize={ClientSize} Bounds={Bounds}", ClientSize, Bounds);

        // Adjust title bar spacer to actual platform decoration height
        if (decorTop > 0 && Math.Abs(decorTop - 28) > 0.5)
        {
            Log.Information("[MainWindow] Adjusting title bar spacer from 28 to {DecorTop}", decorTop);
            var rootGrid = this.Content as Panel;
            var mainGrid = rootGrid?.Children.OfType<Avalonia.Controls.Grid>().FirstOrDefault();
            if (mainGrid?.RowDefinitions.Count > 0)
                mainGrid.RowDefinitions[0].Height = new GridLength(decorTop);
            TitleBarSpacer.Height = decorTop;
        }

        // Compensate for any off-screen margin that extends the layout area
        // beyond the visible window (e.g., macOS window shadow area)
        if (offScreen.Bottom > 0 || offScreen.Top > 0)
        {
            Log.Information("[MainWindow] Applying OffScreenMargin compensation");
            var rootGrid = this.Content as Panel;
            var mainGrid = rootGrid?.Children.OfType<Avalonia.Controls.Grid>().FirstOrDefault();
            if (mainGrid != null)
                mainGrid.Margin = new Thickness(offScreen.Left, 0, offScreen.Right, offScreen.Bottom);
        }

        (App.Services.GetRequiredService<IDialogService>() as DialogService)?.SetOwner(this);

        if (DataContext is MainWindowViewModel vm)
        {
            vm.PropertyChanged += OnMainVmPropertyChanged;

            _universalSearch = new UniversalSearchService(vm.CommandPaletteVM, vm);
            _universalSearch.SetDropdown(SearchDropdown);

            var lastTab = _settings.Settings.LastActiveTab;
            var pluginRegistry = App.Services.GetRequiredService<IPluginRegistry>();
            if (!string.IsNullOrEmpty(lastTab) && pluginRegistry.GetPluginForNavItem(lastTab) != null)
            {
                vm.SelectTabCommand.Execute(lastTab);
            }
            else if (pluginRegistry.NavigationItems.Count > 0)
            {
                vm.SelectTabCommand.Execute(pluginRegistry.NavigationItems[0].Id);
            }

            WireBalloonPositioning();
        }

        UpdateContentAreaWidth();
    }

    private void OnMainVmPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MainWindowViewModel.IsSidebarCollapsed))
        {
            UpdateContentAreaWidth();
        }
        else if (e.PropertyName == nameof(MainWindowViewModel.AiTrayDisplayMode))
        {
            OnAiTrayDisplayModeChanged();
        }
        else if (e.PropertyName == nameof(MainWindowViewModel.IsAiTrayDetached) &&
                 DataContext is MainWindowViewModel vm && vm.IsAiTrayDetached && _floatingAiWindow != null)
        {
            // Bring floating window to front when toggle is pressed in detached mode
            _floatingAiWindow.Activate();
        }
    }

    private void UpdateContentAreaWidth()
    {
        if (!_isInitialized) return;

        var sidebarCollapsed = (DataContext as MainWindowViewModel)?.IsSidebarCollapsed ?? false;
        var navSidebarWidth = sidebarCollapsed ? 56.0 : 220.0;
        var contentWidth = Bounds.Width - navSidebarWidth;

        if (contentWidth > 0)
        {
            _responsiveLayout.UpdateContentAreaWidth(contentWidth);
        }
    }

    private void OnPositionChanged(object? sender, PixelPointEventArgs e)
    {
        if (_isInitialized && WindowState == WindowState.Normal)
        {
            _windowSettings.UpdateWindowBounds(this);
        }
    }

    private void OnWindowPropertyChanged(object? sender, AvaloniaPropertyChangedEventArgs e)
    {
        if (!_isInitialized) return;

        if (e.Property == WidthProperty || e.Property == HeightProperty || e.Property == WindowStateProperty)
        {
            _windowSettings.UpdateWindowBounds(this);
            UpdateContentAreaWidth();
            UpdateAiTrayMaxHeight();
        }
    }

    protected override void OnClosing(WindowClosingEventArgs e)
    {
        _windowSettings.UpdateWindowBounds(this);
        _settings.Flush();

        // Close floating AI window if open
        if (_floatingAiWindow != null)
        {
            _isReattaching = true;
            _floatingAiWindow.Close();
            _floatingAiWindow = null;
        }

        if (DataContext is MainWindowViewModel vm)
        {
            vm.Cleanup();
        }

        base.OnClosing(e);
    }

    // ========================================================================
    // Title Bar + Pointer Handlers
    // ========================================================================

    private void OnTitleBarPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            BeginMoveDrag(e);
        }
    }

    // ========================================================================
    // Info Panel Drag-to-Resize
    // ========================================================================

    private void OnInfoPanelDragStart(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is not MainWindowViewModel vm) return;
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed) return;

        _isResizingInfoPanel = true;
        _infoPanelResizeStartX = e.GetPosition(this).X;
        _infoPanelResizeStartWidth = vm.InfoPanelVM.PanelWidth;
        e.Pointer.Capture((IInputElement)sender!);
        e.Handled = true;
    }

    private void OnInfoPanelDragMove(object? sender, PointerEventArgs e)
    {
        if (!_isResizingInfoPanel) return;
        if (DataContext is not MainWindowViewModel vm) return;

        var currentX = e.GetPosition(this).X;
        var delta = _infoPanelResizeStartX - currentX;
        var newWidth = Math.Clamp(_infoPanelResizeStartWidth + delta, 220, 600);
        vm.InfoPanelVM.PanelWidth = newWidth;
        e.Handled = true;
    }

    private void OnInfoPanelDragEnd(object? sender, PointerReleasedEventArgs e)
    {
        if (!_isResizingInfoPanel) return;
        _isResizingInfoPanel = false;
        e.Pointer.Capture(null);
        e.Handled = true;
    }

    // AI tray no longer closes on click-away — only via close button or Escape key

    private void OnAiTrayDragStart(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is not MainWindowViewModel vm) return;
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed) return;

        _isResizingAiTray = true;
        _aiTrayResizeStartX = e.GetPosition(this).X;
        _aiTrayResizeStartWidth = vm.AiTrayWidth;
        e.Pointer.Capture((IInputElement)sender!);
        e.Handled = true;
    }

    private void OnAiTrayDragMove(object? sender, PointerEventArgs e)
    {
        if (!_isResizingAiTray) return;
        if (DataContext is not MainWindowViewModel vm) return;

        var currentX = e.GetPosition(this).X;
        var delta = _aiTrayResizeStartX - currentX;
        var newWidth = Math.Clamp(_aiTrayResizeStartWidth + delta, 320, 700);
        vm.AiTrayWidth = newWidth;
        e.Handled = true;
    }

    private void OnAiTrayDragEnd(object? sender, PointerReleasedEventArgs e)
    {
        if (!_isResizingAiTray) return;
        _isResizingAiTray = false;
        e.Pointer.Capture(null);
        e.Handled = true;
    }

    private void OnInfoPanelBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is MainWindowViewModel vm && vm.InfoPanelVM.IsOpen)
        {
            vm.InfoPanelVM.CloseCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void OnOverlayPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is MainWindowViewModel vm && vm.IsUserMenuOpen)
        {
            vm.CloseUserMenuCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void OnBackdropPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is MainWindowViewModel vm)
        {
            vm.CloseAllPanelsCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void OnQuickActionBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is MainWindowViewModel vm && vm.IsQuickActionOverlayOpen)
        {
            vm.CloseQuickActionOverlay();
            e.Handled = true;
        }
    }

    private void OnQuickActionContentPressed(object? sender, PointerPressedEventArgs e)
    {
        // Prevent backdrop click-through when clicking inside the form
        e.Handled = true;
    }

    // ========================================================================
    // AI Tray Display Mode: Detach / Reattach / Half-Height
    // ========================================================================

    private void OnAiTrayDisplayModeChanged()
    {
        if (DataContext is not MainWindowViewModel vm) return;

        switch (vm.AiTrayDisplayMode)
        {
            case AiTrayDisplayMode.Detached:
                DetachAiTray(vm);
                break;
            case AiTrayDisplayMode.AttachedFull:
                if (_floatingAiWindow != null) ReattachAiTray(vm);
                vm.AiTrayMaxHeight = double.PositiveInfinity;
                break;
            case AiTrayDisplayMode.AttachedHalf:
                if (_floatingAiWindow != null) ReattachAiTray(vm);
                UpdateAiTrayMaxHeight();
                break;
        }
    }

    private void DetachAiTray(MainWindowViewModel vm)
    {
        var trayControl = this.FindControl<AiSuggestionTray>("AiSuggestionTrayControl");
        if (trayControl == null) return;

        // Close link picker if open (clean state before reparent)
        vm.AiTrayVM.ChatLinkPicker.Close();

        // Remove from inline drawer
        var inlineGrid = trayControl.Parent as Grid;
        inlineGrid?.Children.Remove(trayControl);

        // Explicitly set DataContext — the XAML binding "{Binding AiTrayVM}" would fail
        // in the floating window since its DataContext IS the AiSuggestionTrayViewModel,
        // not a MainWindowViewModel that has an AiTrayVM property.
        trayControl.DataContext = vm.AiTrayVM;

        // Create floating window
        _floatingAiWindow = new AiTrayWindow
        {
            Content = trayControl
        };

        // Position near the main window's right edge
        var mainPos = Position;
        var mainBounds = Bounds;
        _floatingAiWindow.Position = new PixelPoint(
            mainPos.X + (int)mainBounds.Width + 8,
            mainPos.Y + 40);

        _floatingAiWindow.WindowClosingByUser += OnFloatingAiWindowClosing;
        vm.AiTrayVM.ReattachRequested += OnAiTrayReattachRequested;
        vm.AiTrayVM.IsDetached = true;
        _floatingAiWindow.Show();
    }

    private void ReattachAiTray(MainWindowViewModel vm)
    {
        if (_floatingAiWindow == null) return;

        var trayControl = _floatingAiWindow.Content as AiSuggestionTray;
        _floatingAiWindow.Content = null;

        // Put the tray control back in the inline drawer
        if (trayControl != null)
        {
            var borderContainer = this.FindControl<Border>("AiTrayBorderContainer");
            var inlineGrid = borderContainer?.Child as Grid;
            if (inlineGrid != null)
            {
                Grid.SetRow(trayControl, 1);
                trayControl.DataContext = vm.AiTrayVM;
                inlineGrid.Children.Add(trayControl);
            }
        }

        _isReattaching = true;
        vm.AiTrayVM.ReattachRequested -= OnAiTrayReattachRequested;
        vm.AiTrayVM.IsDetached = false;
        _floatingAiWindow.WindowClosingByUser -= OnFloatingAiWindowClosing;
        _floatingAiWindow.Close();
        _floatingAiWindow = null;
        _isReattaching = false;
    }

    private void OnFloatingAiWindowClosing(object? sender, EventArgs e)
    {
        if (_isReattaching) return;

        // User closed the floating window via OS X button — reattach to main window
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            if (DataContext is MainWindowViewModel vm)
            {
                // Reattach the control before changing mode
                var trayControl = _floatingAiWindow?.Content as AiSuggestionTray;
                if (_floatingAiWindow != null)
                    _floatingAiWindow.Content = null;

                if (trayControl != null)
                {
                    var borderContainer = this.FindControl<Border>("AiTrayBorderContainer");
                    var inlineGrid = borderContainer?.Child as Grid;
                    if (inlineGrid != null)
                    {
                        Grid.SetRow(trayControl, 1);
                        trayControl.DataContext = vm.AiTrayVM;
                        inlineGrid.Children.Add(trayControl);
                    }
                }

                _floatingAiWindow = null;
                vm.AiTrayVM.ReattachRequested -= OnAiTrayReattachRequested;
                vm.AiTrayVM.IsDetached = false;
                vm.AiTrayDisplayMode = AiTrayDisplayMode.AttachedFull;
                vm.AiTrayMaxHeight = double.PositiveInfinity;
            }
        });
    }

    private void OnAiTrayReattachRequested(object? sender, EventArgs e)
    {
        if (DataContext is MainWindowViewModel vm)
            vm.SetAiTrayDisplayModeCommand.Execute(AiTrayDisplayMode.AttachedFull);
    }

    private void UpdateAiTrayMaxHeight()
    {
        if (DataContext is not MainWindowViewModel vm) return;
        if (vm.AiTrayDisplayMode != AiTrayDisplayMode.AttachedHalf) return;

        // Use the content area height (exclude title bar ~28px and status bar ~32px)
        var availableHeight = Math.Max(300, Bounds.Height - 60);
        vm.AiTrayMaxHeight = availableHeight / 2.0;
    }

    // ========================================================================
    // AI Balloon Dynamic Positioning
    // ========================================================================

    private void SetupBalloonPositioning()
    {
        // Hook into the ViewModel to reposition when balloon message changes
        this.DataContextChanged += (_, _) => WireBalloonPositioning();
    }

    private void WireBalloonPositioning()
    {
        if (DataContext is not MainWindowViewModel vm) return;

        vm.AiTrayVM.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(vm.AiTrayVM.BalloonMessage) ||
                e.PropertyName == nameof(vm.AiTrayVM.HasBalloonMessage))
            {
                Avalonia.Threading.Dispatcher.UIThread.Post(() =>
                {
                    var balloon = this.FindControl<Border>("AiBalloon");
                    // AiStarIcon is inside NavigationSidebar (separate name scope) — search visual tree
                    var starIcon = this.GetVisualDescendants()
                        .OfType<PathIcon>()
                        .FirstOrDefault(p => p.Name == "AiStarIcon");
                    if (balloon != null && starIcon != null)
                        PositionBalloonOverStar(balloon, starIcon);
                }, Avalonia.Threading.DispatcherPriority.Render);
            }
        };
    }

    private void PositionBalloonOverStar(Border balloon, PathIcon starIcon)
    {
        if (DataContext is not MainWindowViewModel vm || !vm.AiTrayVM.HasBalloonMessage)
            return;

        try
        {
            // Get star icon center position relative to this window
            var starBounds = starIcon.Bounds;
            var starCenter = starIcon.TranslatePoint(
                new Point(starBounds.Width / 2, 0), this);

            if (starCenter == null) return;

            // The arrow tip is 32px from the balloon's left edge (24px arrow margin + 8px half-width)
            const double arrowOffsetFromLeft = 32;

            // Calculate left margin so arrow tip aligns with star center
            var leftMargin = starCenter.Value.X - arrowOffsetFromLeft;
            leftMargin = Math.Max(8, leftMargin); // floor at 8px

            balloon.Margin = new Thickness(leftMargin, 0, 0, 48);
        }
        catch
        {
            // Fallback to default position if measurement fails
            balloon.Margin = new Thickness(260, 0, 0, 48);
        }
    }
}
