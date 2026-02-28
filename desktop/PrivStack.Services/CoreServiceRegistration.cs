using System.Runtime.InteropServices;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services.Abstractions;
using PrivStack.Services.AI;
using PrivStack.Services.Api;
using PrivStack.Services.Biometric;
using PrivStack.Services.Connections;
using PrivStack.Services.FileSync;
using PrivStack.Services.Native;
using PrivStack.Services.Plugin;
using PrivStack.Services.Sdk;
using PrivStack.Services.Update;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using IAiService = PrivStack.Sdk.Services.IAiService;
using IIntentEngine = PrivStack.Sdk.Services.IIntentEngine;
using IToastService = PrivStack.Sdk.IToastService;

namespace PrivStack.Services;

/// <summary>
/// Registers core (non-UI) services shared by both Desktop and Server.
/// UI-specific services (Theme, FontScale, Dialog, Toast, etc.) are added by the caller.
/// </summary>
public static class CoreServiceRegistration
{
    public static void RegisterCoreServices(IServiceCollection services)
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
        // IBackupService registered by caller (Desktop: BackupService, Server: headless stub)
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
        // IPluginRegistry registered by caller (Desktop: PluginRegistry with Avalonia hooks, Server: headless version)

        services.AddSingleton<FfiSdkTransport>();
        services.AddSingleton<ISdkTransport>(sp => sp.GetRequiredService<FfiSdkTransport>());
        services.AddSingleton<SdkHost>();
        services.AddSingleton<IPrivStackSdk>(sp => sp.GetRequiredService<SdkHost>());
        services.AddSingleton<ISyncOutboundService, SyncOutboundService>();
        services.AddSingleton<IFileEventSyncService, FileEventSyncService>();
        services.AddSingleton<ISnapshotSyncService, SnapshotSyncService>();
        services.AddSingleton<SeedDataService>();
        services.AddSingleton<InfoPanelService>();
        services.AddSingleton<BacklinkService>();
        services.AddSingleton<EntityMetadataService>();
        services.AddSingleton<ViewStatePrefetchService>();

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
        services.AddSingleton<INavigationService, NavigationServiceAdapter>();
        services.AddSingleton<DatasetInsightOrchestrator>();

        // RAG pipeline (indexing + search — embedding implementation registered by caller)
        services.AddSingleton<EmbeddingModelManager>();
        services.AddSingleton<RagIndexService>();
        services.AddSingleton<RagSearchService>();

        // Local HTTP API server
        services.AddSingleton<LocalApiServer>();
        services.AddSingleton<ILocalApiServer>(sp => sp.GetRequiredService<LocalApiServer>());
    }

    public static void WireCorePostBuild(IServiceProvider provider)
    {
        // Wire SyncOutbound into SdkHost (cross-singleton dependency resolved after build)
        var sdkHost = provider.GetRequiredService<SdkHost>();
        sdkHost.SetSyncOutbound(provider.GetRequiredService<ISyncOutboundService>());

        // Wire file-based event sync into outbound service
        if (provider.GetRequiredService<ISyncOutboundService>() is SyncOutboundService outbound)
            outbound.SetFileEventSync(provider.GetRequiredService<IFileEventSyncService>());

        // Wire SDK transport into LocalApiServer for passthrough endpoints
        if (provider.GetRequiredService<ILocalApiServer>() is LocalApiServer apiServer)
            apiServer.SetSdkTransport(provider.GetRequiredService<ISdkTransport>());

        // Wire license read-only detection from SdkHost into the expiration service
        var expirationService = provider.GetRequiredService<LicenseExpirationService>();
        sdkHost.LicenseReadOnlyBlocked += (_, _) => expirationService.OnMutationBlocked();
    }
}
