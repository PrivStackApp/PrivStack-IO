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

    /// <summary>Token budget per tier (local models).</summary>
    public static int MaxTokensFor(ResponseTier tier) => tier switch
    {
        ResponseTier.Short  => 80,
        ResponseTier.Medium => 400,
        ResponseTier.Long   => 800,
        _ => 200,
    };

    /// <summary>
    /// Token budget per tier for cloud models. Cloud providers handle their own limits
    /// and bill per token, so we use generous ceilings to avoid truncation errors.
    /// The system prompt's length guidance still controls actual verbosity.
    /// </summary>
    public static int CloudMaxTokensFor(ResponseTier tier) => tier switch
    {
        ResponseTier.Short  => 1024,
        ResponseTier.Medium => 4096,
        ResponseTier.Long   => 8192,
        _ => 4096,
    };

    /// <summary>Max sentences to keep per tier during post-processing truncation.</summary>
    private static int MaxSentences(ResponseTier tier) => tier switch
    {
        ResponseTier.Short  => 3,
        ResponseTier.Medium => 8,
        ResponseTier.Long   => 40,
        _ => 4,
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
        "create a", "create an", "add a", "add an", "make a", "make an",
        "log a", "log my", "draft an email", "save a", "subscribe to",
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
    /// Builds a richer system prompt for cloud models (Anthropic, OpenAI, Gemini).
    /// Includes persona detail, memory context, and relaxed formatting constraints.
    /// </summary>
    public static string GetCloudSystemPrompt(ResponseTier tier, string userName, string? memoryContext)
    {
        var brevity = tier switch
        {
            ResponseTier.Short  => "Keep your answer to 1-2 sentences.",
            ResponseTier.Medium => "Answer in a few sentences, up to a short paragraph.",
            ResponseTier.Long   => "Give a thorough, detailed answer. Use paragraphs if helpful.",
            _ => "Be concise.",
        };

        var memoryBlock = string.IsNullOrEmpty(memoryContext)
            ? ""
            : $"\n\n{memoryContext}";

        return $"""
            You are {Name}, a knowledgeable personal assistant built into PrivStack (a privacy-first productivity app). The user is {userName}. {brevity}

            You have personality — you're friendly, direct, and occasionally witty. You remember things about the user across conversations. Never mention being an AI or a language model. You are {Name}. When data from the user's workspace is provided below, always reference it for accurate answers. Never guess or fabricate numbers — only state what the data shows.{memoryBlock}
            """;
    }

    /// <summary>
    /// Builds the system prompt. Kept extremely minimal for local LLM compatibility.
    /// </summary>
    /// <remarks>
    /// Small local models (Phi-3, Mistral 7B) cannot follow complex multi-rule prompts.
    /// Keep this to the absolute minimum number of short, direct sentences.
    /// Do NOT add examples — local models parrot them verbatim.
    /// Do NOT add numbered rules — local models echo the list back.
    /// </remarks>
    public static string GetSystemPrompt(ResponseTier tier, string userName)
    {
        var brevity = tier switch
        {
            ResponseTier.Short  => "Answer in one sentence only.",
            ResponseTier.Medium => "Keep your answer to a few sentences.",
            ResponseTier.Long   => "Give a thorough answer.",
            _ => "Be brief.",
        };

        return $"""
            You are {Name}, a concise offline assistant. The user is {userName}. {brevity} Never mention being an AI. No markdown. No lists. No notes or disclaimers. When data is provided below, always use it to answer accurately. Never guess or make up numbers.
            """;
    }

    // ── Intent catalog for chat-initiated actions ──────────────────

    /// <summary>
    /// Builds a dynamic intent catalog from all available intents for injection
    /// into the cloud system prompt, enabling chat-initiated action execution.
    /// </summary>
    public static string? BuildIntentCatalog(IReadOnlyList<PrivStack.Sdk.Capabilities.IntentDescriptor> intents)
    {
        if (intents.Count == 0) return null;

        var sb = new System.Text.StringBuilder();
        sb.AppendLine("""
            CRITICAL — ACTION EXECUTION RULES:
            You have the ability to perform real actions in the user's workspace using [ACTION] blocks.
            When the user asks you to create, add, log, draft, or do something actionable, you MUST include an [ACTION] block.
            WITHOUT an [ACTION] block, NOTHING happens — the action is NOT performed. NEVER say "I've created" or "Done" unless you include the [ACTION] block.
            If the user asks for something you have no action for, say you can't do that yet. Do NOT pretend you did it.

            Format — place this at the END of your response, after your conversational message:
            [ACTION]
            {"intent_id": "tasks.create_task", "slots": {"title": "Review the budget", "priority": "high"}}
            [/ACTION]

            You may include multiple [ACTION] blocks if the user asks for multiple things.

            Available actions:
            """);

        foreach (var intent in intents)
        {
            var slotList = string.Join(", ", intent.Slots.Select(s =>
                s.Required ? $"{s.Name}*" : s.Name));
            sb.AppendLine($"- {intent.IntentId}: {intent.DisplayName} (slots: {slotList})");
        }

        sb.AppendLine();
        sb.AppendLine("Only use [ACTION] blocks when the user explicitly asks you to create/do something. For questions, just answer normally.");
        sb.AppendLine("Slots marked with * are required. You may omit optional slots if not mentioned.");
        sb.AppendLine("If an action is not in the list above, do NOT fabricate it. Tell the user it's not available.");

        return sb.ToString().TrimEnd();
    }

    // ── Response sanitization ────────────────────────────────────────

    // Matches LLM chat tokens like <|im_end|>, <|end|>, <|endoftext|>, etc.
    // Also captures Ċ (U+010A) or similar control chars that Qwen/other models prepend to stop tokens.
    [GeneratedRegex(@"[\u010A\u0120]?<\|?/?(system|user|assistant|end|im_start|im_end|eot_id|start_header_id|end_header_id|begin_of_text|end_of_text|endoftext|s)\|?>", RegexOptions.IgnoreCase)]
    private static partial Regex ChatTokenPattern();

    // Catches any remaining Ċ (U+010A) or Ġ (U+0120) characters that appear standalone at end of output
    [GeneratedRegex(@"[\u010A\u0120]+\s*$")]
    private static partial Regex TrailingModelArtifacts();

    [GeneratedRegex(@"^\s*-?\s*(User|Assistant|Duncan|System|Note)\s*:.*$", RegexOptions.IgnoreCase | RegexOptions.Multiline)]
    private static partial Regex RolePrefixLinePattern();

    [GeneratedRegex(@"\*{1,2}([^*]+)\*{1,2}")]
    private static partial Regex MarkdownBoldPattern();

    [GeneratedRegex(@"^[\s\-\*•]+", RegexOptions.Multiline)]
    private static partial Regex BulletPrefixPattern();

    [GeneratedRegex(@"^#{1,6}\s+", RegexOptions.Multiline)]
    private static partial Regex MarkdownHeaderPattern();

    [GeneratedRegex(@"\n{3,}")]
    private static partial Regex ExcessiveNewlines();

    /// <summary>
    /// Strips chat tokens, markdown, role prefixes, self-referential lines,
    /// and enforces sentence count limits per tier.
    /// </summary>
    public static string Sanitize(string response, ResponseTier tier = ResponseTier.Medium)
    {
        if (string.IsNullOrEmpty(response)) return response;

        // Strip raw chat tokens and model-specific artifacts (Ċ, Ġ before stop tokens)
        var cleaned = ChatTokenPattern().Replace(response, "");
        cleaned = TrailingModelArtifacts().Replace(cleaned, "");

        // Strip entire lines that are role prefixes or "**Note:**" disclaimers
        cleaned = RolePrefixLinePattern().Replace(cleaned, "");

        // Strip markdown bold/italic
        cleaned = MarkdownBoldPattern().Replace(cleaned, "$1");

        // Strip markdown headers
        cleaned = MarkdownHeaderPattern().Replace(cleaned, "");

        // Strip bullet prefixes (convert to plain sentences)
        cleaned = BulletPrefixPattern().Replace(cleaned, "");

        // Collapse excessive newlines
        cleaned = ExcessiveNewlines().Replace(cleaned, "\n\n");

        cleaned = cleaned.Trim();

        // Hard sentence-count cap per tier
        cleaned = TruncateToSentences(cleaned, MaxSentences(tier));

        return cleaned;
    }

    /// <summary>
    /// Truncates text to a maximum number of sentences.
    /// </summary>
    private static string TruncateToSentences(string text, int maxSentences)
    {
        if (maxSentences <= 0 || string.IsNullOrEmpty(text)) return text;

        var count = 0;
        for (var i = 0; i < text.Length; i++)
        {
            var c = text[i];
            if (c is '.' or '!' or '?')
            {
                // Skip consecutive punctuation (e.g., "..." or "?!")
                while (i + 1 < text.Length && text[i + 1] is '.' or '!' or '?')
                    i++;

                count++;
                if (count >= maxSentences)
                    return text[..(i + 1)].Trim();
            }
        }

        return text;
    }
}
