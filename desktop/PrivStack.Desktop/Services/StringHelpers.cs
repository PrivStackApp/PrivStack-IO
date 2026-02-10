namespace PrivStack.Desktop.Services;

/// <summary>
/// Common string utility methods used by plugins.
/// </summary>
public static class StringHelpers
{
    /// <summary>
    /// Strips emoji and surrogate characters from a string, preserving standard text.
    /// </summary>
    public static string StripEmojis(string input)
    {
        if (string.IsNullOrEmpty(input)) return input;
        return string.Concat(input.Where(c => !char.IsSurrogate(c) || char.IsLetterOrDigit(c)));
    }
}
