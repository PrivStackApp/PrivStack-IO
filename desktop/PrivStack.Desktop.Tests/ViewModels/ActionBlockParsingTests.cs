using PrivStack.Desktop.ViewModels.AiTray;

namespace PrivStack.Desktop.Tests.ViewModels;

public class ActionBlockParsingTests
{
    [Fact]
    public void ParseActionBlocks_BareJson_SingleIntent_Recovered()
    {
        var input = """{"intent_id": "notes.create_note", "slots": {"title": "Hello", "content": "World"}}""";

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        actions.Should().HaveCount(1);
        actions[0].IntentId.Should().Be("notes.create_note");
        actions[0].Slots["title"].Should().Be("Hello");
        actions[0].Slots["content"].Should().Be("World");
        cleanText.Should().BeEmpty();
    }

    [Fact]
    public void ParseActionBlocks_BareJson_WithSurroundingProse()
    {
        var input = """
            Sure, I'll create that note for you.

            {"intent_id": "notes.create_note", "slots": {"title": "Meeting Notes"}}

            Let me know if you need anything else!
            """;

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        actions.Should().HaveCount(1);
        actions[0].IntentId.Should().Be("notes.create_note");
        cleanText.Should().Contain("create that note");
        cleanText.Should().Contain("Let me know");
        cleanText.Should().NotContain("intent_id");
    }

    [Fact]
    public void ParseActionBlocks_BareJson_IgnoredWhenActionTagsPresent()
    {
        var input = """Here's your note. [ACTION]{"intent_id": "notes.create_note", "slots": {"title": "Test"}}[/ACTION]""";

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        // Primary parser should find it — fallback should NOT double-extract
        actions.Should().HaveCount(1);
        actions[0].IntentId.Should().Be("notes.create_note");
    }

    [Fact]
    public void ParseActionBlocks_BareJson_IgnoresNonIntentJson()
    {
        var input = """Here is some data: {"key": "value", "count": 42}""";

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        actions.Should().BeEmpty();
        cleanText.Should().Contain("key");
    }

    [Fact]
    public void ParseActionBlocks_BareJson_MultipleBareIntents()
    {
        var input = """
            Creating both items.
            {"intent_id": "notes.create_note", "slots": {"title": "Note 1"}}
            {"intent_id": "tasks.create_task", "slots": {"title": "Task 1"}}
            """;

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        actions.Should().HaveCount(2);
        actions[0].IntentId.Should().Be("notes.create_note");
        actions[1].IntentId.Should().Be("tasks.create_task");
        cleanText.Should().Contain("Creating both items");
        cleanText.Should().NotContain("intent_id");
    }

    [Fact]
    public void ParseActionBlocks_BareJson_MalformedJson_Skipped()
    {
        var input = """Here: {"intent_id": "notes.create_note", "slots": {"title": "broken""";

        var (cleanText, actions) = AiSuggestionTrayViewModel.ParseActionBlocks(input);

        actions.Should().BeEmpty();
        // Should not crash — malformed JSON is gracefully skipped
    }
}
