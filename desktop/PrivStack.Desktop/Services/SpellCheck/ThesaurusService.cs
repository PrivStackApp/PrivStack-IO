using System.Text;
using PrivStack.Sdk;
using Serilog;

namespace PrivStack.Desktop.Services.SpellCheck;

/// <summary>
/// Represents a group of synonyms for a specific part of speech.
/// </summary>
/// <param name="PartOfSpeech">The part of speech (noun, verb, adj, adv).</param>
/// <param name="Synonyms">The list of synonyms.</param>
public record SynonymGroup(string PartOfSpeech, IReadOnlyList<string> Synonyms);

/// <summary>
/// Provides thesaurus functionality using OpenOffice thesaurus files.
/// </summary>
public sealed class ThesaurusService : IThesaurusService
{
    private static readonly ILogger _log = Log.ForContext<ThesaurusService>();
    private static ThesaurusService? _instance;
    private static readonly object _instanceLock = new();

    // Index mapping word (lowercase) to list of synonym groups
    private readonly Dictionary<string, List<SynonymGroup>> _entries = new(StringComparer.OrdinalIgnoreCase);
    private bool _isLoaded;
    private bool _isLoading;

    /// <summary>
    /// Gets the singleton instance of the ThesaurusService.
    /// </summary>
    public static ThesaurusService Instance
    {
        get
        {
            if (_instance == null)
            {
                lock (_instanceLock)
                {
                    if (_instance == null)
                    {
                        _instance = new ThesaurusService();
                        HostServices.Thesaurus = _instance;
                    }
                }
            }
            return _instance;
        }
    }

    private ThesaurusService()
    {
        _ = LoadThesaurusAsync();
    }

    /// <summary>
    /// Gets whether the thesaurus has been loaded.
    /// </summary>
    public bool IsLoaded => _isLoaded;

    /// <summary>
    /// Gets whether the thesaurus is currently loading.
    /// </summary>
    public bool IsLoading => _isLoading;

    /// <summary>
    /// Checks if synonyms are available for a word.
    /// </summary>
    public bool HasSynonyms(string word)
    {
        if (string.IsNullOrWhiteSpace(word) || !_isLoaded)
            return false;

        return _entries.ContainsKey(word.ToLowerInvariant());
    }

    /// <summary>
    /// Gets synonyms for a word, grouped by part of speech.
    /// </summary>
    /// <param name="word">The word to look up.</param>
    /// <returns>A list of synonym groups, or empty if not found.</returns>
    public IReadOnlyList<SynonymGroup> GetSynonyms(string word)
    {
        if (string.IsNullOrWhiteSpace(word) || !_isLoaded)
            return [];

        var key = word.ToLowerInvariant();
        if (_entries.TryGetValue(key, out var groups))
            return groups;

        return [];
    }

    /// <summary>
    /// Gets all synonyms for a word as a flat list (no grouping).
    /// </summary>
    /// <param name="word">The word to look up.</param>
    /// <param name="maxSynonyms">Maximum number of synonyms to return.</param>
    /// <returns>A list of synonyms.</returns>
    public IEnumerable<string> GetSynonymsFlat(string word, int maxSynonyms = 10)
    {
        return GetSynonyms(word)
            .SelectMany(g => g.Synonyms)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Take(maxSynonyms);
    }

    /// <summary>
    /// Ensures the thesaurus is loaded. Call this at app startup to pre-load.
    /// </summary>
    public async Task EnsureLoadedAsync()
    {
        if (_isLoaded)
            return;

        await LoadThesaurusAsync();
    }

    private async Task LoadThesaurusAsync()
    {
        if (_isLoading || _isLoaded)
            return;

        _isLoading = true;
        _log.Information("Loading thesaurus...");

        try
        {
            await Task.Run(() =>
            {
                var filePath = Path.Combine(AppContext.BaseDirectory, "Dictionaries", "th_en_US.dat");

                if (!File.Exists(filePath))
                {
                    _log.Error("Thesaurus file not found at {Path}", filePath);
                    return;
                }

                using var stream = File.OpenRead(filePath);
                ParseThesaurus(stream);
                _isLoaded = true;
                _log.Information("Thesaurus loaded with {Count} entries", _entries.Count);
            });
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to load thesaurus");
        }
        finally
        {
            _isLoading = false;
        }
    }

    private void ParseThesaurus(Stream stream)
    {
        using var reader = new StreamReader(stream, Encoding.UTF8);

        // First line is encoding declaration
        var encodingLine = reader.ReadLine();
        if (string.IsNullOrEmpty(encodingLine))
            return;

        while (!reader.EndOfStream)
        {
            // Read word line: word|meaning_count
            var wordLine = reader.ReadLine();
            if (string.IsNullOrEmpty(wordLine))
                continue;

            var wordParts = wordLine.Split('|');
            if (wordParts.Length < 2)
                continue;

            var word = wordParts[0].ToLowerInvariant();
            if (!int.TryParse(wordParts[1], out var meaningCount))
                continue;

            var groups = new List<SynonymGroup>();

            // Read each meaning line: (pos)|synonym1|synonym2|...
            for (var i = 0; i < meaningCount && !reader.EndOfStream; i++)
            {
                var meaningLine = reader.ReadLine();
                if (string.IsNullOrEmpty(meaningLine))
                    continue;

                var parts = meaningLine.Split('|');
                if (parts.Length < 2)
                    continue;

                // Extract part of speech (remove parentheses)
                var pos = parts[0].Trim('(', ')').Trim();

                // Get synonyms (skip generic terms and related terms markers)
                var synonyms = parts
                    .Skip(1)
                    .Where(s => !string.IsNullOrWhiteSpace(s) &&
                                !s.Contains("(generic term)", StringComparison.OrdinalIgnoreCase) &&
                                !s.Contains("(related term)", StringComparison.OrdinalIgnoreCase) &&
                                !s.Contains("(similar term)", StringComparison.OrdinalIgnoreCase))
                    .Select(s => s.Trim())
                    .Where(s => s.Length > 0)
                    .Distinct(StringComparer.OrdinalIgnoreCase)
                    .ToList();

                if (synonyms.Count > 0)
                {
                    groups.Add(new SynonymGroup(FormatPartOfSpeech(pos), synonyms));
                }
            }

            if (groups.Count > 0)
            {
                _entries[word] = groups;
            }
        }
    }

    private static string FormatPartOfSpeech(string pos)
    {
        return pos.ToLowerInvariant() switch
        {
            "noun" => "Noun",
            "verb" => "Verb",
            "adj" => "Adjective",
            "adv" => "Adverb",
            "prep" => "Preposition",
            "conj" => "Conjunction",
            "pron" => "Pronoun",
            "interj" => "Interjection",
            _ => pos
        };
    }
}
