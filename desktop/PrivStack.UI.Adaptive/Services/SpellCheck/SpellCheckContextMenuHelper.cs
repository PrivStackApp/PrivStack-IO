using Avalonia.Controls;
using PrivStack.Sdk;
using Serilog;

namespace PrivStack.UI.Adaptive.Services.SpellCheck;

/// <summary>
/// Result of extracting a word at a cursor position.
/// </summary>
/// <param name="Start">The start index of the word in the text.</param>
/// <param name="End">The end index of the word in the text.</param>
/// <param name="Word">The extracted word.</param>
public record WordAtCaret(int Start, int End, string Word);

/// <summary>
/// Helper class for building spell check and thesaurus context menus.
/// </summary>
public static class SpellCheckContextMenuHelper
{
    /// <summary>
    /// Gets the word at the specified caret position.
    /// </summary>
    /// <param name="text">The full text.</param>
    /// <param name="caretIndex">The caret position.</param>
    /// <returns>The word at the caret, or null if no word found.</returns>
    public static WordAtCaret? GetWordAtCaret(string? text, int caretIndex)
    {
        if (string.IsNullOrEmpty(text) || caretIndex < 0)
            return null;

        // Clamp caret to valid range
        caretIndex = Math.Min(caretIndex, text.Length);

        // Find word start
        var start = caretIndex;
        while (start > 0 && IsWordChar(text[start - 1]))
            start--;

        // Find word end
        var end = caretIndex;
        while (end < text.Length && IsWordChar(text[end]))
            end++;

        if (start == end)
            return null;

        var word = text[start..end];

        // Ignore if it's just numbers or special characters
        if (string.IsNullOrWhiteSpace(word) || word.All(char.IsDigit))
            return null;

        return new WordAtCaret(start, end, word);
    }

    /// <summary>
    /// Determines if a character is part of a word.
    /// </summary>
    public static bool IsWordChar(char c)
    {
        return char.IsLetter(c) || c == '\'' || c == '-';
    }

    private static readonly ILogger _log = Log.ForContext(typeof(SpellCheckContextMenuHelper));

    /// <summary>
    /// Populates a context menu with spell check and thesaurus options.
    /// </summary>
    /// <param name="menu">The context menu to populate.</param>
    /// <param name="word">The word at the cursor.</param>
    /// <param name="onReplace">Callback when a replacement word is selected.</param>
    /// <param name="onAddToDictionary">Callback when "Add to Dictionary" is clicked.</param>
    public static void PopulateContextMenu(
        ContextMenu menu,
        WordAtCaret word,
        Action<string> onReplace,
        Action onAddToDictionary)
    {
        menu.Items.Clear();

        var spellCheck = HostServices.SpellCheck;
        var thesaurus = HostServices.Thesaurus;

        if (spellCheck == null || thesaurus == null)
        {
            menu.Items.Add(new MenuItem
            {
                Header = "Spell check not available",
                IsEnabled = false,
                FontStyle = Avalonia.Media.FontStyle.Italic
            });
            return;
        }

        _log.Debug("Context menu for word: '{Word}', SpellCheck loaded: {SpellLoaded}, Thesaurus loaded: {ThesLoaded}",
            word.Word, spellCheck.IsLoaded, thesaurus.IsLoaded);

        var isCorrect = spellCheck.Check(word.Word);
        var hasSynonyms = thesaurus.HasSynonyms(word.Word);

        _log.Debug("Word '{Word}': isCorrect={IsCorrect}, hasSynonyms={HasSynonyms}",
            word.Word, isCorrect, hasSynonyms);

        // Spelling section (if misspelled)
        if (!isCorrect)
        {
            var headerItem = new MenuItem
            {
                Header = $"Spelling: \"{word.Word}\"",
                IsEnabled = false,
                FontStyle = Avalonia.Media.FontStyle.Italic
            };
            menu.Items.Add(headerItem);
            menu.Items.Add(new Separator());

            var suggestions = spellCheck.Suggest(word.Word, 5).ToList();
            _log.Debug("Spelling suggestions for '{Word}': {Count} found", word.Word, suggestions.Count);

            if (suggestions.Count > 0)
            {
                foreach (var suggestion in suggestions)
                {
                    var suggestionItem = new MenuItem { Header = suggestion };
                    suggestionItem.Click += (_, _) => onReplace(suggestion);
                    menu.Items.Add(suggestionItem);
                }
            }
            else
            {
                menu.Items.Add(new MenuItem
                {
                    Header = "No spelling suggestions",
                    IsEnabled = false,
                    FontStyle = Avalonia.Media.FontStyle.Italic
                });
            }

            menu.Items.Add(new Separator());

            // Add to Dictionary option
            var addItem = new MenuItem { Header = "Add to Dictionary" };
            addItem.Click += (_, _) => onAddToDictionary();
            menu.Items.Add(addItem);
        }

        // Thesaurus section - flat list of synonyms
        if (hasSynonyms)
        {
            if (menu.Items.Count > 0)
                menu.Items.Add(new Separator());

            // Get all synonyms as a flat list, deduplicated
            var allSynonyms = thesaurus.GetSynonymsFlat(word.Word, 10).ToList();
            _log.Debug("Synonyms for '{Word}': {Count}", word.Word, allSynonyms.Count);

            var synonymsMenu = new MenuItem { Header = "Synonyms" };

            foreach (var synonym in allSynonyms)
            {
                var synItem = new MenuItem { Header = synonym };
                synItem.Click += (_, _) => onReplace(synonym);
                synonymsMenu.Items.Add(synItem);
            }

            menu.Items.Add(synonymsMenu);
        }

        // If word is correct but no synonyms, show helpful message
        if (menu.Items.Count == 0)
        {
            if (isCorrect)
            {
                menu.Items.Add(new MenuItem
                {
                    Header = $"\"{word.Word}\" - Correct spelling",
                    IsEnabled = false,
                    FontStyle = Avalonia.Media.FontStyle.Italic
                });
                menu.Items.Add(new MenuItem
                {
                    Header = "No synonyms available",
                    IsEnabled = false,
                    FontStyle = Avalonia.Media.FontStyle.Italic
                });
            }
            else
            {
                menu.Items.Add(new MenuItem
                {
                    Header = "No suggestions available",
                    IsEnabled = false,
                    FontStyle = Avalonia.Media.FontStyle.Italic
                });
            }
        }
    }

    /// <summary>
    /// Handles the context menu opening event for a TextBox.
    /// </summary>
    /// <param name="textBox">The TextBox that triggered the context menu.</param>
    /// <param name="e">The event args (can be cancelled if no word at cursor).</param>
    /// <param name="menu">The context menu to populate.</param>
    public static void HandleContextMenuOpening(
        TextBox textBox,
        System.ComponentModel.CancelEventArgs e,
        ContextMenu menu)
    {
        var spellCheck = HostServices.SpellCheck;
        if (spellCheck == null)
        {
            e.Cancel = true;
            return;
        }

        var word = GetWordAtCaret(textBox.Text, textBox.CaretIndex);

        if (word == null)
        {
            // No word at cursor - show default context menu or cancel
            e.Cancel = true;
            return;
        }

        PopulateContextMenu(
            menu,
            word,
            replacement => ReplaceWord(textBox, word, replacement),
            () => spellCheck.AddToUserDictionary(word.Word));
    }

    /// <summary>
    /// Replaces a word in a TextBox with a new word.
    /// </summary>
    private static void ReplaceWord(TextBox textBox, WordAtCaret word, string replacement)
    {
        var text = textBox.Text;
        if (string.IsNullOrEmpty(text))
            return;

        var newText = text[..word.Start] + replacement + text[word.End..];
        textBox.Text = newText;

        // Position caret after the replaced word
        textBox.CaretIndex = word.Start + replacement.Length;
    }
}
