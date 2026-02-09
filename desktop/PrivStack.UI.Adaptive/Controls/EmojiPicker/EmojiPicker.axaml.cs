using System.ComponentModel;
using System.Windows.Input;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Media;
using Avalonia.Threading;

namespace PrivStack.UI.Adaptive.Controls.EmojiPicker;

public partial class EmojiPicker : UserControl
{
    private const int ColumnsPerRow = 6;
    private static IBrush HighlightBrush => Application.Current?.FindResource("ThemePrimaryMutedBrush") as IBrush
        ?? new SolidColorBrush(Color.FromArgb(40, 100, 100, 255));
    private static readonly IBrush TransparentBrush = Brushes.Transparent;

    public EmojiPicker()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;

        // Use tunnel routing to intercept arrow keys before TextBox handles them
        AddHandler(KeyDownEvent, OnPreviewKeyDown, RoutingStrategies.Tunnel);
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        if (DataContext is INotifyPropertyChanged npc)
        {
            npc.PropertyChanged += (s, args) =>
            {
                if (args.PropertyName == "IsOpen")
                {
                    var isOpen = DataContext?.GetType().GetProperty("IsOpen")?.GetValue(DataContext) as bool?;
                    if (isOpen == true)
                    {
                        Dispatcher.UIThread.Post(() =>
                        {
                            var searchBox = this.FindControl<TextBox>("SearchBox");
                            searchBox?.Focus();
                            searchBox?.SelectAll();
                            UpdateHighlights();
                        }, DispatcherPriority.Background);
                    }
                }
                else if (args.PropertyName is "IsCategoryFocused" or "SelectedCategoryItem" or "SelectedEmoji")
                {
                    Dispatcher.UIThread.Post(UpdateHighlights, DispatcherPriority.Background);
                }
            };
        }
    }

    private object? GetProp(object? obj, string name) =>
        obj?.GetType().GetProperty(name)?.GetValue(obj);

    private ICommand? GetCommand(string name) =>
        DataContext?.GetType().GetProperty(name)?.GetValue(DataContext) as ICommand;

    private void UpdateHighlights()
    {
        var dc = DataContext;
        if (dc == null) return;

        var isCategoryFocused = GetProp(dc, "IsCategoryFocused") as bool? ?? false;
        var selectedCategoryItem = GetProp(dc, "SelectedCategoryItem");
        var selectedEmoji = GetProp(dc, "SelectedEmoji");

        // Update category highlights
        var categoryItems = this.FindControl<ItemsControl>("CategoryTabs");
        if (categoryItems != null)
        {
            foreach (var container in categoryItems.GetRealizedContainers())
            {
                if (container is ContentPresenter { Child: Border border })
                {
                    var category = border.DataContext;
                    var isSelected = isCategoryFocused && ReferenceEquals(selectedCategoryItem, category);
                    border.Background = isSelected ? HighlightBrush : TransparentBrush;
                }
            }
        }

        // Update emoji highlights and scroll selected into view
        var emojiGrid = this.FindControl<ItemsControl>("EmojiGrid");
        if (emojiGrid != null)
        {
            Border? selectedBorder = null;
            foreach (var container in emojiGrid.GetRealizedContainers())
            {
                if (container is ContentPresenter { Child: Border border })
                {
                    var emoji = border.DataContext;
                    var isSelected = !isCategoryFocused && ReferenceEquals(selectedEmoji, emoji);
                    border.Background = isSelected ? HighlightBrush : TransparentBrush;
                    if (isSelected)
                        selectedBorder = border;
                }
            }

            selectedBorder?.BringIntoView();
        }
    }

    private void OnPreviewKeyDown(object? sender, KeyEventArgs e)
    {
        var dc = DataContext;
        if (dc == null) return;

        var isOpen = GetProp(dc, "IsOpen") as bool?;
        if (isOpen != true) return;

        var isCategoryFocused = GetProp(dc, "IsCategoryFocused") as bool? ?? false;

        var searchBox = this.FindControl<TextBox>("SearchBox");
        var searchQuery = GetProp(dc, "SearchQuery") as string;
        var isSearchEmpty = string.IsNullOrEmpty(searchBox?.Text);
        var isAtSearchStart = searchBox == null || searchBox.CaretIndex == 0;
        var isAtSearchEnd = searchBox == null || searchBox.CaretIndex >= (searchBox.Text?.Length ?? 0);

        switch (e.Key)
        {
            case Key.Escape:
                if (!string.IsNullOrEmpty(searchQuery))
                {
                    dc.GetType().GetProperty("SearchQuery")?.SetValue(dc, string.Empty);
                }
                else
                {
                    GetCommand("CloseCommand")?.Execute(null);
                }
                e.Handled = true;
                break;

            case Key.Enter:
                if (isCategoryFocused)
                {
                    GetCommand("FocusEmojisCommand")?.Execute(null);
                }
                else
                {
                    var selectedEmoji = GetProp(dc, "SelectedEmoji");
                    if (selectedEmoji != null)
                    {
                        GetCommand("SelectEmojiCommand")?.Execute(selectedEmoji);
                    }
                }
                e.Handled = true;
                break;

            case Key.Up:
                if (isCategoryFocused)
                {
                    searchBox?.Focus();
                }
                else
                {
                    var selectedEmoji = GetProp(dc, "SelectedEmoji");
                    var filteredEmojis = GetProp(dc, "FilteredEmojis");
                    var currentIndex = 0;
                    if (selectedEmoji != null && filteredEmojis != null)
                    {
                        var indexOfMethod = filteredEmojis.GetType().GetMethod("IndexOf");
                        if (indexOfMethod != null)
                            currentIndex = (int)(indexOfMethod.Invoke(filteredEmojis, [selectedEmoji]) ?? 0);
                    }

                    if (currentIndex < ColumnsPerRow)
                    {
                        GetCommand("FocusCategoriesCommand")?.Execute(null);
                    }
                    else
                    {
                        GetCommand("SelectPreviousRowCommand")?.Execute(ColumnsPerRow);
                    }
                }
                e.Handled = true;
                break;

            case Key.Down:
                if (isCategoryFocused)
                {
                    GetCommand("FocusEmojisCommand")?.Execute(null);
                }
                else
                {
                    var selectedEmoji = GetProp(dc, "SelectedEmoji");
                    var filteredEmojis = GetProp(dc, "FilteredEmojis");
                    var count = filteredEmojis?.GetType().GetProperty("Count")?.GetValue(filteredEmojis) as int? ?? 0;

                    if (selectedEmoji == null && count > 0)
                    {
                        GetCommand("FocusEmojisCommand")?.Execute(null);
                    }
                    else
                    {
                        GetCommand("SelectNextRowCommand")?.Execute(ColumnsPerRow);
                    }
                }
                e.Handled = true;
                break;

            case Key.Left:
                if (isCategoryFocused)
                {
                    GetCommand("SelectPreviousCategoryCommand")?.Execute(null);
                    e.Handled = true;
                }
                else if (isSearchEmpty || isAtSearchStart)
                {
                    GetCommand("SelectPreviousCommand")?.Execute(null);
                    e.Handled = true;
                }
                break;

            case Key.Right:
                if (isCategoryFocused)
                {
                    GetCommand("SelectNextCategoryCommand")?.Execute(null);
                    e.Handled = true;
                }
                else if (isSearchEmpty || isAtSearchEnd)
                {
                    GetCommand("SelectNextCommand")?.Execute(null);
                    e.Handled = true;
                }
                break;
        }
    }

    private void OnBackdropPressed(object? sender, PointerPressedEventArgs e)
    {
        GetCommand("CloseCommand")?.Execute(null);
    }

    private void OnEmojiPressed(object? sender, PointerPressedEventArgs e)
    {
        if (sender is Border border)
        {
            var emojiItem = border.DataContext;
            if (emojiItem != null)
            {
                GetCommand("SelectEmojiCommand")?.Execute(emojiItem);
            }
        }
    }

    private void OnCategoryPressed(object? sender, PointerPressedEventArgs e)
    {
        if (sender is Border border)
        {
            var category = border.DataContext;
            if (category != null)
            {
                GetCommand("SelectCategoryCommand")?.Execute(category);
                DataContext?.GetType().GetProperty("IsCategoryFocused")?.SetValue(DataContext, true);
                UpdateHighlights();
            }
        }
    }
}
