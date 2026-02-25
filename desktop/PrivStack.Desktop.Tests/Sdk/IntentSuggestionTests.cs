namespace PrivStack.Desktop.Tests.Sdk;

using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Messaging;
using PrivStack.Sdk.Services;

public class IntentSuggestionTests
{
    [Fact]
    public void IntentSuggestion_construction()
    {
        var signal = new IntentSignalMessage
        {
            SourcePluginId = "privstack.notes",
            SignalType = IntentSignalType.TextContent,
            Content = "Schedule a meeting with John tomorrow at 3pm"
        };

        var suggestion = new IntentSuggestion
        {
            SuggestionId = "sug-1",
            MatchedIntent = new IntentDescriptor
            {
                IntentId = "calendar.create_event",
                DisplayName = "Create Event",
                Description = "Creates a calendar event",
                PluginId = "privstack.calendar"
            },
            Summary = "Create event: Meeting with John",
            Confidence = 0.92,
            SourceSignal = signal,
            ExtractedSlots = new Dictionary<string, string>
            {
                ["title"] = "Meeting with John",
                ["start_date"] = "tomorrow",
                ["start_time"] = "3pm"
            }
        };

        suggestion.SuggestionId.Should().Be("sug-1");
        suggestion.Confidence.Should().BeApproximately(0.92, 0.001);
        suggestion.ExtractedSlots.Should().HaveCount(3);
        suggestion.MatchedIntent.IntentId.Should().Be("calendar.create_event");
    }

    [Fact]
    public void IntentSuggestion_CreatedAt_defaults_to_now()
    {
        var suggestion = new IntentSuggestion
        {
            SuggestionId = "sug-1",
            MatchedIntent = new IntentDescriptor
            {
                IntentId = "test",
                DisplayName = "Test",
                Description = "Test",
                PluginId = "test"
            },
            Summary = "Test",
            Confidence = 0.5,
            SourceSignal = new IntentSignalMessage
            {
                SourcePluginId = "test",
                SignalType = IntentSignalType.TextContent,
                Content = "test"
            },
            ExtractedSlots = new Dictionary<string, string>()
        };

        suggestion.CreatedAt.Should().BeCloseTo(DateTimeOffset.UtcNow, TimeSpan.FromSeconds(5));
    }

    [Fact]
    public void IntentSuggestion_empty_slots()
    {
        var suggestion = new IntentSuggestion
        {
            SuggestionId = "sug-1",
            MatchedIntent = new IntentDescriptor
            {
                IntentId = "test",
                DisplayName = "Test",
                Description = "Test",
                PluginId = "test"
            },
            Summary = "Test",
            Confidence = 1.0,
            SourceSignal = new IntentSignalMessage
            {
                SourcePluginId = "test",
                SignalType = IntentSignalType.UserRequest,
                Content = "do something"
            },
            ExtractedSlots = new Dictionary<string, string>()
        };

        suggestion.ExtractedSlots.Should().BeEmpty();
        suggestion.Confidence.Should().Be(1.0);
    }
}
