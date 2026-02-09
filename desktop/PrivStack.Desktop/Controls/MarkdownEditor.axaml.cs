using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using AvaloniaEdit;
using AvaloniaEdit.Document;

namespace PrivStack.Desktop.Controls;

/// <summary>
/// A simple Markdown editor with toolbar.
/// Stores content as plain Markdown for CRDT compatibility.
/// </summary>
public partial class MarkdownEditor : UserControl
{
    private TextEditor? _editor;
    private bool _isUpdatingFromCode;

    #region Styled Properties

    public static readonly StyledProperty<string> MarkdownProperty =
        AvaloniaProperty.Register<MarkdownEditor, string>(
            nameof(Markdown),
            defaultValue: string.Empty,
            defaultBindingMode: BindingMode.TwoWay);

    public static readonly StyledProperty<bool> IsEditingProperty =
        AvaloniaProperty.Register<MarkdownEditor, bool>(nameof(IsEditing), defaultValue: false);

    public static readonly StyledProperty<bool> IsReadOnlyProperty =
        AvaloniaProperty.Register<MarkdownEditor, bool>(nameof(IsReadOnly), defaultValue: false);

    public static readonly StyledProperty<string> PlaceholderProperty =
        AvaloniaProperty.Register<MarkdownEditor, string>(nameof(Placeholder), defaultValue: "Start writing...");

    /// <summary>
    /// The markdown content.
    /// </summary>
    public string Markdown
    {
        get => GetValue(MarkdownProperty);
        set => SetValue(MarkdownProperty, value);
    }

    /// <summary>
    /// Whether the editor is in editing mode (shows toolbar and editor).
    /// </summary>
    public bool IsEditing
    {
        get => GetValue(IsEditingProperty);
        set => SetValue(IsEditingProperty, value);
    }

    /// <summary>
    /// Whether the editor is read-only.
    /// </summary>
    public bool IsReadOnly
    {
        get => GetValue(IsReadOnlyProperty);
        set => SetValue(IsReadOnlyProperty, value);
    }

    /// <summary>
    /// Placeholder text when editor is empty.
    /// </summary>
    public string Placeholder
    {
        get => GetValue(PlaceholderProperty);
        set => SetValue(PlaceholderProperty, value);
    }

    #endregion

    #region Events

    public static readonly RoutedEvent<RoutedEventArgs> MarkdownChangedEvent =
        RoutedEvent.Register<MarkdownEditor, RoutedEventArgs>(nameof(MarkdownChanged), RoutingStrategies.Bubble);

    public event EventHandler<RoutedEventArgs>? MarkdownChanged
    {
        add => AddHandler(MarkdownChangedEvent, value);
        remove => RemoveHandler(MarkdownChangedEvent, value);
    }

    public static readonly RoutedEvent<ImageInsertEventArgs> ImageInsertRequestedEvent =
        RoutedEvent.Register<MarkdownEditor, ImageInsertEventArgs>(nameof(ImageInsertRequested), RoutingStrategies.Bubble);

    public event EventHandler<ImageInsertEventArgs>? ImageInsertRequested
    {
        add => AddHandler(ImageInsertRequestedEvent, value);
        remove => RemoveHandler(ImageInsertRequestedEvent, value);
    }

    #endregion

    public MarkdownEditor()
    {
        InitializeComponent();
    }

    protected override void OnLoaded(RoutedEventArgs e)
    {
        base.OnLoaded(e);

        _editor = this.FindControl<TextEditor>("Editor");
        if (_editor != null)
        {
            _editor.Document = new TextDocument(Markdown ?? string.Empty);
            _editor.TextArea.TextEntering += OnTextEntering;
            _editor.KeyDown += OnEditorKeyDown;

            // Set up syntax highlighting for Markdown
            _editor.SyntaxHighlighting = AvaloniaEdit.Highlighting.HighlightingManager.Instance.GetDefinition("MarkDown");
        }
    }

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == MarkdownProperty && _editor != null && !_isUpdatingFromCode)
        {
            _isUpdatingFromCode = true;
            var newValue = change.GetNewValue<string>() ?? string.Empty;
            if (_editor.Document.Text != newValue)
            {
                _editor.Document.Text = newValue;
            }
            _isUpdatingFromCode = false;
        }

        // When switching to edit mode, ensure editor has latest content and focus
        if (change.Property == IsEditingProperty && change.GetNewValue<bool>() && _editor != null)
        {
            _isUpdatingFromCode = true;
            if (_editor.Document.Text != Markdown)
            {
                _editor.Document.Text = Markdown ?? string.Empty;
            }
            _isUpdatingFromCode = false;

            // Focus the editor when entering edit mode
            Avalonia.Threading.Dispatcher.UIThread.Post(() => _editor?.Focus());
        }
    }

    private void OnEditorTextChanged(object? sender, EventArgs e)
    {
        if (_editor == null || _isUpdatingFromCode) return;

        _isUpdatingFromCode = true;
        Markdown = _editor.Document.Text;
        _isUpdatingFromCode = false;

        RaiseEvent(new RoutedEventArgs(MarkdownChangedEvent));
    }

    private void OnTextEntering(object? sender, TextInputEventArgs e)
    {
        // Auto-complete for lists could be added here
    }

    private void OnEditorKeyDown(object? sender, KeyEventArgs e)
    {
        if (_editor == null) return;

        // Keyboard shortcuts
        if (e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.KeyModifiers.HasFlag(KeyModifiers.Meta))
        {
            switch (e.Key)
            {
                case Key.B:
                    WrapSelection("**", "**");
                    e.Handled = true;
                    break;
                case Key.I:
                    WrapSelection("*", "*");
                    e.Handled = true;
                    break;
                case Key.K:
                    InsertLink();
                    e.Handled = true;
                    break;
            }
        }
    }

    #region Toolbar Click Handlers

    private void OnBoldClick(object? sender, RoutedEventArgs e) => WrapSelection("**", "**");
    private void OnItalicClick(object? sender, RoutedEventArgs e) => WrapSelection("*", "*");
    private void OnStrikethroughClick(object? sender, RoutedEventArgs e) => WrapSelection("~~", "~~");
    private void OnInlineCodeClick(object? sender, RoutedEventArgs e) => WrapSelection("`", "`");

    private void OnHeading1Click(object? sender, RoutedEventArgs e) => PrefixLine("# ");
    private void OnHeading2Click(object? sender, RoutedEventArgs e) => PrefixLine("## ");
    private void OnHeading3Click(object? sender, RoutedEventArgs e) => PrefixLine("### ");

    private void OnBulletListClick(object? sender, RoutedEventArgs e) => PrefixLine("- ");
    private void OnNumberedListClick(object? sender, RoutedEventArgs e) => PrefixLine("1. ");
    private void OnTaskListClick(object? sender, RoutedEventArgs e) => PrefixLine("- [ ] ");

    private void OnLinkClick(object? sender, RoutedEventArgs e) => InsertLink();

    private async void OnImageClick(object? sender, RoutedEventArgs e)
    {
        var topLevel = TopLevel.GetTopLevel(this);
        if (topLevel == null) return;

        var files = await topLevel.StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Select Image",
            AllowMultiple = false,
            FileTypeFilter = new[]
            {
                new FilePickerFileType("Images") { Patterns = new[] { "*.png", "*.jpg", "*.jpeg", "*.gif", "*.webp", "*.svg" } }
            }
        });

        if (files.Count > 0)
        {
            var file = files[0];
            var args = new ImageInsertEventArgs(file);
            RaiseEvent(args);

            if (!string.IsNullOrEmpty(args.ResultMarkdownPath))
            {
                InsertText($"![{args.AltText ?? file.Name}]({args.ResultMarkdownPath})");
            }
        }
    }

    private void OnCodeBlockClick(object? sender, RoutedEventArgs e)
    {
        if (_editor == null) return;

        var selection = _editor.SelectedText;
        if (string.IsNullOrEmpty(selection))
        {
            InsertText("```\n\n```");
            _editor.CaretOffset -= 4;
        }
        else
        {
            WrapSelection("```\n", "\n```");
        }
    }

    private void OnBlockquoteClick(object? sender, RoutedEventArgs e) => PrefixLine("> ");

    private void OnHorizontalRuleClick(object? sender, RoutedEventArgs e)
    {
        if (_editor == null) return;

        var line = _editor.Document.GetLineByOffset(_editor.CaretOffset);
        var insertPos = line.EndOffset;

        _editor.Document.Insert(insertPos, "\n\n---\n\n");
        _editor.CaretOffset = insertPos + 7;
    }

    #endregion

    #region Text Manipulation Helpers

    private void WrapSelection(string before, string after)
    {
        if (_editor == null) return;

        var selection = _editor.TextArea.Selection;
        var selectedText = _editor.SelectedText;

        if (string.IsNullOrEmpty(selectedText))
        {
            // No selection - insert placeholder
            var placeholder = before == "**" ? "bold text" :
                             before == "*" ? "italic text" :
                             before == "~~" ? "strikethrough" :
                             before == "`" ? "code" : "text";

            InsertText($"{before}{placeholder}{after}");
            _editor.Select(_editor.CaretOffset - after.Length - placeholder.Length, placeholder.Length);
        }
        else
        {
            // Wrap selection
            var startOffset = _editor.SelectionStart;
            _editor.Document.Replace(selection.SurroundingSegment, $"{before}{selectedText}{after}");
            _editor.Select(startOffset + before.Length, selectedText.Length);
        }

        _editor.Focus();
    }

    private void PrefixLine(string prefix)
    {
        if (_editor == null) return;

        var line = _editor.Document.GetLineByOffset(_editor.CaretOffset);
        var lineText = _editor.Document.GetText(line.Offset, line.Length);

        // Check if prefix already exists
        if (lineText.StartsWith(prefix))
        {
            // Remove prefix
            _editor.Document.Remove(line.Offset, prefix.Length);
        }
        else
        {
            // Add prefix
            _editor.Document.Insert(line.Offset, prefix);
            _editor.CaretOffset = line.Offset + prefix.Length + lineText.Length;
        }

        _editor.Focus();
    }

    private void InsertText(string text)
    {
        if (_editor == null) return;

        var offset = _editor.CaretOffset;
        _editor.Document.Insert(offset, text);
        _editor.CaretOffset = offset + text.Length;
        _editor.Focus();
    }

    private void InsertLink()
    {
        if (_editor == null) return;

        var selectedText = _editor.SelectedText;

        if (string.IsNullOrEmpty(selectedText))
        {
            InsertText("[link text](url)");
            _editor.Select(_editor.CaretOffset - 15, 9);
        }
        else
        {
            var startOffset = _editor.SelectionStart;
            _editor.Document.Replace(_editor.SelectionStart, _editor.SelectionLength, $"[{selectedText}](url)");
            _editor.Select(startOffset + selectedText.Length + 3, 3);
        }

        _editor.Focus();
    }

    #endregion

    #region Public Methods

    /// <summary>
    /// Focus the editor.
    /// </summary>
    public void FocusEditor()
    {
        _editor?.Focus();
    }

    /// <summary>
    /// Insert markdown text at the current cursor position.
    /// </summary>
    public void InsertMarkdown(string markdown)
    {
        InsertText(markdown);
    }

    /// <summary>
    /// Apply a template to the editor, replacing placeholders.
    /// </summary>
    public void ApplyTemplate(string templateContent, Dictionary<string, string>? replacements = null)
    {
        var content = templateContent;

        if (replacements != null)
        {
            foreach (var (key, value) in replacements)
            {
                content = content.Replace($"{{{{{key}}}}}", value);
            }
        }

        // Common replacements
        content = content.Replace("{{date}}", DateTime.Now.ToString("MMMM d, yyyy"));
        content = content.Replace("{{time}}", DateTime.Now.ToString("h:mm tt"));
        content = content.Replace("{{datetime}}", DateTime.Now.ToString("MMMM d, yyyy h:mm tt"));

        Markdown = content;
    }

    #endregion
}

/// <summary>
/// Event args for image insertion requests.
/// </summary>
public class ImageInsertEventArgs : RoutedEventArgs
{
    public IStorageFile File { get; }
    public string? ResultMarkdownPath { get; set; }
    public string? AltText { get; set; }

    public ImageInsertEventArgs(IStorageFile file) : base(MarkdownEditor.ImageInsertRequestedEvent)
    {
        File = file;
        AltText = Path.GetFileNameWithoutExtension(file.Name);
    }
}
