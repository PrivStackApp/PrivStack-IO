using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.AI;
using PrivStack.Services.Plugin;
using PrivStack.Sdk;
using PrivStack.Sdk.Capabilities;
using PrivStack.Sdk.Services;
using PrivStack.Services.Connections;
using IIntentEngine = PrivStack.Sdk.Services.IIntentEngine;

namespace PrivStack.Services.Sdk;

/// <summary>
/// Creates IPluginHost instances for plugins. Used by PluginRegistry during
/// plugin initialization for SDK-based plugins.
/// </summary>
internal sealed class PluginHostFactory
{
    private readonly SdkHost _sdkHost;
    private readonly CapabilityBroker _capabilityBroker = new();
    private readonly ISdkDialogService _dialogService;
    private readonly IAppSettingsService _appSettings;
    private readonly IPluginRegistry _pluginRegistry;
    private readonly IUiDispatcher _dispatcher;
    private readonly InfoPanelService _infoPanelService;
    private readonly IFocusModeService _focusModeService;
    private readonly IConnectionService _connectionService;
    private readonly IPropertyService _propertyService;
    private readonly IToastService _toastService;
    private readonly IAiService _aiService;
    private readonly IIntentEngine _intentEngine;
    private readonly IAiSuggestionService _suggestionService;
    private readonly IAudioRecorderService _audioRecorder;
    private readonly ITranscriptionService _transcription;

    public ICapabilityBroker CapabilityBroker => _capabilityBroker;

    public PluginHostFactory()
    {
        _sdkHost = ServiceProviderAccessor.Services.GetRequiredService<SdkHost>();
        _appSettings = ServiceProviderAccessor.Services.GetRequiredService<IAppSettingsService>();
        _pluginRegistry = ServiceProviderAccessor.Services.GetRequiredService<IPluginRegistry>();
        _dialogService = ServiceProviderAccessor.Services.GetRequiredService<ISdkDialogService>();
        _dispatcher = ServiceProviderAccessor.Services.GetRequiredService<IUiDispatcher>();
        _infoPanelService = ServiceProviderAccessor.Services.GetRequiredService<InfoPanelService>();
        _focusModeService = ServiceProviderAccessor.Services.GetRequiredService<IFocusModeService>();
        _connectionService = ServiceProviderAccessor.Services.GetRequiredService<IConnectionService>();
        _propertyService = ServiceProviderAccessor.Services.GetRequiredService<EntityMetadataService>();
        _toastService = ServiceProviderAccessor.Services.GetRequiredService<IToastService>();
        _aiService = ServiceProviderAccessor.Services.GetRequiredService<IAiService>();
        _intentEngine = ServiceProviderAccessor.Services.GetRequiredService<IIntentEngine>();
        _suggestionService = ServiceProviderAccessor.Services.GetRequiredService<IAiSuggestionService>();
        _audioRecorder = ServiceProviderAccessor.Services.GetRequiredService<IAudioRecorderService>();
        _transcription = ServiceProviderAccessor.Services.GetRequiredService<ITranscriptionService>();

        // Register the default local filesystem storage provider
        _capabilityBroker.Register<IStorageProvider>(new LocalStorageProvider());

        // Register dataset service for cross-plugin access
        var datasetService = ServiceProviderAccessor.Services.GetRequiredService<IDatasetService>();
        _capabilityBroker.Register<IDatasetService>(datasetService);

        // Register shell-level RAG content provider (global features, shortcuts, intents, etc.)
        _capabilityBroker.Register<IIndexableContentProvider>(new ShellContentProvider(_intentEngine, _pluginRegistry));
    }

    public IPluginHost CreateHost(string pluginId)
    {
        return new PluginHost(_sdkHost, _capabilityBroker, pluginId, _dialogService, _appSettings, _pluginRegistry, _dispatcher, _infoPanelService, _focusModeService, _toastService, _connectionService, _propertyService, _aiService, _intentEngine, _suggestionService, _audioRecorder, _transcription);
    }
}
