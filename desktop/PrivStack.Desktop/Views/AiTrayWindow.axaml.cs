using Avalonia.Controls;

namespace PrivStack.Desktop.Views;

public partial class AiTrayWindow : Window
{
    public event EventHandler? WindowClosingByUser;

    public AiTrayWindow()
    {
        InitializeComponent();
    }

    protected override void OnClosing(WindowClosingEventArgs e)
    {
        // Notify the main window to reattach before the window actually closes
        WindowClosingByUser?.Invoke(this, EventArgs.Empty);
        base.OnClosing(e);
    }
}
