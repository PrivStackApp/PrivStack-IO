namespace PrivStack.Desktop.ViewModels.AiTray;

/// <summary>
/// Wiki-link picker integration for the chat input. Exposes the picker ViewModel
/// for XAML binding and handles link insertion into chat text.
/// </summary>
public partial class AiSuggestionTrayViewModel
{
    private ChatLinkPickerViewModel? _chatLinkPicker;

    /// <summary>
    /// Lazy-initialized link picker ViewModel. Created on first access to avoid
    /// constructor bloat when picker is never used.
    /// </summary>
    internal ChatLinkPickerViewModel ChatLinkPicker
    {
        get
        {
            if (_chatLinkPicker == null)
            {
                _chatLinkPicker = new ChatLinkPickerViewModel(_pluginRegistry);
                _chatLinkPicker.ItemSelected += OnLinkPickerItemSelected;
            }
            return _chatLinkPicker;
        }
    }

    /// <summary>
    /// Called by the code-behind when [[ is detected in the chat input.
    /// Opens the picker and removes the [[ trigger characters.
    /// </summary>
    internal void OpenLinkPicker()
    {
        ChatLinkPicker.Open();
    }

    private void OnLinkPickerItemSelected(string wikiLink)
    {
        // Insert the wiki-link at the current text position (append to existing text)
        var current = ChatInputText ?? "";
        ChatInputText = current + wikiLink + " ";
    }
}
