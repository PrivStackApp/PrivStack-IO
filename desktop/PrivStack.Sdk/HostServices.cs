namespace PrivStack.Sdk;

/// <summary>
/// Static service locator for code-behind access in plugins.
/// Wired up by the host during startup.
/// </summary>
public static class HostServices
{
    public static IFontScaleService? FontScale { get; set; }
    public static ISpellCheckService? SpellCheck { get; set; }
    public static IThesaurusService? Thesaurus { get; set; }
}
