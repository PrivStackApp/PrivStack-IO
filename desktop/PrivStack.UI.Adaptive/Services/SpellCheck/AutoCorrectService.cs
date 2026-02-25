namespace PrivStack.UI.Adaptive.Services.SpellCheck;

/// <summary>
/// Auto-replaces common typos when a word boundary (space, punctuation) is typed.
/// </summary>
public sealed class AutoCorrectService
{
    public static readonly AutoCorrectService Instance = new();

    public bool IsEnabled { get; set; } = true;

    private readonly Dictionary<string, string> _corrections = new(StringComparer.OrdinalIgnoreCase)
    {
        ["teh"] = "the",
        ["thier"] = "their",
        ["adn"] = "and",
        ["dont"] = "don't",
        ["doesnt"] = "doesn't",
        ["didnt"] = "didn't",
        ["cant"] = "can't",
        ["wont"] = "won't",
        ["isnt"] = "isn't",
        ["wasnt"] = "wasn't",
        ["werent"] = "weren't",
        ["hasnt"] = "hasn't",
        ["havent"] = "haven't",
        ["wouldnt"] = "wouldn't",
        ["shouldnt"] = "shouldn't",
        ["couldnt"] = "couldn't",
        ["im"] = "I'm",
        ["ive"] = "I've",
        ["thats"] = "that's",
        ["whats"] = "what's",
        ["heres"] = "here's",
        ["theres"] = "there's",
        ["lets"] = "let's",
        ["youre"] = "you're",
        ["theyre"] = "they're",
        ["weve"] = "we've",
        ["recieve"] = "receive",
        ["reciever"] = "receiver",
        ["occured"] = "occurred",
        ["occurence"] = "occurrence",
        ["seperate"] = "separate",
        ["definately"] = "definitely",
        ["accomodate"] = "accommodate",
        ["occurrance"] = "occurrence",
        ["wiht"] = "with",
        ["taht"] = "that",
        ["hte"] = "the",
        ["nto"] = "not",
        ["fro"] = "for",
        ["yuo"] = "you",
        ["nad"] = "and",
    };

    /// <summary>
    /// Returns the corrected word, or null if no autocorrect applies.
    /// </summary>
    public string? GetCorrection(string word)
    {
        if (string.IsNullOrWhiteSpace(word)) return null;
        return _corrections.TryGetValue(word, out var correction) ? correction : null;
    }
}
