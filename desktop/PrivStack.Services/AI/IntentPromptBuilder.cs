using System.Text;
using PrivStack.Sdk.Capabilities;

namespace PrivStack.Services.AI;

/// <summary>
/// Model capability tier for intent classification prompt selection.
/// </summary>
internal enum ModelTier
{
    /// <summary>Local models &lt;5B params — terse few-shot, limited intent set.</summary>
    Small,
    /// <summary>Local models 7B+ — richer instructions, all intents, slot descriptions.</summary>
    Medium,
    /// <summary>Cloud models — full detailed prompt, chain-of-thought, edge cases.</summary>
    Large,
}

/// <summary>
/// Builds the AI system/user prompts for intent classification.
/// Supports 3 tiers: Small (terse few-shot), Medium (richer local), Large (full cloud).
/// </summary>
internal static class IntentPromptBuilder
{
    /// <summary>Backward-compatible overload — defaults to Small tier.</summary>
    public static string BuildSystemPrompt(IReadOnlyList<IntentDescriptor> intents, DateTimeOffset now) =>
        BuildSystemPrompt(intents, now, ModelTier.Small);

    public static string BuildSystemPrompt(
        IReadOnlyList<IntentDescriptor> intents, DateTimeOffset now, ModelTier tier) => tier switch
    {
        ModelTier.Large => BuildLargePrompt(intents, now),
        ModelTier.Medium => BuildMediumPrompt(intents, now),
        _ => BuildSmallPrompt(intents, now),
    };

    // ── Small Tier (current behavior, unchanged) ────────────────────

    private static string BuildSmallPrompt(IReadOnlyList<IntentDescriptor> intents, DateTimeOffset now)
    {
        var sb = new StringBuilder(2048);

        sb.AppendLine("Extract ACTIONS from text. Most text has 0 or 1 actions. Be selective.");
        sb.AppendLine("calendar.create_event = a CONFIRMED meeting/event at a specific time.");
        sb.AppendLine("tasks.create_task = something to DO (call, buy, send, fix, book, schedule).");
        sb.AppendLine("Generate a short clear title. Fill description with details from the text. Fill all applicable slots.");
        sb.AppendLine("When a source entity is provided, use its title to generate a contextually accurate event/task title instead of generic labels.");
        sb.AppendLine();

        sb.AppendLine("Action IDs and slots:");
        foreach (var intent in intents)
        {
            var allSlots = intent.Slots.Select(s =>
                s.Required ? s.Name : $"{s.Name}?");
            sb.AppendLine($"- {intent.IntentId} [{string.Join(", ", allSlots)}]");
        }

        sb.AppendLine();
        sb.AppendLine($"Today: {now:yyyy-MM-dd dddd}");
        sb.AppendLine();

        AppendSmallExamples(sb, now);

        sb.Append("VALID intent_id: ");
        sb.AppendLine(string.Join(", ", intents.Select(i => i.IntentId)));
        sb.AppendLine("Output JSON only.");

        return sb.ToString();
    }

    private static void AppendSmallExamples(StringBuilder sb, DateTimeOffset now)
    {
        sb.AppendLine("--- Example 1 ---");
        sb.AppendLine("Text: \"Team standup meeting every Monday at 9am in the conference room\"");
        AppendExample(sb, "calendar.create_event", 0.9,
            "Team standup Monday 9am",
            ("title", "Team Standup"), ("start_time", FutureDay(now, DayOfWeek.Monday, 9)),
            ("location", "Conference room"), ("description", "Weekly team standup meeting"));

        sb.AppendLine("--- Example 2 ---");
        sb.AppendLine("Text: \"Call the dentist to book a cleaning appointment next week\"");
        AppendExample(sb, "tasks.create_task", 0.9,
            "Call dentist to book cleaning",
            ("title", "Call dentist for cleaning appointment"),
            ("description", "Call the dentist office to schedule a cleaning appointment for next week"),
            ("priority", "medium"));

        sb.AppendLine("--- Example 3 ---");
        sb.AppendLine("Text: \"Had a great day at the park. The sunset was beautiful.\"");
        sb.AppendLine("{\"intents\":[]}");
        sb.AppendLine();

        sb.AppendLine("--- Example 4 ---");
        sb.AppendLine("Text: \"Send the quarterly report to Sarah by Friday, it's urgent\"");
        AppendExample(sb, "tasks.create_task", 0.9,
            "Send quarterly report to Sarah by Friday",
            ("title", "Send quarterly report to Sarah"),
            ("description", "Send the quarterly report to Sarah before end of day Friday"),
            ("due_date", FutureDayDate(now, DayOfWeek.Friday)),
            ("priority", "high"));
    }

    // ── Medium Tier (local 7B+ models) ──────────────────────────────

    private static string BuildMediumPrompt(IReadOnlyList<IntentDescriptor> intents, DateTimeOffset now)
    {
        var sb = new StringBuilder(4096);

        sb.AppendLine("You are an intent classification system. Analyze text and extract actionable intents.");
        sb.AppendLine("Most text contains 0 or 1 intents. Only extract an intent when there is a clear, actionable request.");
        sb.AppendLine("Do NOT extract intents from casual observations, opinions, or general statements.");
        sb.AppendLine();
        sb.AppendLine("Rules:");
        sb.AppendLine("- Generate a short, specific title for each intent.");
        sb.AppendLine("- Fill the description slot with relevant details from the source text.");
        sb.AppendLine("- Fill all applicable slots with values extracted from the text.");
        sb.AppendLine("- Confidence: 0.9 = very clear intent, 0.7 = likely, 0.5 = uncertain.");
        sb.AppendLine("- When a source entity is provided, use its title to generate a contextually accurate title.");
        sb.AppendLine("- If the text contains no actionable intent, return {\"intents\":[]}.");
        sb.AppendLine();

        sb.AppendLine("Available intents and their slots:");
        foreach (var intent in intents)
        {
            sb.Append($"- {intent.IntentId}: {intent.DisplayName}");
            if (intent.Slots.Count > 0)
            {
                var slotDescs = intent.Slots.Select(s =>
                {
                    var req = s.Required ? "required" : "optional";
                    return $"{s.Name} ({s.Type}, {req})";
                });
                sb.Append($"  [{string.Join(", ", slotDescs)}]");
            }
            sb.AppendLine();
        }

        sb.AppendLine();
        sb.AppendLine($"Current date: {now:yyyy-MM-dd dddd}");
        sb.AppendLine("Use ISO 8601 format for dates/times (e.g. 2025-03-15T14:00:00).");
        sb.AppendLine();

        // 4 standard examples + 2 edge cases
        AppendSmallExamples(sb, now);

        sb.AppendLine("--- Example 5 ---");
        sb.AppendLine("Text: \"I really need to get better at exercising regularly\"");
        sb.AppendLine("{\"intents\":[]}");
        sb.AppendLine();

        sb.AppendLine("--- Example 6 ---");
        sb.AppendLine("Text: \"Draft an email to the team about the project deadline moving to next month\"");
        AppendExample(sb, "email.draft_email", 0.85,
            "Draft email about deadline change",
            ("subject", "Project Deadline Update"),
            ("body", "Draft email informing the team that the project deadline has been moved to next month"));

        sb.Append("VALID intent_id values: ");
        sb.AppendLine(string.Join(", ", intents.Select(i => i.IntentId)));
        sb.AppendLine("Output JSON only, no explanation.");

        return sb.ToString();
    }

    // ── Large Tier (cloud models) ───────────────────────────────────

    private static string BuildLargePrompt(IReadOnlyList<IntentDescriptor> intents, DateTimeOffset now)
    {
        var sb = new StringBuilder(8192);

        sb.AppendLine("You are an expert intent classification system for a personal productivity application.");
        sb.AppendLine("Your task is to analyze text content and extract actionable intents with high precision.");
        sb.AppendLine();
        sb.AppendLine("## Classification Guidelines");
        sb.AppendLine();
        sb.AppendLine("1. **Be selective**: Most text contains 0 or 1 intents. Only extract when there is a clear, actionable request.");
        sb.AppendLine("2. **No false positives**: Casual observations, opinions, stories, and general statements are NOT intents.");
        sb.AppendLine("3. **Contextual titles**: Generate concise, specific titles that capture the essence of the action.");
        sb.AppendLine("4. **Rich slot filling**: Extract all applicable slot values from the text. Use context clues for implicit values.");
        sb.AppendLine("5. **Source entity awareness**: When a source entity is provided, use its title to generate a contextually accurate intent title rather than generic labels.");
        sb.AppendLine();
        sb.AppendLine("## Confidence Scoring");
        sb.AppendLine("- **0.9–1.0**: Explicit, unambiguous request (\"Schedule a meeting with John at 3pm\")");
        sb.AppendLine("- **0.7–0.8**: Strong implication of action (\"I need to call the dentist about my appointment\")");
        sb.AppendLine("- **0.5–0.6**: Possible intent but ambiguous (\"The report is due Friday\" — could be informational)");
        sb.AppendLine("- **Below 0.5**: Do not include — too uncertain.");
        sb.AppendLine();
        sb.AppendLine("## Edge Cases");
        sb.AppendLine("- **Negation**: \"I decided NOT to schedule the meeting\" → no intent.");
        sb.AppendLine("- **Past tense**: \"I already called the dentist\" → no intent (already done).");
        sb.AppendLine("- **Hypothetical**: \"I might need to buy groceries\" → no intent (not committed).");
        sb.AppendLine("- **Multiple intents**: If text clearly contains 2+ distinct actions, extract each separately.");
        sb.AppendLine();

        sb.AppendLine("## Available Intents");
        sb.AppendLine();
        foreach (var intent in intents)
        {
            sb.AppendLine($"### `{intent.IntentId}` — {intent.DisplayName}");
            sb.AppendLine($"  {intent.Description}");
            if (intent.Slots.Count > 0)
            {
                sb.AppendLine("  Slots:");
                foreach (var slot in intent.Slots)
                {
                    var req = slot.Required ? "required" : "optional";
                    sb.AppendLine($"    - `{slot.Name}` ({slot.Type}, {req}): {slot.Description}");
                }
            }
            sb.AppendLine();
        }

        sb.AppendLine($"Current date: {now:yyyy-MM-dd dddd}");
        sb.AppendLine("Use ISO 8601 format for all dates/times (e.g. 2025-03-15T14:00:00).");
        sb.AppendLine("Resolve relative dates (\"next Tuesday\", \"tomorrow\", \"this Friday\") against the current date.");
        sb.AppendLine();

        sb.AppendLine("## Examples");
        sb.AppendLine();

        sb.AppendLine("--- Example 1 ---");
        sb.AppendLine("Text: \"Team standup meeting every Monday at 9am in the conference room\"");
        AppendExample(sb, "calendar.create_event", 0.9,
            "Team standup Monday 9am",
            ("title", "Team Standup"), ("start_time", FutureDay(now, DayOfWeek.Monday, 9)),
            ("location", "Conference room"), ("description", "Weekly team standup meeting"));

        sb.AppendLine("--- Example 2 ---");
        sb.AppendLine("Text: \"Call the dentist to book a cleaning appointment next week\"");
        AppendExample(sb, "tasks.create_task", 0.9,
            "Call dentist to book cleaning",
            ("title", "Call dentist for cleaning appointment"),
            ("description", "Call the dentist office to schedule a cleaning appointment for next week"),
            ("priority", "medium"));

        sb.AppendLine("--- Example 3 ---");
        sb.AppendLine("Text: \"Had a great day at the park. The sunset was beautiful.\"");
        sb.AppendLine("{\"intents\":[]}");
        sb.AppendLine();

        sb.AppendLine("--- Example 4 ---");
        sb.AppendLine("Text: \"Send the quarterly report to Sarah by Friday, it's urgent\"");
        AppendExample(sb, "tasks.create_task", 0.9,
            "Send quarterly report to Sarah by Friday",
            ("title", "Send quarterly report to Sarah"),
            ("description", "Send the quarterly report to Sarah before end of day Friday"),
            ("due_date", FutureDayDate(now, DayOfWeek.Friday)),
            ("priority", "high"));

        sb.AppendLine("--- Example 5 ---");
        sb.AppendLine("Text: \"I already finished the budget review and sent it to accounting\"");
        sb.AppendLine("{\"intents\":[]}");
        sb.AppendLine();

        sb.AppendLine("--- Example 6 ---");
        sb.AppendLine("Text: \"Lunch with Maria on Thursday at noon at the Italian place, and remind me to buy her birthday gift beforehand\"");
        AppendExample(sb, "calendar.create_event", 0.9,
            "Lunch with Maria Thursday noon",
            ("title", "Lunch with Maria"), ("start_time", FutureDay(now, DayOfWeek.Thursday, 12)),
            ("location", "Italian restaurant"), ("description", "Lunch meeting with Maria"));
        AppendExample(sb, "tasks.create_task", 0.8,
            "Buy birthday gift for Maria",
            ("title", "Buy birthday gift for Maria"),
            ("description", "Purchase a birthday gift for Maria before Thursday lunch"),
            ("due_date", FutureDayDate(now, DayOfWeek.Thursday)),
            ("priority", "medium"));

        sb.Append("VALID intent_id values: ");
        sb.AppendLine(string.Join(", ", intents.Select(i => i.IntentId)));
        sb.AppendLine();
        sb.AppendLine("Respond with a JSON object containing an \"intents\" array. Output JSON only, no explanation or commentary.");

        return sb.ToString();
    }

    // ── Shared ──────────────────────────────────────────────────────

    public static string BuildUserPrompt(string content, string? entityType, string? entityTitle)
    {
        var sb = new StringBuilder(content.Length + 128);
        if (!string.IsNullOrEmpty(entityType) && !string.IsNullOrEmpty(entityTitle))
            sb.AppendLine($"Source ({entityType}): \"{entityTitle}\"");
        sb.AppendLine("Text:");
        sb.Append(content.Length > 2000 ? content[..2000] + "..." : content);
        return sb.ToString();
    }

    private static void AppendExample(
        StringBuilder sb, string intentId, double confidence, string summary,
        params (string name, string value)[] slots)
    {
        var slotJson = string.Join(",", slots.Select(s => $"\"{s.name}\":\"{s.value}\""));
        sb.AppendLine($"{{\"intents\":[{{\"intent_id\":\"{intentId}\",\"confidence\":{confidence},\"summary\":\"{summary}\",\"slots\":{{{slotJson}}}}}]}}");
        sb.AppendLine();
    }

    private static string FutureDay(DateTimeOffset now, DayOfWeek target, int hour)
    {
        var daysAhead = ((int)target - (int)now.DayOfWeek + 7) % 7;
        if (daysAhead == 0) daysAhead = 7;
        var date = now.Date.AddDays(daysAhead).AddHours(hour);
        return date.ToString("yyyy-MM-ddTHH:mm:ss");
    }

    private static string FutureDayDate(DateTimeOffset now, DayOfWeek target)
    {
        var daysAhead = ((int)target - (int)now.DayOfWeek + 7) % 7;
        if (daysAhead == 0) daysAhead = 7;
        return now.Date.AddDays(daysAhead).ToString("yyyy-MM-dd");
    }
}
