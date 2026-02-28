using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Sdk;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Desktop.ViewModels;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Ipc;
using PrivStack.Services.Plugin;
using PrivStack.Services.Sdk;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using IToastService = PrivStack.Sdk.IToastService;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Configures the DI container with core services (from PrivStack.Services)
/// plus Desktop-specific UI services and ViewModels.
/// </summary>
public static class ServiceRegistration
{
    public static IServiceProvider Configure()
    {
        var services = new ServiceCollection();

        // Register core (non-UI) services from the shared Services library
        CoreServiceRegistration.RegisterCoreServices(services);

        // Override AppSettingsService with Desktop subclass that has Window operations
        services.AddSingleton<DesktopAppSettingsService>();
        services.AddSingleton<IAppSettingsService>(sp => sp.GetRequiredService<DesktopAppSettingsService>());
        services.AddSingleton<IWindowSettingsService>(sp => sp.GetRequiredService<DesktopAppSettingsService>());

        // Desktop-specific: backup, plugin registry, whisper, audio
        services.AddSingleton<IBackupService, BackupService>();
        services.AddSingleton<IPluginRegistry, PluginRegistry>();

        // Desktop-specific: Whisper speech-to-text + audio recording
        services.AddSingleton<WhisperService>();
        services.AddSingleton<WhisperModelManager>();
        services.AddSingleton<IAudioRecorderService, AudioRecorderServiceAdapter>();
        services.AddSingleton<ITranscriptionService, TranscriptionServiceAdapter>();

        // UI-specific services
        services.AddSingleton<IThemeService, ThemeService>();
        services.AddSingleton<IFontScaleService, FontScaleService>();
        services.AddSingleton<IResponsiveLayoutService, ResponsiveLayoutService>();

        services.AddSingleton<DialogService>();
        services.AddSingleton<PrivStack.Services.Abstractions.IDialogService>(sp => sp.GetRequiredService<DialogService>());
        services.AddSingleton<ISdkDialogService>(sp =>
            new SdkDialogServiceAdapter(sp.GetRequiredService<DialogService>()));

        services.AddSingleton<IUiDispatcher, AvaloniaUiDispatcher>();

        services.AddSingleton<ToastService>();
        services.AddSingleton<IToastService>(sp => sp.GetRequiredService<ToastService>());

        services.AddSingleton<FocusModeService>();
        services.AddSingleton<IFocusModeService>(sp => sp.GetRequiredService<FocusModeService>());

        services.AddSingleton<ISystemNotificationService, SystemNotificationService>();

        // Services without interfaces (used directly)
        services.AddSingleton<CustomThemeStore>();
        services.AddSingleton<QuickActionService>();

        // IPC server for browser extension bridge
        services.AddSingleton<IpcMessageRouter>();
        services.AddSingleton<IpcServer>();
        services.AddSingleton<IIpcServer>(sp => sp.GetRequiredService<IpcServer>());

        // ViewModels (transient — created fresh each resolution)
        services.AddTransient<MainWindowViewModel>();
        services.AddTransient<SetupWizardViewModel>();
        services.AddTransient<SettingsViewModel>();
        services.AddTransient<UpdateViewModel>();

        var provider = services.BuildServiceProvider();
        CoreServiceRegistration.WireCorePostBuild(provider);

        // Wire vault unlock prompt — plugins call RequestVaultUnlockAsync to trigger this
        var dialogService = provider.GetRequiredService<DialogService>();
        var sdkHost = provider.GetRequiredService<SdkHost>();
        sdkHost.SetVaultUnlockPrompt(async (vaultId, ct) =>
        {
            return await Avalonia.Threading.Dispatcher.UIThread.InvokeAsync(async () =>
            {
                string? pluginName = null;
                string? pluginIcon = null;

                var pluginRegistry = provider.GetRequiredService<IPluginRegistry>();
                var vaultConsumers = pluginRegistry.GetCapabilityProviders<IVaultConsumer>();
                foreach (var consumer in vaultConsumers)
                {
                    if (consumer.VaultIds.Contains(vaultId) && consumer is IAppPlugin plugin)
                    {
                        pluginName = plugin.Metadata.Name;
                        pluginIcon = plugin.Metadata.Icon;
                        break;
                    }
                }

                return await dialogService.ShowVaultUnlockAsync(pluginName, pluginIcon);
            });
        });

        return provider;
    }
}
