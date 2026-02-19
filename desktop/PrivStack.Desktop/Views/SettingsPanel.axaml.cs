using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Views;

public partial class SettingsPanel : UserControl
{
    public SettingsPanel()
    {
        InitializeComponent();
    }

    protected override void OnLoaded(RoutedEventArgs e)
    {
        base.OnLoaded(e);

        var categoryList = this.FindControl<ListBox>("CategoryList");
        if (categoryList != null)
        {
            categoryList.SelectedIndex = 0;
            categoryList.SelectionChanged += OnCategoryListSelectionChanged;
        }
    }

    protected override void OnUnloaded(RoutedEventArgs e)
    {
        var categoryList = this.FindControl<ListBox>("CategoryList");
        if (categoryList != null)
            categoryList.SelectionChanged -= OnCategoryListSelectionChanged;

        base.OnUnloaded(e);
    }

    private void OnCategoryListSelectionChanged(object? sender, SelectionChangedEventArgs e)
    {
        var tabs = this.FindControl<Carousel>("SettingsTabs");
        if (sender is ListBox list && tabs != null && list.SelectedIndex >= 0)
            tabs.SelectedIndex = list.SelectedIndex;
    }

    private async void OnChooseProfileImageClick(object? sender, RoutedEventArgs e)
    {
        var topLevel = TopLevel.GetTopLevel(this);
        if (topLevel == null) return;

        var files = await topLevel.StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Choose Profile Image",
            AllowMultiple = false,
            FileTypeFilter =
            [
                new FilePickerFileType("Images") { Patterns = ["*.png", "*.jpg", "*.jpeg", "*.bmp", "*.webp"] }
            ]
        });

        if (files.Count > 0 && DataContext is SettingsViewModel vm)
        {
            var path = files[0].Path.LocalPath;
            vm.SetProfileImage(path);
        }
    }
}
