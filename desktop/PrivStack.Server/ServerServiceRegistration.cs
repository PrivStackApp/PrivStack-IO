using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Plugin;
using PrivStack.Services.Sdk;
using PrivStack.Sdk;
using PrivStack.Sdk.Services;
using IToastService = PrivStack.Sdk.IToastService;

namespace PrivStack.Server;

/// <summary>
/// DI configuration for the headless server. Registers core services plus headless stubs.
/// </summary>
internal static class ServerServiceRegistration
{
    public static IServiceProvider Configure()
    {
        var services = new ServiceCollection();
        CoreServiceRegistration.RegisterCoreServices(services);

        // Headless stubs — no-op implementations for UI services
        services.AddSingleton<IFontScaleService, HeadlessFontScaleService>();
        services.AddSingleton<IResponsiveLayoutService, HeadlessResponsiveLayoutService>();
        services.AddSingleton<IDialogService, HeadlessDialogService>();
        services.AddSingleton<ISdkDialogService, HeadlessSdkDialogService>();
        services.AddSingleton<IUiDispatcher, HeadlessUiDispatcher>();
        services.AddSingleton<IToastService, HeadlessToastService>();
        services.AddSingleton<IFocusModeService, HeadlessFocusModeService>();
        services.AddSingleton<ISystemNotificationService, HeadlessSystemNotificationService>();
        services.AddSingleton<IAudioRecorderService, HeadlessAudioRecorderService>();
        services.AddSingleton<ITranscriptionService, HeadlessTranscriptionService>();
        services.AddSingleton<IBackupService, HeadlessBackupService>();
        services.AddSingleton<PrivStack.Services.AI.IEmbeddingService, HeadlessEmbeddingService>();

        // Plugin registry — headless version (no Avalonia lifecycle hooks)
        services.AddSingleton<IPluginRegistry, HeadlessPluginRegistry>();

        var provider = services.BuildServiceProvider();
        CoreServiceRegistration.WireCorePostBuild(provider);

        // In headless mode, vault unlock uses the cached master password
        var sdkHost = provider.GetRequiredService<SdkHost>();
        var passwordCache = provider.GetRequiredService<IMasterPasswordCache>();
        sdkHost.SetVaultUnlockPrompt((vaultId, ct) =>
        {
            var cached = passwordCache.Get();
            return Task.FromResult(cached);
        });

        return provider;
    }
}
