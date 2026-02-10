// ============================================================================
// File: WasmPluginProxy.cs
// Description: IAppPlugin wrapper around FFI calls to the Rust plugin host.
//              Makes Wasm plugins appear identical to native .NET plugins.
// ============================================================================

using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;
using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Services;
using PrivStack.Sdk;
using PrivStack.UI.Adaptive;
using Serilog;
using NativeLib = PrivStack.Desktop.Native.NativeLibrary;

namespace PrivStack.Desktop.Services.Plugin;

/// <summary>
/// Proxy that wraps a Wasm plugin loaded in the Rust plugin host,
/// presenting the standard <see cref="IAppPlugin"/> interface to the .NET app.
/// The PluginRegistry treats this identically to a native .dll plugin.
/// </summary>
internal sealed class WasmPluginProxy : ObservableObject, IAppPlugin
{
    private static readonly ILogger _log = Log.ForContext<WasmPluginProxy>();
    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    private readonly string _pluginId;
    private readonly string? _templateJson;
    private PluginState _state = PluginState.Discovered;
    private WasmViewModelProxy? _viewModel;
    private bool _disposed;

    /// <summary>
    /// Command palettes loaded from the plugin's command_palettes.json sidecar.
    /// </summary>
    public List<PluginPaletteDefinition>? CommandPalettes { get; init; }

    public WasmPluginProxy(WasmPluginMetadataDto metadata, List<WasmEntitySchemaDto>? schemas, string? templateJson = null)
    {
        _pluginId = metadata.Id;
        _templateJson = templateJson;

        Metadata = new PluginMetadata
        {
            Id = metadata.Id,
            Name = metadata.Name,
            Description = metadata.Description,
            Version = Version.TryParse(metadata.Version, out var v) ? v : new Version(0, 1, 0),
            Author = metadata.Author,
            Icon = metadata.Icon,
            NavigationOrder = (int)metadata.NavigationOrder,
            Category = MapCategory(metadata.Category),
            CanDisable = metadata.CanDisable,
            IsExperimental = metadata.IsExperimental,
        };

        NavigationItem = new NavigationItem
        {
            Id = metadata.Id,
            DisplayName = metadata.Name,
            Icon = metadata.Icon,
            Order = (int)metadata.NavigationOrder,
        };

        // Convert schemas
        if (schemas is { Count: > 0 })
        {
            EntitySchemas = schemas.Select(s => new EntitySchema
            {
                EntityType = s.EntityType,
                IndexedFields = s.IndexedFields.Select(f => new IndexedField
                {
                    FieldPath = f.FieldPath,
                    FieldType = MapFieldType(f.FieldType),
                    Searchable = f.Searchable,
                    Dimensions = f.VectorDim.HasValue ? (int)f.VectorDim.Value : null,
                    Options = f.EnumOptions,
                }).ToList(),
                MergeStrategy = MapMergeStrategy(s.MergeStrategy),
            }).ToList();
        }
        else
        {
            EntitySchemas = [];
        }
    }

    public PluginMetadata Metadata { get; }
    public NavigationItem? NavigationItem { get; }
    public PrivStack.Sdk.ICommandProvider? CommandProvider => null; // Commands come via FFI
    public IReadOnlyList<EntitySchema> EntitySchemas { get; }

    public PluginState State
    {
        get => _state;
        private set => SetProperty(ref _state, value);
    }

    public Task<bool> InitializeAsync(IPluginHost host, CancellationToken cancellationToken = default)
    {
        State = PluginState.Initialized;
        _log.Information("Wasm plugin initialized: {PluginId}", _pluginId);
        return Task.FromResult(true);
    }

    public void Activate()
    {
        State = PluginState.Active;
        var result = NativeLib.PluginActivate(_pluginId);
        if (result != Native.PrivStackError.Ok)
            _log.Warning("Wasm plugin activate() call failed for {PluginId}: {Error}", _pluginId, result);
        _log.Debug("Wasm plugin activated: {PluginId}", _pluginId);
    }

    public void Deactivate()
    {
        State = PluginState.Deactivated;
        _log.Debug("Wasm plugin deactivated: {PluginId}", _pluginId);
    }

    public ViewModelBase CreateViewModel()
    {
        if (_viewModel is null)
        {
            _viewModel = new WasmViewModelProxy(_pluginId);
            if (_templateJson is not null)
            {
                _viewModel.SetTemplate(_templateJson);
            }
        }
        return _viewModel;
    }

    public void ResetViewModel()
    {
        _viewModel = null;
    }

    public Task OnNavigatedToAsync(CancellationToken cancellationToken = default)
    {
        // Notify the Wasm plugin that it's now visible
        NativeLib.PluginNavigatedTo(_pluginId);

        // Send local timezone info so plugin can display dates correctly
        var now = DateTimeOffset.UtcNow;
        var today = DateTime.Today.ToString("yyyy-MM-dd");
        var offsetMinutes = (int)TimeZoneInfo.Local.GetUtcOffset(DateTime.UtcNow).TotalMinutes;
        var tzArgs = JsonSerializer.Serialize(new { today, offset = offsetMinutes });
        var tzPtr = NativeLib.PluginSendCommand(_pluginId, "set_timezone", tzArgs);
        if (tzPtr != nint.Zero) NativeLib.FreeString(tzPtr);

        _viewModel?.RefreshViewState();
        return Task.CompletedTask;
    }

    public void OnNavigatedFrom()
    {
        NativeLib.PluginNavigatedFrom(_pluginId);
    }

    /// <summary>
    /// Sends an SDK message to this plugin via FFI and returns the response.
    /// </summary>
    public WasmSdkResponse? SendSdkMessage(WasmSdkMessage message)
    {
        var messageJson = JsonSerializer.Serialize(message, _jsonOptions);
        var resultPtr = NativeLib.PluginRouteSdk(_pluginId, messageJson);
        if (resultPtr == nint.Zero) return null;

        try
        {
            var json = Marshal.PtrToStringUTF8(resultPtr);
            if (string.IsNullOrEmpty(json)) return null;
            return JsonSerializer.Deserialize<WasmSdkResponse>(json, _jsonOptions);
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        State = PluginState.Disposed;

        // Unload from the Rust side
        var result = NativeLib.PluginUnload(_pluginId);
        if (result != Native.PrivStackError.Ok)
        {
            _log.Warning("Failed to unload Wasm plugin {PluginId}: {Error}", _pluginId, result);
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    private static PluginCategory MapCategory(string? category) => category?.ToLowerInvariant() switch
    {
        "productivity" => PluginCategory.Productivity,
        "security" => PluginCategory.Security,
        "communication" => PluginCategory.Communication,
        "information" => PluginCategory.Information,
        "utility" => PluginCategory.Utility,
        "extension" => PluginCategory.Extension,
        _ => PluginCategory.Utility,
    };

    private static FieldType MapFieldType(string? fieldType) => fieldType?.ToLowerInvariant() switch
    {
        "text" => FieldType.Text,
        "tag" => FieldType.Tag,
        "date_time" or "datetime" => FieldType.DateTime,
        "number" => FieldType.Number,
        "bool" or "boolean" => FieldType.Bool,
        "vector" => FieldType.Vector,
        "decimal" => FieldType.Decimal,
        "relation" => FieldType.Relation,
        "counter" => FieldType.Counter,
        "json" => FieldType.Json,
        "enum" or "enumeration" => FieldType.Enum,
        "geo_point" => FieldType.GeoPoint,
        "duration" => FieldType.Duration,
        _ => FieldType.Text,
    };

    private static MergeStrategy MapMergeStrategy(string? strategy) => strategy?.ToLowerInvariant() switch
    {
        "lww_document" => MergeStrategy.LwwDocument,
        "lww_per_field" => MergeStrategy.LwwPerField,
        "custom" => MergeStrategy.Custom,
        _ => MergeStrategy.LwwPerField,
    };
}

/// <summary>
/// ViewModel proxy for Wasm plugins. Holds JSON view state from the plugin
/// and notifies the UI when state changes.
/// </summary>
internal sealed class WasmViewModelProxy : ViewModelBase
{
    private static readonly ILogger _log = Log.ForContext<WasmViewModelProxy>();
    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    };

    private readonly string _pluginId;
    private TemplateEngine? _templateEngine;
    private string? _viewStateJson;
    private bool _usePartialRefresh;
    private string? _currentEntityId;

    /// <summary>
    /// Fired before ViewStateJson is about to change.
    /// Listeners can use this to prepare for partial refresh.
    /// Args: (usePartialRefresh)
    /// </summary>
    public event Action<bool>? ViewStateChanging;

    public WasmViewModelProxy(string pluginId)
    {
        _pluginId = pluginId;
    }

    /// <summary>
    /// Sets the template for host-side rendering. When set, RefreshViewState()
    /// calls get_view_data() instead of get_view_state() and evaluates the template.
    /// </summary>
    public void SetTemplate(string templateJson)
    {
        try
        {
            _templateEngine = new TemplateEngine(templateJson);
            _log.Information("Template engine initialized for plugin {PluginId}", _pluginId);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Failed to parse template for plugin {PluginId}, falling back to legacy path", _pluginId);
            _templateEngine = null;
        }
    }

    /// <summary>
    /// Plugin ID for command routing.
    /// </summary>
    public string PluginId => _pluginId;

    /// <summary>
    /// The currently displayed entity ID (e.g., page ID for notes plugin).
    /// Updated when navigating to a specific entity.
    /// </summary>
    public string? CurrentEntityId => _currentEntityId;

    /// <summary>
    /// Raw JSON view state from the Wasm plugin. The generic renderer
    /// binds to this and creates UI from the declarative JSON.
    /// </summary>
    public string? ViewStateJson
    {
        get => _viewStateJson;
        private set
        {
            // Fire event before change so listeners can prepare (e.g., request partial refresh)
            ViewStateChanging?.Invoke(_usePartialRefresh);
            _usePartialRefresh = false; // Reset flag

            // Force property change even if value is the same (e.g., re-navigating to same entity)
            // SetProperty won't raise PropertyChanged if value equals, but we need to re-render
            var forceNotify = _viewStateJson == value && value != null;
            SetProperty(ref _viewStateJson, value);
            if (forceNotify)
            {
                _log.Debug("ViewStateJson: Forcing PropertyChanged (same value, len={Len})", value?.Length ?? 0);
                OnPropertyChanged(nameof(ViewStateJson));
            }
        }
    }

    /// <summary>
    /// Returns the current rendered view state JSON without setting it on the property.
    /// Used by the renderer's split view to refresh block data without a full re-render.
    /// </summary>
    /// <summary>
    /// Returns the raw plugin view data JSON (before template rendering).
    /// Used by the renderer for reading plugin state like selected_folder_id.
    /// </summary>
    public string? GetRawViewData()
    {
        var dataPtr = NativeLib.PluginGetViewData(_pluginId);
        if (dataPtr == nint.Zero) return null;
        try
        {
            return Marshal.PtrToStringUTF8(dataPtr);
        }
        finally
        {
            NativeLib.FreeString(dataPtr);
        }
    }

    public string? GetRenderedViewState()
    {
        if (_templateEngine is null) return ViewStateJson;

        var dataPtr = NativeLib.PluginGetViewData(_pluginId);
        if (dataPtr == nint.Zero) return ViewStateJson;

        try
        {
            var dataJson = Marshal.PtrToStringUTF8(dataPtr);
            if (string.IsNullOrEmpty(dataJson) || dataJson == "{}") return ViewStateJson;

            var componentTree = _templateEngine.Evaluate(dataJson);
            return $@"{{""components"":{componentTree}}}";
        }
        catch
        {
            return ViewStateJson;
        }
        finally
        {
            NativeLib.FreeString(dataPtr);
        }
    }

    /// <summary>
    /// Refreshes the view state by querying the plugin's get_view_state() export via FFI.
    /// Checks the prefetch cache first to skip FFI call if data was preloaded on hover.
    /// </summary>
    /// <param name="forPluginId">Optional: plugin ID for entity-specific cache lookup.</param>
    /// <param name="forEntityId">Optional: entity ID for entity-specific cache lookup.</param>
    public void RefreshViewState(string? forPluginId = null, string? forEntityId = null)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        _log.Debug("RefreshViewState: [T+{T}ms] START plugin={Plugin}, forPlugin={ForPlugin}, forEntity={ForEntity}",
            sw.ElapsedMilliseconds, _pluginId, forPluginId ?? "(null)", forEntityId ?? "(null)");

        // Track the current entity ID for cache invalidation on navigation
        if (forEntityId != null)
            _currentEntityId = forEntityId;

        // Detect same-plugin navigation for partial refresh optimization
        var isSamePlugin = forPluginId == null || forPluginId == _pluginId;
        var isEntityNavigation = forEntityId != null;
        _usePartialRefresh = isSamePlugin && isEntityNavigation && _viewStateJson != null;
        _log.Debug("RefreshViewState: [T+{T}ms] isSamePlugin={Same}, isEntityNav={EntityNav}, usePartialRefresh={Partial}, currentEntity={CurrentEntity}",
            sw.ElapsedMilliseconds, isSamePlugin, isEntityNavigation, _usePartialRefresh, _currentEntityId ?? "(null)");

        // Check prefetch cache for preloaded data
        string? cachedViewData = null;
        try
        {
            var prefetchService = App.Services?.GetService<ViewStatePrefetchService>();

            // Try entity-specific cache first if provided
            if (forPluginId != null && forEntityId != null)
            {
                var cached = prefetchService?.TryGetCached(forPluginId, forEntityId);
                if (cached != null)
                {
                    cachedViewData = cached.ViewStateJson;
                    _log.Debug("RefreshViewState: [T+{T}ms] Entity cache HIT, dataLen={Len}",
                        sw.ElapsedMilliseconds, cachedViewData?.Length ?? 0);
                }
            }

            // Fall back to plugin root cache
            if (cachedViewData == null)
            {
                var cached = prefetchService?.TryGetCached(_pluginId);
                if (cached != null)
                {
                    cachedViewData = cached.ViewStateJson;
                    _log.Debug("RefreshViewState: [T+{T}ms] Plugin root cache HIT, dataLen={Len}",
                        sw.ElapsedMilliseconds, cachedViewData?.Length ?? 0);
                }
            }

            if (cachedViewData == null)
            {
                _log.Debug("RefreshViewState: [T+{T}ms] Cache MISS - will fetch from FFI", sw.ElapsedMilliseconds);
            }
        }
        catch (Exception ex)
        {
            _log.Warning(ex, "RefreshViewState cache lookup failed");
        }

        if (_templateEngine is not null)
        {
            _log.Debug("RefreshViewState: [T+{T}ms] Using template engine", sw.ElapsedMilliseconds);
            RefreshViaTemplate(cachedViewData, sw);
        }
        else
        {
            _log.Debug("RefreshViewState: [T+{T}ms] Using legacy refresh", sw.ElapsedMilliseconds);
            RefreshLegacy();
        }

        _log.Debug("RefreshViewState: [T+{T}ms] END", sw.ElapsedMilliseconds);
    }

    /// <summary>
    /// Refreshes the view state using pre-fetched cached data.
    /// Skips cache lookup and FFI call for fastest possible render.
    /// </summary>
    public void RefreshViewStateFromCache(string cachedViewData, string forPluginId, string forEntityId)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        _log.Debug("RefreshViewStateFromCache: [T+{T}ms] START plugin={Plugin}, forEntity={ForEntity}, dataLen={Len}",
            sw.ElapsedMilliseconds, forPluginId, forEntityId, cachedViewData.Length);

        // Track the current entity ID
        _currentEntityId = forEntityId;

        // Enable partial refresh for same-plugin navigation
        var isSamePlugin = forPluginId == _pluginId;
        _usePartialRefresh = isSamePlugin && _viewStateJson != null;
        _log.Debug("RefreshViewStateFromCache: [T+{T}ms] isSamePlugin={Same}, usePartialRefresh={Partial}",
            sw.ElapsedMilliseconds, isSamePlugin, _usePartialRefresh);

        if (_templateEngine is not null)
        {
            RefreshViaTemplate(cachedViewData, sw);
        }
        else
        {
            // Fallback: for legacy plugins without templates, we'd need to call FFI
            // This shouldn't happen for WASM plugins which all use templates
            _log.Warning("RefreshViewStateFromCache: No template engine, falling back to legacy");
            RefreshLegacy();
        }

        _log.Debug("RefreshViewStateFromCache: [T+{T}ms] END", sw.ElapsedMilliseconds);
    }

    private void RefreshViaTemplate(string? cachedData = null, System.Diagnostics.Stopwatch? sw = null)
    {
        sw ??= System.Diagnostics.Stopwatch.StartNew();
        string? dataJson;
        nint dataPtr = nint.Zero;

        if (cachedData != null)
        {
            // Use cached data - skip FFI call
            dataJson = cachedData;
            _log.Debug("RefreshViaTemplate: [T+{T}ms] Using cached data, len={Len}", sw.ElapsedMilliseconds, dataJson.Length);
        }
        else
        {
            // Fetch fresh data via FFI
            _log.Debug("RefreshViaTemplate: [T+{T}ms] Calling FFI PluginGetViewData", sw.ElapsedMilliseconds);
            dataPtr = NativeLib.PluginGetViewData(_pluginId);
            _log.Debug("RefreshViaTemplate: [T+{T}ms] FFI returned ptr={Ptr}", sw.ElapsedMilliseconds, dataPtr);

            if (dataPtr == nint.Zero)
            {
                _log.Debug("RefreshViaTemplate: [T+{T}ms] GetViewData returned null, falling back to legacy", sw.ElapsedMilliseconds);
                RefreshLegacy();
                return;
            }
            dataJson = Marshal.PtrToStringUTF8(dataPtr);
            _log.Debug("RefreshViaTemplate: [T+{T}ms] FFI data marshaled, len={Len}", sw.ElapsedMilliseconds, dataJson?.Length ?? 0);
        }

        try
        {
            if (string.IsNullOrEmpty(dataJson) || dataJson == "{}")
            {
                _log.Debug("RefreshViaTemplate: [T+{T}ms] Empty data, falling back to legacy", sw.ElapsedMilliseconds);
                RefreshLegacy();
                return;
            }

            _log.Debug("RefreshViaTemplate: [T+{T}ms] Evaluating template...", sw.ElapsedMilliseconds);
            var componentTree = _templateEngine!.Evaluate(dataJson);
            _log.Debug("RefreshViaTemplate: [T+{T}ms] Template evaluated, result len={Len}", sw.ElapsedMilliseconds, componentTree?.Length ?? 0);

            var newJson = $@"{{""components"":{componentTree}}}";
            _log.Debug("RefreshViaTemplate: [T+{T}ms] Setting ViewStateJson, len={Len}", sw.ElapsedMilliseconds, newJson.Length);
            ViewStateJson = newJson;
            _log.Debug("RefreshViaTemplate: [T+{T}ms] ViewStateJson set complete", sw.ElapsedMilliseconds);
        }
        catch (Exception ex)
        {
            _log.Error(ex, "Template evaluation failed for {PluginId}, falling back to legacy", _pluginId);
            RefreshLegacy();
        }
        finally
        {
            if (dataPtr != nint.Zero)
                NativeLib.FreeString(dataPtr);
        }
    }

    private void RefreshLegacy()
    {
        var resultPtr = NativeLib.PluginGetViewState(_pluginId);
        if (resultPtr == nint.Zero) return;

        try
        {
            var json = Marshal.PtrToStringUTF8(resultPtr);
            ViewStateJson = json;
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }

    /// <summary>
    /// Sends a named command to the plugin's handle_command() export.
    /// Returns the JSON result string.
    /// </summary>
    public string? SendCommand(string commandName, string argsJson = "{}")
    {
        var resultPtr = NativeLib.PluginSendCommand(_pluginId, commandName, argsJson);
        if (resultPtr == nint.Zero) return null;

        try
        {
            return Marshal.PtrToStringUTF8(resultPtr);
        }
        finally
        {
            NativeLib.FreeString(resultPtr);
        }
    }
}

// ============================================================================
// DTOs for JSON deserialization from the Rust side
// ============================================================================

internal sealed record WasmPluginMetadataDto
{
    [JsonPropertyName("id")]
    public required string Id { get; init; }

    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("description")]
    public string Description { get; init; } = "";

    [JsonPropertyName("version")]
    public string Version { get; init; } = "0.1.0";

    [JsonPropertyName("author")]
    public string Author { get; init; } = "Unknown";

    [JsonPropertyName("icon")]
    public string? Icon { get; init; }

    [JsonPropertyName("navigation_order")]
    public uint NavigationOrder { get; init; } = 1000;

    [JsonPropertyName("category")]
    public string? Category { get; init; }

    [JsonPropertyName("can_disable")]
    public bool CanDisable { get; init; } = true;

    [JsonPropertyName("is_experimental")]
    public bool IsExperimental { get; init; }

    [JsonPropertyName("capabilities")]
    public List<string>? Capabilities { get; init; }
}

internal sealed record WasmEntitySchemaDto
{
    [JsonPropertyName("entity_type")]
    public required string EntityType { get; init; }

    [JsonPropertyName("indexed_fields")]
    public List<WasmIndexedFieldDto> IndexedFields { get; init; } = [];

    [JsonPropertyName("merge_strategy")]
    public string? MergeStrategy { get; init; }
}

internal sealed record WasmIndexedFieldDto
{
    [JsonPropertyName("field_path")]
    public required string FieldPath { get; init; }

    [JsonPropertyName("field_type")]
    public string? FieldType { get; init; }

    [JsonPropertyName("searchable")]
    public bool Searchable { get; init; }

    [JsonPropertyName("vector_dim")]
    public uint? VectorDim { get; init; }

    [JsonPropertyName("enum_options")]
    public IReadOnlyList<string>? EnumOptions { get; init; }
}

internal sealed record WasmSdkMessage
{
    [JsonPropertyName("action")]
    public required string Action { get; init; }

    [JsonPropertyName("entity_type")]
    public string EntityType { get; init; } = "";

    [JsonPropertyName("entity_id")]
    public string? EntityId { get; init; }

    [JsonPropertyName("payload")]
    public string? Payload { get; init; }

    [JsonPropertyName("parameters")]
    public List<WasmSdkParameter> Parameters { get; init; } = [];

    [JsonPropertyName("source")]
    public string? Source { get; init; }
}

internal sealed record WasmSdkParameter
{
    [JsonPropertyName("key")]
    public required string Key { get; init; }

    [JsonPropertyName("value")]
    public required string Value { get; init; }
}

internal sealed record WasmSdkResponse
{
    [JsonPropertyName("success")]
    public bool Success { get; init; }

    [JsonPropertyName("data")]
    public string? Data { get; init; }

    [JsonPropertyName("error")]
    public string? Error { get; init; }

    [JsonPropertyName("error_code")]
    public int? ErrorCode { get; init; }
}
