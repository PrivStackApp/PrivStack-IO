namespace PrivStack.Sdk;

/// <summary>
/// Abstraction over spell checking for use by plugins.
/// </summary>
public interface ISpellCheckService
{
    bool IsLoaded { get; }
    bool Check(string word);
    IEnumerable<string> Suggest(string word, int maxSuggestions = 5);
    void AddToUserDictionary(string word);
}
