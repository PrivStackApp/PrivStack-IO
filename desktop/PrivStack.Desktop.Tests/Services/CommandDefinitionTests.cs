namespace PrivStack.Desktop.Tests.Services;

using PrivStack.Desktop.Services;

public class CommandDefinitionTests
{
    [Fact]
    public void CommandDefinition_required_fields()
    {
        var executed = false;
        var cmd = new CommandDefinition
        {
            Name = "New Page",
            Description = "Creates a new notes page",
            Execute = () => executed = true
        };
        cmd.Name.Should().Be("New Page");
        cmd.Description.Should().Be("Creates a new notes page");

        cmd.Execute();
        executed.Should().BeTrue();
    }

    [Fact]
    public void CommandDefinition_defaults()
    {
        var cmd = new CommandDefinition
        {
            Name = "Test",
            Description = "Test command",
            Execute = () => { }
        };
        cmd.Keywords.Should().BeEmpty();
        cmd.Category.Should().Be("General");
        cmd.Icon.Should().BeNull();
    }

    [Fact]
    public void CommandDefinition_with_all_fields()
    {
        var cmd = new CommandDefinition
        {
            Name = "Import CSV",
            Description = "Import a CSV file",
            Keywords = "import csv data upload",
            Category = "Files",
            Icon = "Upload",
            Execute = () => { }
        };
        cmd.Keywords.Should().Contain("csv");
        cmd.Category.Should().Be("Files");
        cmd.Icon.Should().Be("Upload");
    }
}
