using PrivStack.Sdk;
using Serilog;
using WeCantSpell.Hunspell;

namespace PrivStack.Desktop.Services.SpellCheck;

/// <summary>
/// Provides spell checking functionality using Hunspell dictionaries.
/// </summary>
public sealed class SpellCheckService : ISpellCheckService
{
    private static readonly ILogger _log = Log.ForContext<SpellCheckService>();
    private static SpellCheckService? _instance;
    private static readonly object _instanceLock = new();

    private WordList? _dictionary;
    private readonly UserDictionaryService _userDictionary;
    private bool _isLoading;

    /// <summary>
    /// Gets the singleton instance of the SpellCheckService.
    /// </summary>
    public static SpellCheckService Instance
    {
        get
        {
            if (_instance == null)
            {
                lock (_instanceLock)
                {
                    if (_instance == null)
                    {
                        _instance = new SpellCheckService();
                        HostServices.SpellCheck = _instance;
                    }
                }
            }
            return _instance;
        }
    }

    private SpellCheckService()
    {
        _userDictionary = UserDictionaryService.Instance;
        _ = LoadDictionaryAsync();
    }

    /// <summary>
    /// Gets whether the dictionary has been loaded and is ready for use.
    /// </summary>
    public bool IsLoaded => _dictionary != null;

    /// <summary>
    /// Gets whether the dictionary is currently loading.
    /// </summary>
    public bool IsLoading => _isLoading;

    /// <summary>
    /// Checks if a word is spelled correctly.
    /// </summary>
    /// <param name="word">The word to check.</param>
    /// <returns>True if the word is correct or in the user dictionary; false otherwise.</returns>
    public bool Check(string word)
    {
        if (string.IsNullOrWhiteSpace(word))
            return true;

        // Always accept words in user dictionary
        if (_userDictionary.Contains(word))
            return true;

        // If dictionary not loaded, assume correct
        if (_dictionary == null)
            return true;

        return _dictionary.Check(word);
    }

    /// <summary>
    /// Gets spelling suggestions for a misspelled word.
    /// </summary>
    /// <param name="word">The misspelled word.</param>
    /// <param name="maxSuggestions">Maximum number of suggestions to return.</param>
    /// <returns>A list of spelling suggestions.</returns>
    public IEnumerable<string> Suggest(string word, int maxSuggestions = 5)
    {
        if (string.IsNullOrWhiteSpace(word) || _dictionary == null)
            return [];

        return _dictionary.Suggest(word).Take(maxSuggestions);
    }

    /// <summary>
    /// Adds a word to the user dictionary so it won't be flagged as misspelled.
    /// </summary>
    /// <param name="word">The word to add.</param>
    public void AddToUserDictionary(string word)
    {
        _userDictionary.AddWord(word);
    }

    /// <summary>
    /// Removes a word from the user dictionary.
    /// </summary>
    /// <param name="word">The word to remove.</param>
    public void RemoveFromUserDictionary(string word)
    {
        _userDictionary.RemoveWord(word);
    }

    /// <summary>
    /// Checks if a word is in the user dictionary.
    /// </summary>
    public bool IsInUserDictionary(string word)
    {
        return _userDictionary.Contains(word);
    }

    private async Task LoadDictionaryAsync()
    {
        if (_isLoading || _dictionary != null)
            return;

        _isLoading = true;
        _log.Information("Loading spell check dictionary...");

        try
        {
            await Task.Run(() =>
            {
                var dictDir = Path.Combine(AppContext.BaseDirectory, "Dictionaries");
                var dicPath = Path.Combine(dictDir, "en_US.dic");
                var affPath = Path.Combine(dictDir, "en_US.aff");

                if (!File.Exists(dicPath) || !File.Exists(affPath))
                {
                    _log.Error("Dictionary files not found in {Dir}", dictDir);
                    return;
                }

                using var dicStream = File.OpenRead(dicPath);
                using var affStream = File.OpenRead(affPath);
                _dictionary = WordList.CreateFromStreams(dicStream, affStream);
                _log.Information("Spell check dictionary loaded successfully");
            });
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to load spell check dictionary");
        }
        finally
        {
            _isLoading = false;
        }
    }

    /// <summary>
    /// Ensures the dictionary is loaded. Call this at app startup to pre-load.
    /// </summary>
    public async Task EnsureLoadedAsync()
    {
        if (_dictionary != null)
            return;

        await LoadDictionaryAsync();
    }
}
