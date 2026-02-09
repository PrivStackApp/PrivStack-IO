using Serilog;

namespace PrivStack.Desktop.Services.SpellCheck;

/// <summary>
/// Manages a custom user dictionary for spell checking.
/// Words added by the user are persisted to a text file.
/// </summary>
public sealed class UserDictionaryService
{
    private static readonly ILogger _log = Log.ForContext<UserDictionaryService>();
    private static UserDictionaryService? _instance;
    private static readonly object _instanceLock = new();

    private readonly string _dictionaryPath;
    private readonly HashSet<string> _customWords = new(StringComparer.OrdinalIgnoreCase);
    private bool _isDirty;
    private System.Timers.Timer? _saveTimer;

    /// <summary>
    /// Gets the singleton instance of the UserDictionaryService.
    /// </summary>
    public static UserDictionaryService Instance
    {
        get
        {
            if (_instance == null)
            {
                lock (_instanceLock)
                {
                    _instance ??= new UserDictionaryService();
                }
            }
            return _instance;
        }
    }

    private UserDictionaryService()
    {
        var folder = DataPaths.BaseDir;
        Directory.CreateDirectory(folder);
        _dictionaryPath = Path.Combine(folder, "user-dictionary.txt");
        Load();
    }

    /// <summary>
    /// Gets the number of words in the user dictionary.
    /// </summary>
    public int WordCount => _customWords.Count;

    /// <summary>
    /// Checks if the user dictionary contains the specified word.
    /// </summary>
    public bool Contains(string word)
    {
        if (string.IsNullOrWhiteSpace(word))
            return false;
        return _customWords.Contains(word);
    }

    /// <summary>
    /// Adds a word to the user dictionary.
    /// </summary>
    public void AddWord(string word)
    {
        if (string.IsNullOrWhiteSpace(word))
            return;

        word = word.Trim();
        if (_customWords.Add(word))
        {
            _isDirty = true;
            SaveDebounced();
            _log.Debug("Added '{Word}' to user dictionary", word);
        }
    }

    /// <summary>
    /// Removes a word from the user dictionary.
    /// </summary>
    public void RemoveWord(string word)
    {
        if (string.IsNullOrWhiteSpace(word))
            return;

        if (_customWords.Remove(word))
        {
            _isDirty = true;
            SaveDebounced();
            _log.Debug("Removed '{Word}' from user dictionary", word);
        }
    }

    /// <summary>
    /// Gets all words in the user dictionary.
    /// </summary>
    public IEnumerable<string> GetAllWords()
    {
        return _customWords.OrderBy(w => w, StringComparer.OrdinalIgnoreCase);
    }

    /// <summary>
    /// Forces an immediate save of the dictionary.
    /// </summary>
    public void Flush()
    {
        _saveTimer?.Stop();
        _saveTimer?.Dispose();
        _saveTimer = null;

        if (_isDirty)
        {
            Save();
        }
    }

    private void Load()
    {
        try
        {
            if (File.Exists(_dictionaryPath))
            {
                foreach (var line in File.ReadAllLines(_dictionaryPath))
                {
                    if (!string.IsNullOrWhiteSpace(line))
                        _customWords.Add(line.Trim());
                }
                _log.Debug("Loaded {Count} custom words from user dictionary", _customWords.Count);
            }
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to load user dictionary from {Path}", _dictionaryPath);
        }
    }

    private void SaveDebounced()
    {
        _saveTimer?.Stop();
        _saveTimer?.Dispose();

        _saveTimer = new System.Timers.Timer(500);
        _saveTimer.AutoReset = false;
        _saveTimer.Elapsed += (_, _) =>
        {
            if (_isDirty)
            {
                Save();
            }
        };
        _saveTimer.Start();
    }

    private void Save()
    {
        try
        {
            var words = _customWords.OrderBy(w => w, StringComparer.OrdinalIgnoreCase);
            File.WriteAllLines(_dictionaryPath, words);
            _isDirty = false;
            _log.Debug("Saved user dictionary with {Count} words", _customWords.Count);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to save user dictionary to {Path}", _dictionaryPath);
        }
    }
}
