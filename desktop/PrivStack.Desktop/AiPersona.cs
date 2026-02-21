using System.Text.RegularExpressions;

namespace PrivStack.Desktop;

/// <summary>
/// Single source of truth for the AI assistant's persona and behavior.
/// Change <see cref="Name"/> to rebrand the AI everywhere in the app.
/// </summary>
public static partial class AiPersona
{
    public const string Name = "Duncan";

    /// <summary>
    /// Response budget tiers. The classifier picks one based on the user's message,
    /// which controls MaxTokens and the length guidance injected into the system prompt.
    /// </summary>
    public enum ResponseTier
    {
        /// <summary>Greetings, yes/no questions, quick factual lookups. 1-2 sentences.</summary>
        Short,
        /// <summary>Explanations, how-to answers, short creative writing. 3-5 sentences.</summary>
        Medium,
        /// <summary>Summarize a page, draft an email, long-form writing. Up to a few paragraphs.</summary>
        Long,
    }

    /// <summary>Token budget per tier.</summary>
    public static int MaxTokensFor(ResponseTier tier) => tier switch
    {
        ResponseTier.Short  => 100,
        ResponseTier.Medium => 300,
        ResponseTier.Long   => 800,
        _ => 200,
    };

    /// <summary>Length guidance sentence injected into the system prompt per tier.</summary>
    private static string LengthRule(ResponseTier tier) => tier switch
    {
        ResponseTier.Short  => "Reply in 1-2 short sentences max.",
        ResponseTier.Medium => "Reply in 3-5 sentences. Be thorough but not verbose.",
        ResponseTier.Long   => "You may use multiple paragraphs. Be thorough and well-structured, but don't pad with filler.",
        _ => "Reply in 1-2 short sentences max.",
    };

    // ── Keyword sets for classification ─────────────────────────────

    private static readonly string[] LongKeywords =
    [
        "summarize this page", "summarize the page", "summarize this note",
        "summarize this document", "summarize everything",
        "write me", "write a", "draft a", "draft an", "draft me",
        "compose", "rewrite this", "rewrite the",
        "explain in detail", "explain thoroughly",
        "full summary", "detailed summary", "long summary",
        "break down", "break this down",
        "list all", "list every", "outline",
    ];

    private static readonly string[] MediumKeywords =
    [
        "summarize", "explain", "how do", "how does", "how can", "how to",
        "what is", "what are", "what does", "why is", "why does", "why do",
        "tell me about", "describe", "compare",
        "help me with", "can you help",
        "suggest", "recommend", "brainstorm", "ideas for",
        "rephrase", "reword", "shorten", "expand",
    ];

    private static readonly string[] ShortKeywords =
    [
        "hey", "hi", "hello", "sup", "yo", "thanks", "thank you",
        "yes", "no", "ok", "okay", "sure", "nah",
        "what time", "what day", "what date",
        "weather", "temperature",
        "ping", "test", "are you there",
    ];

    /// <summary>
    /// Classifies the user's message into a response tier based on intent signals.
    /// Uses keyword matching — longest match wins, with fallback to Medium.
    /// </summary>
    public static ResponseTier Classify(string userMessage)
    {
        var lower = userMessage.ToLowerInvariant();

        foreach (var kw in LongKeywords)
            if (lower.Contains(kw)) return ResponseTier.Long;

        if (userMessage.Split(' ', StringSplitOptions.RemoveEmptyEntries).Length <= 3)
        {
            foreach (var kw in ShortKeywords)
                if (lower.Contains(kw)) return ResponseTier.Short;
        }

        foreach (var kw in MediumKeywords)
            if (lower.Contains(kw)) return ResponseTier.Medium;

        if (userMessage.Length < 20)
            return ResponseTier.Short;

        return ResponseTier.Medium;
    }

    /// <summary>
    /// Builds the full system prompt with tier-appropriate length guidance.
    /// The user's display name is injected so the assistant knows who it's talking to.
    /// </summary>
    /// <remarks>
    /// IMPORTANT: Do NOT put example conversations in this prompt. Local LLMs will
    /// parrot them verbatim instead of treating them as behavioral guidance.
    /// </remarks>
    public static string GetSystemPrompt(ResponseTier tier, string userName) => $"""
        You are {Name}. The user's name is {userName}.
        You are a concise assistant inside PrivStack, a local productivity app.
        You run offline with no internet access.
        {LengthRule(tier)}
        Never describe yourself or say what you are.
        Never use filler phrases.
        Never repeat prior answers.
        Plain text only, no formatting.
        If asked about weather, live data, or anything requiring internet, say you can't access the web in one short sentence.
        """;

    // ── Response sanitization ────────────────────────────────────────

    [GeneratedRegex(@"<\|?(system|user|assistant|end|im_start|im_end)\|?>", RegexOptions.IgnoreCase)]
    private static partial Regex ChatTokenPattern();

    [GeneratedRegex(@"^\s*-\s*(User|Assistant|Duncan|System)\s*:", RegexOptions.IgnoreCase | RegexOptions.Multiline)]
    private static partial Regex RolePrefixPattern();

    /// <summary>
    /// Strips raw chat template tokens and role prefixes that local models sometimes leak.
    /// </summary>
    public static string Sanitize(string response)
    {
        if (string.IsNullOrEmpty(response)) return response;

        // Strip raw chat tokens like <|assistant|>, <|end|>, <|im_start|>, etc.
        var cleaned = ChatTokenPattern().Replace(response, "");

        // Strip lines that start with role prefixes like "- User:", "Assistant:", "Duncan:"
        cleaned = RolePrefixPattern().Replace(cleaned, "");

        // Collapse excessive whitespace from removed tokens
        cleaned = cleaned.Trim();

        return cleaned;
    }
}
