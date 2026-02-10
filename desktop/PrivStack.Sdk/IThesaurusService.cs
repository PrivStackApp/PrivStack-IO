namespace PrivStack.Sdk;

/// <summary>
/// Abstraction over thesaurus lookup for use by plugins.
/// </summary>
public interface IThesaurusService
{
    bool IsLoaded { get; }
    bool HasSynonyms(string word);
    IEnumerable<string> GetSynonymsFlat(string word, int maxSynonyms = 10);
}
