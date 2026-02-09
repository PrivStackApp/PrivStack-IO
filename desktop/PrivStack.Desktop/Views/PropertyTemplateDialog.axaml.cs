using Avalonia.Controls;
using Avalonia.Input;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class PropertyTemplateDialog : UserControl
{
    public PropertyTemplateDialog()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        WireTemplateListClicks();
        WireNewTemplateNameBox();
        WireTemplateNameBox();
    }

    /// <summary>
    /// Wire template list buttons to set SelectedTemplate via code-behind
    /// since ItemsControl buttons inside DataTemplates can't easily bind a click
    /// that sets the parent's SelectedTemplate.
    /// </summary>
    private void WireTemplateListClicks()
    {
        var templateList = this.FindControl<ItemsControl>("TemplateList");
        if (templateList != null)
        {
            templateList.Tapped -= OnTemplateItemTapped;
            templateList.Tapped += OnTemplateItemTapped;
        }
    }

    private void OnTemplateItemTapped(object? sender, TappedEventArgs e)
    {
        if (DataContext is not PropertyTemplateDialogViewModel vm) return;

        var source = e.Source as Control;
        while (source != null)
        {
            if (source is Button btn && btn.DataContext is Models.PropertyTemplate template)
            {
                // Don't select when the delete button was clicked
                if (btn.Command == vm.DeleteTemplateCommand) return;
                vm.SelectedTemplate = template;
                return;
            }
            source = source.Parent as Control;
        }
    }

    private void WireNewTemplateNameBox()
    {
        var box = this.FindControl<TextBox>("NewTemplateNameBox");
        if (box == null) return;
        box.KeyDown -= OnNewTemplateNameKeyDown;
        box.KeyDown += OnNewTemplateNameKeyDown;
    }

    private void OnNewTemplateNameKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key != Key.Enter) return;
        if (DataContext is not PropertyTemplateDialogViewModel vm) return;

        vm.CreateTemplateCommand.Execute(null);
        e.Handled = true;
    }

    private void WireTemplateNameBox()
    {
        var box = this.FindControl<TextBox>("TemplateNameBox");
        if (box == null) return;
        box.LostFocus -= OnTemplateNameBoxLostFocus;
        box.LostFocus += OnTemplateNameBoxLostFocus;
        box.KeyDown -= OnTemplateNameBoxKeyDown;
        box.KeyDown += OnTemplateNameBoxKeyDown;
    }

    private void OnTemplateNameBoxLostFocus(object? sender, Avalonia.Interactivity.RoutedEventArgs e)
    {
        if (DataContext is PropertyTemplateDialogViewModel vm)
            vm.SaveTemplateNameCommand.Execute(null);
    }

    private void OnTemplateNameBoxKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key != Key.Enter) return;
        if (DataContext is not PropertyTemplateDialogViewModel vm) return;

        vm.SaveTemplateNameCommand.Execute(null);
        e.Handled = true;
    }
}
