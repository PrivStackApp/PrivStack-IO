using Avalonia.Controls;
using Avalonia.Interactivity;

namespace PrivStack.Desktop.Views.Dialogs;

public partial class PermissionPromptWindow : Window
{
    public bool Allowed { get; private set; }
    public bool RememberChoice { get; private set; }

    public PermissionPromptWindow()
    {
        InitializeComponent();
    }

    public void SetContent(string pluginName, string capability, string description)
    {
        PluginNameText.Text = pluginName;
        CapabilityText.Text = capability;
        DescriptionText.Text = description;
    }

    private void OnAllow(object? sender, RoutedEventArgs e)
    {
        Allowed = true;
        RememberChoice = RememberCheckbox.IsChecked == true;
        Close(true);
    }

    private void OnDeny(object? sender, RoutedEventArgs e)
    {
        Allowed = false;
        RememberChoice = RememberCheckbox.IsChecked == true;
        Close(false);
    }
}
