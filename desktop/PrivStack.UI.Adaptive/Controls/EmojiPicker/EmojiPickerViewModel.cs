using System.Collections.ObjectModel;
using System.Text.Json;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace PrivStack.UI.Adaptive.Controls.EmojiPicker;

/// <summary>Represents an emoji category.</summary>
public sealed record EmojiCategory(string Id, string Name, string Icon);

/// <summary>Represents an emoji item.</summary>
public sealed record EmojiItem(string Emoji, string Name, string Keywords, string CategoryId);

/// <summary>
/// ViewModel for the shared Emoji Picker (Cmd/Ctrl+E).
/// Contains the full emoji database and category/search filtering.
/// </summary>
public sealed partial class EmojiPickerViewModel : ObservableObject
{
    private const int MaxRecentEmojis = 25;
    private static readonly string RecentEmojisPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "PrivStack", "recent-emojis.json");

    private readonly Action<string> _onEmojiSelected;
    private readonly List<EmojiItem> _allEmojis;
    private static List<string> _recentEmojisList = [];

    [ObservableProperty]
    private bool _isOpen;

    [ObservableProperty]
    private string _searchQuery = string.Empty;

    [ObservableProperty]
    private string _selectedCategory = "recent";

    [ObservableProperty]
    private EmojiCategory? _selectedCategoryItem;

    [ObservableProperty]
    private EmojiItem? _selectedEmoji;

    [ObservableProperty]
    private bool _isCategoryFocused;

    public ObservableCollection<EmojiCategory> Categories { get; } = [];
    public ObservableCollection<EmojiItem> FilteredEmojis { get; } = [];

    static EmojiPickerViewModel()
    {
        LoadRecentEmojis();
    }

    public EmojiPickerViewModel(Action<string> onEmojiSelected)
    {
        _onEmojiSelected = onEmojiSelected;
        _allEmojis = InitializeEmojis();
        InitializeCategories();
    }

    private void InitializeCategories()
    {
        Categories.Add(new EmojiCategory("recent", "Recent", "\ud83d\udd53"));
        Categories.Add(new EmojiCategory("smileys", "Smileys", "\ud83d\ude00"));
        Categories.Add(new EmojiCategory("people", "People", "\ud83d\udc4b"));
        Categories.Add(new EmojiCategory("animals", "Animals", "\ud83d\udc36"));
        Categories.Add(new EmojiCategory("food", "Food", "\ud83c\udf54"));
        Categories.Add(new EmojiCategory("travel", "Travel", "\u2708\ufe0f"));
        Categories.Add(new EmojiCategory("activities", "Activities", "\u26bd"));
        Categories.Add(new EmojiCategory("objects", "Objects", "\ud83d\udca1"));
        Categories.Add(new EmojiCategory("symbols", "Symbols", "\u2764\ufe0f"));
        Categories.Add(new EmojiCategory("flags", "Flags", "\ud83c\udff3\ufe0f"));
    }

    private static void LoadRecentEmojis()
    {
        try
        {
            if (File.Exists(RecentEmojisPath))
            {
                var json = File.ReadAllText(RecentEmojisPath);
                _recentEmojisList = JsonSerializer.Deserialize<List<string>>(json) ?? [];
            }
        }
        catch
        {
            _recentEmojisList = [];
        }
    }

    private static void SaveRecentEmojis()
    {
        try
        {
            var dir = Path.GetDirectoryName(RecentEmojisPath);
            if (!string.IsNullOrEmpty(dir))
                Directory.CreateDirectory(dir);

            var json = JsonSerializer.Serialize(_recentEmojisList);
            File.WriteAllText(RecentEmojisPath, json);
        }
        catch
        {
            // Non-critical -- ignore persistence failures
        }
    }

    private static void AddToRecentEmojis(string emoji)
    {
        _recentEmojisList.Remove(emoji);
        _recentEmojisList.Insert(0, emoji);
        if (_recentEmojisList.Count > MaxRecentEmojis)
            _recentEmojisList = _recentEmojisList.Take(MaxRecentEmojis).ToList();
        SaveRecentEmojis();
    }

    partial void OnSearchQueryChanged(string value)
    {
        FilterEmojis();
    }

    partial void OnSelectedCategoryChanged(string value)
    {
        SelectedCategoryItem = Categories.FirstOrDefault(c => c.Id == value);
        if (string.IsNullOrEmpty(SearchQuery))
        {
            FilterEmojis();
        }
    }

    partial void OnIsOpenChanged(bool value)
    {
        if (value)
        {
            SearchQuery = string.Empty;
            SelectedCategory = "recent";
            IsCategoryFocused = false;
            FilterEmojis();
            SelectedEmoji = FilteredEmojis.FirstOrDefault();
        }
    }

    private void FilterEmojis()
    {
        FilteredEmojis.Clear();

        if (!string.IsNullOrEmpty(SearchQuery))
        {
            var query = SearchQuery.ToLowerInvariant().Trim();
            var filtered = _allEmojis.Where(e =>
                e.Name.Contains(query, StringComparison.OrdinalIgnoreCase) ||
                e.Keywords.Contains(query, StringComparison.OrdinalIgnoreCase));

            foreach (var emoji in filtered.Take(50))
            {
                FilteredEmojis.Add(emoji);
            }
        }
        else if (SelectedCategory == "recent")
        {
            foreach (var recentEmoji in _recentEmojisList)
            {
                var item = _allEmojis.FirstOrDefault(e => e.Emoji == recentEmoji);
                if (item != null) FilteredEmojis.Add(item);
            }

            if (FilteredEmojis.Count == 0)
            {
                foreach (var emoji in _allEmojis.Take(25))
                {
                    FilteredEmojis.Add(emoji);
                }
            }
        }
        else
        {
            foreach (var emoji in _allEmojis.Where(e => e.CategoryId == SelectedCategory))
            {
                FilteredEmojis.Add(emoji);
            }
        }

        SelectedEmoji = FilteredEmojis.FirstOrDefault();
    }

    [RelayCommand]
    private void Open() => IsOpen = true;

    [RelayCommand]
    private void Close() => IsOpen = false;

    [RelayCommand]
    private void Toggle() => IsOpen = !IsOpen;

    [RelayCommand]
    private void SelectEmoji(EmojiItem? item)
    {
        if (item == null) return;
        AddToRecentEmojis(item.Emoji);
        _onEmojiSelected(item.Emoji);
        Close();
    }

    [RelayCommand]
    private void SelectCurrent()
    {
        if (SelectedEmoji != null)
            SelectEmoji(SelectedEmoji);
    }

    [RelayCommand]
    private void SelectCategory(EmojiCategory? category)
    {
        if (category != null)
        {
            SelectedCategory = category.Id;
            IsCategoryFocused = false;
        }
    }

    [RelayCommand]
    private void SelectNext()
    {
        if (FilteredEmojis.Count == 0) return;
        var idx = SelectedEmoji != null ? FilteredEmojis.IndexOf(SelectedEmoji) : -1;
        var next = (idx + 1) % FilteredEmojis.Count;
        SelectedEmoji = FilteredEmojis[next];
    }

    [RelayCommand]
    private void SelectPrevious()
    {
        if (FilteredEmojis.Count == 0) return;
        var idx = SelectedEmoji != null ? FilteredEmojis.IndexOf(SelectedEmoji) : 0;
        var prev = idx <= 0 ? FilteredEmojis.Count - 1 : idx - 1;
        SelectedEmoji = FilteredEmojis[prev];
    }

    [RelayCommand]
    private void SelectNextCategory()
    {
        if (Categories.Count == 0) return;
        var idx = SelectedCategoryItem != null ? Categories.IndexOf(SelectedCategoryItem) : -1;
        var next = (idx + 1) % Categories.Count;
        SelectedCategoryItem = Categories[next];
        SelectedCategory = SelectedCategoryItem.Id;
    }

    [RelayCommand]
    private void SelectPreviousCategory()
    {
        if (Categories.Count == 0) return;
        var idx = SelectedCategoryItem != null ? Categories.IndexOf(SelectedCategoryItem) : 0;
        var prev = idx <= 0 ? Categories.Count - 1 : idx - 1;
        SelectedCategoryItem = Categories[prev];
        SelectedCategory = SelectedCategoryItem.Id;
    }

    [RelayCommand]
    private void FocusCategories()
    {
        IsCategoryFocused = true;
        SelectedCategoryItem ??= Categories.FirstOrDefault();
    }

    [RelayCommand]
    private void FocusEmojis()
    {
        IsCategoryFocused = false;
        SelectedEmoji ??= FilteredEmojis.FirstOrDefault();
    }

    [RelayCommand]
    private void SelectPreviousRow(int columnsPerRow)
    {
        if (FilteredEmojis.Count == 0 || SelectedEmoji == null) return;
        var idx = FilteredEmojis.IndexOf(SelectedEmoji);
        var prev = idx - columnsPerRow;
        if (prev >= 0)
            SelectedEmoji = FilteredEmojis[prev];
    }

    [RelayCommand]
    private void SelectNextRow(int columnsPerRow)
    {
        if (FilteredEmojis.Count == 0 || SelectedEmoji == null) return;
        var idx = FilteredEmojis.IndexOf(SelectedEmoji);
        var next = idx + columnsPerRow;
        if (next < FilteredEmojis.Count)
            SelectedEmoji = FilteredEmojis[next];
    }

    private static List<EmojiItem> InitializeEmojis()
    {
        return
        [
            // Smileys
            new EmojiItem("\ud83d\ude00", "Grinning Face", "smile happy joy laugh grin", "smileys"),
            new EmojiItem("\ud83d\ude01", "Beaming Face", "smile happy eyes joy grin", "smileys"),
            new EmojiItem("\ud83d\ude02", "Face with Tears of Joy", "lol laugh cry happy funny", "smileys"),
            new EmojiItem("\ud83e\udd23", "Rolling on the Floor Laughing", "rofl laugh lol funny", "smileys"),
            new EmojiItem("\ud83d\ude03", "Smiling Face with Open Mouth", "smile happy joy", "smileys"),
            new EmojiItem("\ud83d\ude04", "Smiling Face with Open Mouth and Smiling Eyes", "smile happy joy", "smileys"),
            new EmojiItem("\ud83d\ude05", "Smiling Face with Open Mouth and Cold Sweat", "smile relief nervous", "smileys"),
            new EmojiItem("\ud83d\ude06", "Smiling Face with Open Mouth and Tightly-Closed Eyes", "laugh smile xd", "smileys"),
            new EmojiItem("\ud83d\ude09", "Winking Face", "wink flirt playful", "smileys"),
            new EmojiItem("\ud83d\ude0a", "Smiling Face with Smiling Eyes", "smile blush happy", "smileys"),
            new EmojiItem("\ud83d\ude0d", "Smiling Face with Heart-Eyes", "love heart adore crush", "smileys"),
            new EmojiItem("\ud83d\ude18", "Face Blowing a Kiss", "kiss love flirt", "smileys"),
            new EmojiItem("\ud83d\ude17", "Kissing Face", "kiss love", "smileys"),
            new EmojiItem("\ud83d\ude0b", "Face Savoring Food", "yum tasty delicious", "smileys"),
            new EmojiItem("\ud83e\udd14", "Thinking Face", "think hmm wonder consider", "smileys"),
            new EmojiItem("\ud83d\ude10", "Neutral Face", "meh neutral indifferent", "smileys"),
            new EmojiItem("\ud83d\ude11", "Expressionless Face", "blank expressionless", "smileys"),
            new EmojiItem("\ud83d\ude36", "Face Without Mouth", "silent speechless", "smileys"),
            new EmojiItem("\ud83d\ude44", "Face with Rolling Eyes", "eyeroll annoyed", "smileys"),
            new EmojiItem("\ud83d\ude0f", "Smirking Face", "smirk sly suggestive", "smileys"),
            new EmojiItem("\ud83d\ude23", "Persevering Face", "struggle endure", "smileys"),
            new EmojiItem("\ud83d\ude25", "Disappointed but Relieved Face", "relief sad phew", "smileys"),
            new EmojiItem("\ud83d\ude2e", "Face with Open Mouth", "surprise shock wow", "smileys"),
            new EmojiItem("\ud83d\ude31", "Face Screaming in Fear", "scream scared horror", "smileys"),
            new EmojiItem("\ud83d\ude21", "Pouting Face", "angry mad furious rage", "smileys"),
            new EmojiItem("\ud83d\ude22", "Crying Face", "cry sad tear", "smileys"),
            new EmojiItem("\ud83d\ude2d", "Loudly Crying Face", "sob cry sad bawl", "smileys"),
            new EmojiItem("\ud83d\ude34", "Sleeping Face", "sleep zzz tired", "smileys"),
            new EmojiItem("\ud83e\udd70", "Smiling Face with Hearts", "love adore happy", "smileys"),
            new EmojiItem("\ud83e\udd73", "Partying Face", "party celebrate birthday", "smileys"),

            // People
            new EmojiItem("\ud83d\udc4b", "Waving Hand", "wave hello hi bye", "people"),
            new EmojiItem("\ud83d\udc4d", "Thumbs Up", "like approve yes good", "people"),
            new EmojiItem("\ud83d\udc4e", "Thumbs Down", "dislike disapprove no bad", "people"),
            new EmojiItem("\ud83d\udc4f", "Clapping Hands", "clap applause bravo", "people"),
            new EmojiItem("\ud83d\ude4f", "Folded Hands", "pray thanks please hope", "people"),
            new EmojiItem("\ud83d\udcaa", "Flexed Biceps", "strong muscle power flex", "people"),
            new EmojiItem("\u270d\ufe0f", "Writing Hand", "write pen note", "people"),
            new EmojiItem("\ud83d\udc40", "Eyes", "look see watch observe", "people"),
            new EmojiItem("\ud83e\udde0", "Brain", "think smart intelligent mind", "people"),

            // Animals
            new EmojiItem("\ud83d\udc36", "Dog Face", "dog puppy pet animal", "animals"),
            new EmojiItem("\ud83d\udc31", "Cat Face", "cat kitten pet animal", "animals"),
            new EmojiItem("\ud83e\udd8b", "Butterfly", "butterfly insect nature", "animals"),
            new EmojiItem("\ud83d\udc1d", "Honeybee", "bee honey insect", "animals"),
            new EmojiItem("\ud83e\udd89", "Owl", "owl bird wise night", "animals"),

            // Food
            new EmojiItem("\ud83c\udf54", "Hamburger", "burger food fast", "food"),
            new EmojiItem("\ud83c\udf55", "Pizza", "pizza food slice", "food"),
            new EmojiItem("\u2615", "Hot Beverage", "coffee tea drink hot", "food"),
            new EmojiItem("\ud83c\udf7a", "Beer Mug", "beer drink alcohol cheers", "food"),
            new EmojiItem("\ud83c\udf70", "Shortcake", "cake dessert sweet", "food"),

            // Travel
            new EmojiItem("\u2708\ufe0f", "Airplane", "fly plane travel trip", "travel"),
            new EmojiItem("\ud83c\udf0d", "Globe Europe-Africa", "world earth globe", "travel"),
            new EmojiItem("\ud83c\udf05", "Sunrise", "morning sunrise sun", "travel"),
            new EmojiItem("\ud83c\udfe0", "House with Garden", "home house residence", "travel"),
            new EmojiItem("\ud83c\udfe2", "Office Building", "office work building", "travel"),

            // Activities
            new EmojiItem("\u26bd", "Soccer Ball", "soccer football sport ball", "activities"),
            new EmojiItem("\ud83c\udfc0", "Basketball", "basketball sport ball hoop", "activities"),
            new EmojiItem("\ud83c\udfaf", "Direct Hit", "target bullseye goal", "activities"),
            new EmojiItem("\ud83c\udfc6", "Trophy", "trophy win champion prize award", "activities"),
            new EmojiItem("\ud83c\udfb5", "Musical Note", "music note song melody", "activities"),

            // Objects
            new EmojiItem("\ud83d\udca1", "Light Bulb", "idea bulb light bright", "objects"),
            new EmojiItem("\ud83d\udcbb", "Laptop Computer", "computer laptop code dev", "objects"),
            new EmojiItem("\ud83d\udcf1", "Mobile Phone", "phone mobile cell device", "objects"),
            new EmojiItem("\ud83d\udcda", "Books", "books study read library education", "objects"),
            new EmojiItem("\ud83d\udcc4", "Page Facing Up", "page document file paper", "objects"),
            new EmojiItem("\ud83d\udccb", "Clipboard", "clipboard list tasks plan", "objects"),
            new EmojiItem("\ud83d\udcc5", "Calendar", "calendar date schedule plan", "objects"),
            new EmojiItem("\ud83d\udccc", "Pushpin", "pin location mark bookmark", "objects"),
            new EmojiItem("\ud83d\udcce", "Paperclip", "clip attachment attach", "objects"),
            new EmojiItem("\u2699\ufe0f", "Gear", "gear settings config cog", "objects"),
            new EmojiItem("\ud83d\udd12", "Locked", "lock secure private closed", "objects"),
            new EmojiItem("\ud83d\udd13", "Unlocked", "unlock open public", "objects"),
            new EmojiItem("\ud83d\udd11", "Key", "key password access unlock", "objects"),
            new EmojiItem("\ud83d\udee1\ufe0f", "Shield", "shield protect secure safe", "objects"),
            new EmojiItem("\ud83d\udcdc", "Scroll", "scroll document ancient paper", "objects"),
            new EmojiItem("\ud83d\udcd6", "Open Book", "book read page open", "objects"),
            new EmojiItem("\ud83d\uddbc\ufe0f", "Framed Picture", "picture image art frame photo", "objects"),

            // Symbols
            new EmojiItem("\u2764\ufe0f", "Red Heart", "love heart red", "symbols"),
            new EmojiItem("\u2705", "Check Mark", "check done complete yes", "symbols"),
            new EmojiItem("\u274c", "Cross Mark", "cross no wrong error", "symbols"),
            new EmojiItem("\u2b50", "Star", "star favorite bookmark", "symbols"),
            new EmojiItem("\u26a0\ufe0f", "Warning", "warning caution alert danger", "symbols"),
            new EmojiItem("\u2139\ufe0f", "Information", "info information help", "symbols"),
            new EmojiItem("\u2753", "Question Mark", "question help ask what why", "symbols"),
            new EmojiItem("\u2757", "Exclamation Mark", "exclamation important alert bang", "symbols"),
            new EmojiItem("\u27a1\ufe0f", "Right Arrow", "arrow right next forward", "symbols"),
            new EmojiItem("\ud83d\udd04", "Counterclockwise Arrows", "refresh reload sync update", "symbols"),

            // Flags
            new EmojiItem("\ud83c\udff3\ufe0f", "White Flag", "flag white surrender", "flags"),
            new EmojiItem("\ud83c\udff4", "Black Flag", "flag black pirate", "flags"),
            new EmojiItem("\ud83c\udffc", "Checkered Flag", "finish race complete flag", "flags"),
        ];
    }
}
