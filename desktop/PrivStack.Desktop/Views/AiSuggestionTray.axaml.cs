using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using PrivStack.Desktop.ViewModels.AiTray;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Desktop.Views;

public partial class AiSuggestionTray : UserControl
{
    private AiSuggestionTrayViewModel? _currentVm;

    public AiSuggestionTray()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;

        // Attach TextInput on the ChatInputBox via tunnel routing so we see the event
        // BEFORE the TextBox swallows it. The override OnTextInput never fires because
        // TextBox marks TextInput as handled, preventing bubble propagation.
        // Also attach KeyDown via tunnel to intercept Enter before AcceptsReturn inserts a newline.
        Loaded += (_, _) =>
        {
            var chatInput = this.FindControl<TextBox>("ChatInputBox");
            chatInput?.AddHandler(TextInputEvent, OnChatInputTextInput, RoutingStrategies.Tunnel);
            chatInput?.AddHandler(KeyDownEvent, OnChatInputKeyDown, RoutingStrategies.Tunnel);
        };
    }

    private void OnDataContextChanged(object? sender, System.EventArgs e)
    {
        if (_currentVm != null)
            _currentVm.ScrollToBottomRequested -= OnScrollToBottomRequested;

        _currentVm = DataContext as AiSuggestionTrayViewModel;
        if (_currentVm != null)
            _currentVm.ScrollToBottomRequested += OnScrollToBottomRequested;
    }

    private void OnScrollToBottomRequested(object? sender, System.EventArgs e)
    {
        var sv = this.FindControl<ScrollViewer>("MessageScrollViewer");
        if (sv != null)
        {
            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                sv.ScrollToEnd();
            }, Avalonia.Threading.DispatcherPriority.Render);
        }
    }

    internal void OnTabChat(object? sender, RoutedEventArgs e) => SwitchTab(0);
    internal void OnTabIntents(object? sender, RoutedEventArgs e) => SwitchTab(1);
    internal void OnTabHistory(object? sender, RoutedEventArgs e) => SwitchTab(2);

    private void SwitchTab(int index)
    {
        if (_currentVm != null)
            _currentVm.SelectedTabIndex = index;
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        base.OnKeyDown(e);

        // If link picker is open, route keyboard events to it
        if (_currentVm?.ChatLinkPicker.IsOpen == true)
        {
            switch (e.Key)
            {
                case Key.Up:
                    _currentVm.ChatLinkPicker.MoveUp();
                    e.Handled = true;
                    return;
                case Key.Down:
                    _currentVm.ChatLinkPicker.MoveDown();
                    e.Handled = true;
                    return;
                case Key.Enter:
                    _currentVm.ChatLinkPicker.SelectCurrent();
                    // Refocus the chat input after selection
                    FocusChatInput();
                    e.Handled = true;
                    return;
                case Key.Escape:
                    _currentVm.ChatLinkPicker.Close();
                    FocusChatInput();
                    e.Handled = true;
                    return;
            }
        }

    }

    /// <summary>
    /// Tunnel handler on ChatInputBox for KeyDown. Since AcceptsReturn is true,
    /// we intercept Enter (without Shift) at tunnel phase to send the message
    /// instead of inserting a newline. Shift+Enter falls through to the TextBox
    /// and inserts a newline normally.
    /// </summary>
    private void OnChatInputKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter && !e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            if (_currentVm?.SendChatMessageCommand.CanExecute(null) == true)
            {
                _currentVm.SendChatMessageCommand.Execute(null);
                e.Handled = true;
            }
        }
    }

    /// <summary>
    /// Tunnel handler on the ChatInputBox. Because TextBox handles TextInput (marks it
    /// handled), the bubble-phase override OnTextInput on this UserControl never fires.
    /// Tunnel routing lets us intercept BEFORE the TextBox processes the keystroke.
    /// At tunnel time, e.Text is the incoming character but it hasn't been inserted yet —
    /// so we check text[caretIndex - 1] for the first '[' (already in the text).
    /// </summary>
    private void OnChatInputTextInput(object? sender, TextInputEventArgs e)
    {
        if (e.Text != "[" || _currentVm == null) return;

        var input = sender as TextBox;
        if (input == null) return;

        var text = input.Text;
        var caretIndex = input.CaretIndex;

        // At tunnel time the second [ hasn't been inserted yet.
        // The first [ is at caretIndex - 1 (already committed from the previous keystroke).
        if (caretIndex >= 1 && text != null && text[caretIndex - 1] == '[')
        {
            // Remove the first [ that's already in the text
            var newText = text[..(caretIndex - 1)] + text[caretIndex..];
            _currentVm.ChatInputText = newText;
            input.CaretIndex = Math.Max(0, caretIndex - 1);

            // Open the link picker
            _currentVm.OpenLinkPicker();

            // Focus the search box
            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                var searchBox = this.FindControl<TextBox>("LinkPickerSearchBox");
                searchBox?.Focus();
            }, Avalonia.Threading.DispatcherPriority.Render);

            // Prevent the second [ from being inserted into the TextBox
            e.Handled = true;
        }
    }

    // ── Link Picker Event Handlers ──────────────────────────────────

    private void OnLinkPickerItemPressed(object? sender, PointerPressedEventArgs e)
    {
        if (sender is Border border && border.DataContext is LinkableItem item
            && _currentVm != null)
        {
            var wikiLink = $"[[{item.LinkType}:{item.Id}|{item.Title}]]";
            _currentVm.ChatLinkPicker.Close();
            var current = _currentVm.ChatInputText ?? "";
            _currentVm.ChatInputText = current + wikiLink + " ";
            FocusChatInput();
        }
    }

    private void OnLinkPickerItemEntered(object? sender, PointerEventArgs e)
    {
        if (sender is Border border && _currentVm != null)
        {
            var itemsControl = this.FindControl<ItemsControl>("LinkPickerItems");
            if (itemsControl?.ItemsSource == null) return;

            var index = 0;
            foreach (var item in itemsControl.ItemsSource)
            {
                if (ReferenceEquals(item, border.DataContext))
                {
                    _currentVm.ChatLinkPicker.SelectedIndex = index;
                    break;
                }
                index++;
            }
        }
    }

    private void FocusChatInput()
    {
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            var input = this.FindControl<TextBox>("ChatInputBox");
            input?.Focus();
        }, Avalonia.Threading.DispatcherPriority.Render);
    }
}
