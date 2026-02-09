namespace PrivStack.Desktop.Helpers;

/// <summary>
/// Provides platform-appropriate emoji. Some emoji render as monochrome/blank on Windows
/// but work fine on macOS. This class maps semantic keys to emoji that render correctly
/// on the current OS.
/// </summary>
public static class PlatformEmoji
{
    private static readonly Dictionary<string, string> Emoji;

    static PlatformEmoji()
    {
        var baseEmoji = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
        {
            // Documents/Notes
            { "Document", "📄" },
            { "Note", "📝" },
            { "Page", "📄" },

            // Media
            { "Image", "📷" },       // 🖼️ is monochrome on some Windows configs
            { "Photo", "📷" },
            { "Video", "🎬" },
            { "Audio", "🎵" },
            { "File", "📎" },

            // Calendar
            { "Calendar", "📅" },
            { "Event", "📅" },

            // Tasks
            { "Task", "✅" },
            { "Todo", "✅" },         // ☑️ renders inconsistently on Windows

            // Contacts
            { "Contact", "👤" },
            { "Person", "👤" },

            // Snippets
            { "Snippet", "📋" },      // ✂️ can be monochrome on Windows
            { "Code", "💻" },

            // RSS
            { "RSS", "📰" },
            { "Feed", "📰" },
            { "Article", "📰" },

            // Passwords
            { "Password", "🔐" },
            { "Credential", "🔑" },

            // Journal
            { "Journal", "📔" },
            { "Entry", "📔" },

            // Storage/Settings
            { "Folder", "📁" },
            { "FolderOpen", "📂" },
            { "Cloud", "☁️" },
            { "Lock", "🔒" },
            { "Pin", "📍" },
            { "Warning", "⚠" },
            { "Info", "ℹ" },
            { "Check", "✓" },
        };

        if (OperatingSystem.IsMacOS())
        {
            // macOS renders all emoji natively via Apple Color Emoji — override nothing
            baseEmoji["Image"] = "🖼️";
            baseEmoji["Snippet"] = "✂️";
            baseEmoji["Todo"] = "☑️";
        }
        else if (OperatingSystem.IsLinux())
        {
            // Noto Color Emoji has similar coverage to macOS
            baseEmoji["Image"] = "🖼️";
            baseEmoji["Snippet"] = "✂️";
            baseEmoji["Todo"] = "☑️";
        }
        // Windows uses the safe defaults set above

        Emoji = baseEmoji;
    }

    /// <summary>
    /// Gets the platform-appropriate emoji for the given semantic key.
    /// Returns the key itself if no mapping exists.
    /// </summary>
    public static string Get(string key)
    {
        return Emoji.TryGetValue(key, out var emoji) ? emoji : key;
    }

    /// <summary>
    /// Gets the full emoji dictionary (for converters that need bulk access).
    /// </summary>
    public static IReadOnlyDictionary<string, string> All => Emoji;
}
