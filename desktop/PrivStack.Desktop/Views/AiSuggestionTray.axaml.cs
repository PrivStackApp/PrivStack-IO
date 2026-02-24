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

        if (e.Key == Key.Enter && !e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            var input = this.FindControl<TextBox>("ChatInputBox");
            if (input?.IsFocused == true && DataContext is AiSuggestionTrayViewModel vm)
            {
                if (vm.SendChatMessageCommand.CanExecute(null))
                {
                    vm.SendChatMessageCommand.Execute(null);
                    e.Handled = true;
                }
            }
        }
    }

    protected override void OnTextInput(TextInputEventArgs e)
    {
        base.OnTextInput(e);

        // Detect [[ trigger for link picker
        if (e.Text == "[" && _currentVm != null)
        {
            var input = this.FindControl<TextBox>("ChatInputBox");
            if (input?.IsFocused != true) return;

            var text = input.Text;
            var caretIndex = input.CaretIndex;

            // Check if the character before the caret is also [
            if (caretIndex >= 2 && text != null && text[caretIndex - 2] == '[')
            {
                // Remove the [[ from the text
                var newText = text[..(caretIndex - 2)] + text[caretIndex..];
                _currentVm.ChatInputText = newText;
                input.CaretIndex = caretIndex - 2;

                // Open the link picker
                _currentVm.OpenLinkPicker();

                // Focus the search box
                Avalonia.Threading.Dispatcher.UIThread.Post(() =>
                {
                    var searchBox = this.FindControl<TextBox>("LinkPickerSearchBox");
                    searchBox?.Focus();
                }, Avalonia.Threading.DispatcherPriority.Render);

                e.Handled = true;
            }
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
