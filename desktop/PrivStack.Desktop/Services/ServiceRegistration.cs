using System.Runtime.InteropServices;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Sdk;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.AI;
using PrivStack.Desktop.Services.Biometric;
using PrivStack.Desktop.Services.Connections;
using PrivStack.Desktop.Services.FileSync;
using PrivStack.Desktop.Services.Headless;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Desktop.Services.Api;
using PrivStack.Desktop.Services.Ipc;
using PrivStack.Desktop.Services.Update;
using PrivStack.Desktop.ViewModels;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using IAiService = PrivStack.Sdk.Services.IAiService;
using IIntentEngine = PrivStack.Sdk.Services.IIntentEngine;
using IToastService = PrivStack.Sdk.IToastService;

namespace PrivStack.Desktop.Services;

/// <summary>
/// Configures the DI container with all services and ViewModels.
/// </summary>
public static class ServiceRegistration
{
    public static IServiceProvider Configure()
    {
        var services = new ServiceCollection();
        RegisterCoreServices(services);

        // UI-specific services
        services.AddSingleton<IThemeService, ThemeService>();
        services.AddSingleton<IFontScaleService, FontScaleService>();
        services.AddSingleton<IResponsiveLayoutService, ResponsiveLayoutService>();
        services.AddSingleton<DialogService>();
        services.AddSingleton<IDialogService>(sp => sp.GetRequiredService<DialogService>());
        services.AddSingleton<IUiDispatcher, AvaloniaUiDispatcher>();

        services.AddSingleton<ToastService>();
        services.AddSingleton<IToastService>(sp => sp.GetRequiredService<ToastService>());

        services.AddSingleton<FocusModeService>();
        services.AddSingleton<IFocusModeService>(sp => sp.GetRequiredService<FocusModeService>());

        services.AddSingleton<ISystemNotificationService, SystemNotificationService>();

        // Services without interfaces (used directly)
        services.AddSingleton<CustomThemeStore>();
        services.AddSingleton<ViewStatePrefetchService>();
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
        WireCorePostBuild(provider);

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

    public static IServiceProvider ConfigureHeadless()
    {
        var services = new ServiceCollection();
        RegisterCoreServices(services);

        // Headless stubs — no-op implementations for UI services
        services.AddSingleton<IThemeService, HeadlessThemeService>();
        services.AddSingleton<IFontScaleService, HeadlessFontScaleService>();
        services.AddSingleton<IResponsiveLayoutService, HeadlessResponsiveLayoutService>();
        services.AddSingleton<IDialogService, HeadlessDialogService>();
        services.AddSingleton<IUiDispatcher, HeadlessUiDispatcher>();
        services.AddSingleton<IToastService, HeadlessToastService>();
        services.AddSingleton<IFocusModeService, HeadlessFocusModeService>();
        services.AddSingleton<ISystemNotificationService, HeadlessSystemNotificationService>();

        var provider = services.BuildServiceProvider();
        WireCorePostBuild(provider);

        // In headless mode, vault unlock uses the cached master password instead of a dialog
        var sdkHost = provider.GetRequiredService<SdkHost>();
        var passwordCache = provider.GetRequiredService<IMasterPasswordCache>();
        sdkHost.SetVaultUnlockPrompt((vaultId, ct) =>
        {
            var cached = passwordCache.Get();
            return Task.FromResult(cached);
        });

        return provider;
    }

    private static void RegisterCoreServices(IServiceCollection services)
    {
        // Core services (singletons — same lifetime as previous .Instance pattern)
        services.AddSingleton<IAppSettingsService, AppSettingsService>();
        services.AddSingleton<PrivStackService>();
        services.AddSingleton<IPrivStackNative>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<IPrivStackRuntime>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<IAuthService>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<ISyncService>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<IPairingService>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<ICloudStorageService>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<ILicensingService>(sp => sp.GetRequiredService<PrivStackService>());
        services.AddSingleton<ICloudSyncService, CloudSyncService>();
        services.AddSingleton<IWorkspaceService, WorkspaceService>();
        services.AddSingleton<IBackupService, BackupService>();
        services.AddSingleton<ISensitiveLockService, SensitiveLockService>();
        services.AddSingleton<IMasterPasswordCache, MasterPasswordCache>();

        // Biometric authentication — platform-conditional
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
            services.AddSingleton<IBiometricService, MacBiometricService>();
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            services.AddSingleton<IBiometricService, WindowsBiometricService>();
        else
            services.AddSingleton<IBiometricService, NullBiometricService>();

        services.AddSingleton<ISyncIngestionService, SyncIngestionService>();
        services.AddSingleton<IPluginRegistry, PluginRegistry>();

        services.AddSingleton<SdkHost>();
        services.AddSingleton<IPrivStackSdk>(sp => sp.GetRequiredService<SdkHost>());
        services.AddSingleton<ISyncOutboundService, SyncOutboundService>();
        services.AddSingleton<IFileEventSyncService, FileEventSyncService>();
        services.AddSingleton<ISnapshotSyncService, SnapshotSyncService>();
        services.AddSingleton<SeedDataService>();
        services.AddSingleton<InfoPanelService>();
        services.AddSingleton<BacklinkService>();
        services.AddSingleton<EntityMetadataService>();

        services.AddSingleton<LicenseExpirationService>();
        services.AddSingleton<SubscriptionValidationService>();
        services.AddSingleton<ReminderSchedulerService>();
        services.AddSingleton<PrivStackApiClient>();
        services.AddSingleton<OAuthLoginService>();
        services.AddSingleton<IPluginInstallService, PluginInstallService>();
        services.AddSingleton<IUpdateService, RegistryUpdateService>();

        // External connections (GitHub, Google, Microsoft)
        services.AddSingleton<GitHubDeviceFlowService>();
        services.AddSingleton<OAuthBrowserFlowService>();
        services.AddSingleton<ConnectionService>();
        services.AddSingleton<IConnectionService>(sp => sp.GetRequiredService<ConnectionService>());

        services.AddSingleton<WhisperService>();
        services.AddSingleton<WhisperModelManager>();
        services.AddSingleton<AiModelManager>();
        services.AddSingleton<AiService>();
        services.AddSingleton<IAiService>(sp => sp.GetRequiredService<AiService>());
        services.AddSingleton<AiMemoryService>();
        services.AddSingleton<AiMemoryExtractor>();
        services.AddSingleton<AiConversationStore>();
        services.AddSingleton<IntentEngine>();
        services.AddSingleton<IIntentEngine>(sp => sp.GetRequiredService<IntentEngine>());
        services.AddSingleton<IAiSuggestionService, AiSuggestionServiceImpl>();
        services.AddSingleton<LinkProviderCacheService>();
        services.AddSingleton<IDatasetService, DatasetService>();
        services.AddSingleton<INavigationService, Sdk.NavigationServiceAdapter>();
        services.AddSingleton<AI.DatasetInsightOrchestrator>();

        // RAG pipeline (embedding + indexing + search)
        services.AddSingleton<EmbeddingModelManager>();
        services.AddSingleton<EmbeddingService>();
        services.AddSingleton<RagIndexService>();
        services.AddSingleton<RagSearchService>();

        // Local HTTP API server
        services.AddSingleton<LocalApiServer>();
        services.AddSingleton<ILocalApiServer>(sp => sp.GetRequiredService<LocalApiServer>());
    }

    private static void WireCorePostBuild(IServiceProvider provider)
    {
        // Wire SyncOutbound into SdkHost (cross-singleton dependency resolved after build)
        var sdkHost = provider.GetRequiredService<SdkHost>();
        sdkHost.SetSyncOutbound(provider.GetRequiredService<ISyncOutboundService>());

        // Wire file-based event sync into outbound service
        if (provider.GetRequiredService<ISyncOutboundService>() is SyncOutboundService outbound)
            outbound.SetFileEventSync(provider.GetRequiredService<IFileEventSyncService>());

        // Wire license read-only detection from SdkHost into the expiration service
        var expirationService = provider.GetRequiredService<LicenseExpirationService>();
        sdkHost.LicenseReadOnlyBlocked += (_, _) => expirationService.OnMutationBlocked();
    }
}
