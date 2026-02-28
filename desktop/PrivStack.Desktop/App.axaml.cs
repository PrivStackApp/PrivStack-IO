using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Data.Core;
using Avalonia.Data.Core.Plugins;
using Avalonia.Threading;
using System.Linq;
using Avalonia.Markup.Xaml;
using AvaloniaWebView;
using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Native;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Services.Api;
using PrivStack.Services.FileSync;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Sdk.Capabilities;
using PrivStack.Desktop.ViewModels;
using PrivStack.Desktop.Views;
using PrivStack.Sdk;
using PrivStack.UI.Adaptive;

namespace PrivStack.Desktop;

public partial class App : Application
{
    /// <summary>
    /// DI container — available for the transition period while Views still need service locator.
    /// </summary>
    public static IServiceProvider Services { get; internal set; } = null!;

    /// <summary>
    /// True when Desktop is running in client mode (proxying SDK calls to a headless server).
    /// In client mode, DuckDB is not opened locally — the server owns the database.
    /// </summary>
    public static bool IsClientMode { get; private set; }

    /// <summary>
    /// License status received from the headless server during client mode detection.
    /// Null in standalone mode (license is checked locally via native service).
    /// </summary>
    private static string? _serverLicenseStatus;

    public override void RegisterServices()
    {
        base.RegisterServices();

        // Only initialize WebView on Windows - macOS has .NET 9 compatibility issues
        if (OperatingSystem.IsWindows())
        {
            AvaloniaWebViewBuilder.Initialize(default);
        }
    }

    public override void Initialize()
    {
        // Build the DI container before anything else
        Services = ServiceRegistration.Configure();
        PrivStack.Services.ServiceProviderAccessor.Services = Services;

        Log.Debug("Avalonia XAML loader starting");
        AvaloniaXamlLoader.Load(this);
        Log.Debug("Avalonia XAML loader completed");

        // Initialize theme service after XAML is loaded
        Log.Debug("Initializing theme service");
        Services.GetRequiredService<IThemeService>().Initialize();

        // Initialize font scale service after theme (font scale is reapplied after theme changes)
        Log.Debug("Initializing font scale service");
        Services.GetRequiredService<IFontScaleService>().Initialize();

        // Initialize responsive layout service after font scale
        Log.Debug("Initializing responsive layout service");
        Services.GetRequiredService<IResponsiveLayoutService>().Initialize();
    }

    public override void OnFrameworkInitializationCompleted()
    {
        Log.Information("Framework initialization completed");

        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // Avoid duplicate validations from both Avalonia and the CommunityToolkit.
            DisableAvaloniaDataAnnotationValidation();

            // Check if first-run setup is needed, or if workspaces are missing
            var workspaceService = Services.GetRequiredService<IWorkspaceService>();
            if (!SetupWizardViewModel.IsSetupComplete() || !workspaceService.HasWorkspaces)
            {
                // For first-run or missing workspaces, DON'T initialize service yet.
                // Setup wizard will initialize it after user picks data directory.
                Log.Information("Setup required (first-run or no workspaces), showing setup wizard");
                ShowSetupWizard(desktop);
            }
            else if (TryEnterClientMode())
            {
                // A headless server is already running — Desktop becomes a client.
                // Skip DuckDB init and unlock screen; route SDK calls over HTTP.
                Log.Information("Client mode active, loading application directly");
                _ = EnterClientModeAsync(desktop).ContinueWith(t =>
                {
                    if (t.IsFaulted)
                    {
                        var ex = t.Exception!.InnerException ?? t.Exception;
                        Log.Error(ex, "Client mode startup failed");
                        Console.Error.WriteLine($"[privstack] Client mode startup failed: {ex.GetType().Name}: {ex.Message}");
                    }
                }, TaskScheduler.Default);
            }
            else
            {
                // Normal standalone mode — initialize local DuckDB
                Log.Information("Initializing PrivStack native service");
                InitializeService();

                // Show unlock screen
                Log.Information("Setup complete, showing unlock screen");
                ShowUnlockScreen(desktop);
            }
        }

        base.OnFrameworkInitializationCompleted();
    }

    private void InitializeService()
    {
        try
        {
            var workspaceService = Services.GetRequiredService<IWorkspaceService>();
            var active = workspaceService.GetActiveWorkspace();

            if (active != null)
            {
                // Ensure DataPaths is workspace-aware before anything touches paths
                var resolvedDir = workspaceService.ResolveWorkspaceDir(active);
                DataPaths.SetActiveWorkspace(active.Id, resolvedDir);

                // Reconfigure logger to write to workspace-specific log directory
                Log.ReconfigureForWorkspace(active.Id);

                // Run one-time data migration for existing installs
                WorkspaceDataMigration.MigrateIfNeeded(active.Id, resolvedDir);
            }

            var dbPath = workspaceService.GetActiveDataPath();
            Log.Information("Using workspace database path: {DbPath}", dbPath);

            var dir = Path.GetDirectoryName(dbPath)!;
            Directory.CreateDirectory(dir);

            // Diagnostic logging for storage state before init
            LogStorageDiagnostics(dbPath);

            Services.GetRequiredService<IPrivStackRuntime>().Initialize(dbPath);
            Log.Information("Native service initialized successfully");

            // Clean up orphaned root-level DB files from pre-workspace-scoping
            CleanupOrphanedRootFiles();
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to initialize native service");
            Serilog.Log.CloseAndFlush();
        }
    }

    /// <summary>
    /// Checks if a headless server is already running by probing the status endpoint.
    /// If detected, swaps the SDK transport from FFI to HTTP and returns true.
    /// </summary>
    private bool TryEnterClientMode()
    {
        try
        {
            var appSettings = Services.GetRequiredService<IAppSettingsService>();
            var apiKey = appSettings.Settings.ApiKey;
            if (string.IsNullOrEmpty(apiKey))
                return false;

            // Determine server URL — check headless-config.json first, fall back to defaults
            var serverUrl = ResolveServerUrl(appSettings);
            Log.Debug("Probing for running headless server at {Url}", serverUrl);

            using var httpClient = new HttpClient { Timeout = TimeSpan.FromSeconds(1) };
            using var request = new HttpRequestMessage(HttpMethod.Get, $"{serverUrl}/api/v1/status");
            using var response = httpClient.Send(request, HttpCompletionOption.ResponseContentRead);

            if (!response.IsSuccessStatusCode)
                return false;

            using var stream = response.Content.ReadAsStream();
            using var reader = new StreamReader(stream);
            var body = reader.ReadToEnd();

            if (!body.Contains("\"status\":\"ok\""))
                return false;

            // Parse license status from server response
            try
            {
                using var doc = System.Text.Json.JsonDocument.Parse(body);
                if (doc.RootElement.TryGetProperty("license_status", out var lsProp))
                    _serverLicenseStatus = lsProp.GetString();
            }
            catch { /* Non-critical — leave as null */ }

            Log.Information("Detected running headless server at {Url}, switching to client mode (license={License})",
                serverUrl, _serverLicenseStatus ?? "unknown");

            // Swap the transport in SdkHost from FFI to HTTP
            var transport = new HttpSdkTransport(serverUrl, apiKey);
            var sdkHost = Services.GetRequiredService<SdkHost>();
            sdkHost.SetTransport(transport);

            // Set workspace paths even in client mode (needed for settings, logs, plugin paths)
            var workspaceService = Services.GetRequiredService<IWorkspaceService>();
            var active = workspaceService.GetActiveWorkspace();
            if (active != null)
            {
                var resolvedDir = workspaceService.ResolveWorkspaceDir(active);
                DataPaths.SetActiveWorkspace(active.Id, resolvedDir);
                Log.ReconfigureForWorkspace(active.Id);
            }

            IsClientMode = true;
            return true;
        }
        catch
        {
            Log.Debug("No running headless server detected (this is normal for standalone mode)");
            return false;
        }
    }

    /// <summary>
    /// Resolves the server URL from headless-config.json or falls back to defaults.
    /// </summary>
    private static string ResolveServerUrl(IAppSettingsService appSettings)
    {
        var configPath = Path.Combine(DataPaths.BaseDir, "headless-config.json");
        if (File.Exists(configPath))
        {
            try
            {
                var json = File.ReadAllText(configPath);
                using var doc = System.Text.Json.JsonDocument.Parse(json);
                var root = doc.RootElement;

                var bind = root.TryGetProperty("bind_address", out var bindProp)
                    ? bindProp.GetString() ?? "127.0.0.1"
                    : "127.0.0.1";
                var port = root.TryGetProperty("port", out var portProp)
                    ? portProp.GetInt32()
                    : appSettings.Settings.ApiPort;

                return $"http://{bind}:{port}";
            }
            catch
            {
                // Fall through to default
            }
        }

        return $"http://127.0.0.1:{appSettings.Settings.ApiPort}";
    }

    /// <summary>
    /// Client mode startup: skip unlock screen, discover plugins, show main window directly.
    /// Background services that duplicate the server's work are not started.
    /// </summary>
    private async Task EnterClientModeAsync(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var pluginRegistry = Services.GetRequiredService<IPluginRegistry>();
        await Task.Run(() => pluginRegistry.DiscoverAndInitialize());

        await ShowMainWindow(desktop, skipPluginInit: true, isClientMode: true);
    }

    /// <summary>
    /// Removes orphaned data.*.duckdb and data.peer_id files from the root BaseDir
    /// that were created by the old setup wizard initializing at root level.
    /// </summary>
    private static void CleanupOrphanedRootFiles()
    {
        try
        {
            var baseDir = DataPaths.BaseDir;
            var orphanPatterns = new[] { "data.*.duckdb", "data.*.duckdb.wal", "data.peer_id" };

            foreach (var pattern in orphanPatterns)
            {
                foreach (var file in Directory.GetFiles(baseDir, pattern))
                {
                    try
                    {
                        File.Delete(file);
                        Log.Information("Cleaned up orphaned root file: {File}", Path.GetFileName(file));
                    }
                    catch (Exception ex)
                    {
                        Log.Logger.Warning(ex, "Could not delete orphaned root file: {File}", file);
                    }
                }
            }
        }
        catch (Exception ex)
        {
            Log.Logger.Warning(ex, "Failed to clean up orphaned root files");
        }
    }

    /// <summary>
    /// Logs detailed diagnostics about DuckDB file state before initialization.
    /// </summary>
    private static void LogStorageDiagnostics(string dbPath)
    {
        try
        {
            var basePath = Path.ChangeExtension(dbPath, null); // strip .duckdb
            var dbDir = Path.GetDirectoryName(dbPath)!;

            string[] suffixes = ["vault.duckdb", "blobs.duckdb", "entities.duckdb", "events.duckdb"];

            Log.Information("[StorageDiag] Base path: {BasePath}", basePath);
            Log.Information("[StorageDiag] Directory exists: {Exists}, writable: {Writable}",
                Directory.Exists(dbDir),
                IsDirectoryWritable(dbDir));

            foreach (var suffix in suffixes)
            {
                var filePath = $"{basePath}.{suffix}";
                var walPath = $"{filePath}.wal";

                if (File.Exists(filePath))
                {
                    var info = new FileInfo(filePath);
                    Log.Information("[StorageDiag] {File}: size={Size}, modified={Modified}",
                        Path.GetFileName(filePath),
                        info.Length,
                        info.LastWriteTimeUtc.ToString("o"));
                    if (info.IsReadOnly)
                        Log.Warning("[StorageDiag] {File}: IS READ-ONLY!", Path.GetFileName(filePath));
                }
                else
                {
                    Log.Warning("[StorageDiag] {File}: DOES NOT EXIST", Path.GetFileName(filePath));
                }

                if (File.Exists(walPath))
                {
                    var walInfo = new FileInfo(walPath);
                    Log.Warning("[StorageDiag] {WalFile}: WAL EXISTS! size={Size}",
                        Path.GetFileName(walPath),
                        walInfo.Length);
                }
            }

            // Check peer_id
            var peerIdPath = $"{basePath}.peer_id";
            if (File.Exists(peerIdPath))
            {
                var peerId = File.ReadAllText(peerIdPath).Trim();
                Log.Information("[StorageDiag] peer_id: {PeerId}", peerId);
            }
            else
            {
                Log.Warning("[StorageDiag] peer_id file DOES NOT EXIST");
            }

            // List all files in directory for completeness
            var allFiles = Directory.GetFiles(dbDir);
            Log.Information("[StorageDiag] Directory contains {Count} files: {Files}",
                allFiles.Length,
                string.Join(", ", allFiles.Select(Path.GetFileName)));
        }
        catch (Exception ex)
        {
            Log.Error(ex, "[StorageDiag] Failed to collect storage diagnostics");
        }
    }

    private static bool IsDirectoryWritable(string path)
    {
        try
        {
            var testFile = Path.Combine(path, $".privstack_write_test_{Guid.NewGuid():N}");
            File.WriteAllText(testFile, "test");
            File.Delete(testFile);
            return true;
        }
        catch
        {
            return false;
        }
    }

    private void ShowSetupWizard(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var setupVm = Services.GetRequiredService<SetupWizardViewModel>();
        var setupWindow = new SetupWindow(setupVm);

        setupVm.SetupCompleted += async (_, _) =>
        {
            // Show loading state on the Complete step while app initializes
            setupVm.IsAppLoading = true;
            setupVm.LoadingMessage = "Loading plugins...";

            await Task.Delay(50);

            var pluginRegistry = Services.GetRequiredService<IPluginRegistry>();
            await Task.Run(() => pluginRegistry.DiscoverAndInitialize());

            setupVm.LoadingMessage = "Starting up...";
            await Task.Delay(30);

            // IMPORTANT: Set the new MainWindow BEFORE closing setup window
            // Otherwise Avalonia shuts down when it sees MainWindow closed
            await ShowMainWindow(desktop, skipPluginInit: true,
                updateStatus: msg => setupVm.LoadingMessage = msg,
                onBeforeTransition: () =>
                {
                    setupVm.IsAppLoading = false;
                    setupVm.LoadingMessage = "Launching...";
                    // SetupWizardViewModel doesn't have shimmer, just hide the loading state
                });
            setupWindow.Close();
        };

        desktop.MainWindow = setupWindow;
    }

    private void ShowUnlockScreen(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var unlockVm = new UnlockViewModel(
            Services.GetRequiredService<IAuthService>(),
            Services.GetRequiredService<IPrivStackRuntime>(),
            Services.GetRequiredService<IWorkspaceService>(),
            Services.GetRequiredService<IMasterPasswordCache>(),
            Services.GetRequiredService<IBiometricService>(),
            Services.GetRequiredService<IAppSettingsService>());
        var unlockWindow = new UnlockWindow(unlockVm);

        unlockVm.AppUnlocked += async (_, _) =>
        {
            Log.Information("App unlocked, loading application...");
            unlockVm.IsAppLoading = true;
            unlockVm.LoadingMessage = "Loading plugins...";

            // Yield to let the UI update before heavy work
            await Task.Delay(50);

            // Plugin discovery and heavy init on background thread
            var pluginRegistry = Services.GetRequiredService<IPluginRegistry>();
            await Task.Run(() => pluginRegistry.DiscoverAndInitialize());

            unlockVm.LoadingMessage = "Starting up...";
            await Task.Delay(30);

            // The rest must happen on the UI thread (window creation)
            await ShowMainWindow(desktop, skipPluginInit: true,
                updateStatus: msg => unlockVm.LoadingMessage = msg,
                onBeforeTransition: () =>
                {
                    // Stop the shimmer animation before heavy XAML parsing blocks the UI thread.
                    // A static "Launching..." is better UX than a frozen shimmer.
                    unlockVm.IsLaunching = true;
                    unlockVm.LoadingMessage = "Launching...";
                });
            unlockWindow.Close();
        };

        unlockVm.RecoveryRequested += (_, _) =>
        {
            Log.Information("Recovery requested, showing recovery screen");
            ShowRecoveryScreen(desktop, unlockWindow);
        };

        unlockVm.DataResetRequested += (_, _) =>
        {
            Log.Information("Data reset requested, returning to setup wizard");
            ShowSetupWizard(desktop);
            unlockWindow.Close();
        };

        desktop.MainWindow = unlockWindow;
    }

    /// <summary>
    /// Locks the app and transitions back to the unlock screen.
    /// Called from Settings → Logout.
    /// </summary>
    public void RequestLogout()
    {
        if (ApplicationLifetime is not IClassicDesktopStyleApplicationLifetime desktop) return;

        Services.GetService<IMasterPasswordCache>()?.Clear();

        var currentWindow = desktop.MainWindow;
        ShowUnlockScreen(desktop);
        currentWindow?.Close();
    }

    private void ShowRecoveryScreen(IClassicDesktopStyleApplicationLifetime desktop, UnlockWindow unlockWindow)
    {
        var recoveryVm = new RecoveryViewModel(
            Services.GetRequiredService<IAuthService>(),
            Services.GetRequiredService<IMasterPasswordCache>());

        var recoveryView = new RecoveryView { DataContext = recoveryVm };
        unlockWindow.Content = recoveryView;

        recoveryVm.RecoveryCompleted += async (_, _) =>
        {
            Log.Information("Recovery completed, loading application...");

            await Task.Delay(50);

            var pluginRegistry = Services.GetRequiredService<IPluginRegistry>();
            await Task.Run(() => pluginRegistry.DiscoverAndInitialize());

            await ShowMainWindow(desktop, skipPluginInit: true);
            unlockWindow.Close();
        };

        recoveryVm.RecoveryCancelled += (_, _) =>
        {
            Log.Information("Recovery cancelled, returning to unlock screen");
            ShowUnlockScreen(desktop);
            unlockWindow.Close();
        };
    }

    private async Task ShowMainWindow(
        IClassicDesktopStyleApplicationLifetime desktop,
        bool skipPluginInit = false,
        Action<string>? updateStatus = null,
        Action? onBeforeTransition = null,
        bool isClientMode = false)
    {
        if (!skipPluginInit)
        {
            // Discover and initialize plugins before showing the main window
            Log.Information("Discovering and initializing plugins");
            var pluginRegistry = Services.GetRequiredService<IPluginRegistry>();
            pluginRegistry.DiscoverAndInitialize();
            Log.Information("Plugin initialization complete");
        }

        updateStatus?.Invoke("Initializing security...");
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // Initialize sensitive lock service with saved settings (fast — just sets a property)
        Log.Information("Initializing sensitive lock service");
        var appSettings = Services.GetRequiredService<IAppSettingsService>();
        var lockoutMinutes = appSettings.Settings.SensitiveLockoutMinutes;
        var sensitiveLock = Services.GetRequiredService<ISensitiveLockService>();
        sensitiveLock.LockoutMinutes = lockoutMinutes;
        Log.Information("Sensitive lock service initialized with {Minutes} minute lockout (locked)", lockoutMinutes);

        updateStatus?.Invoke("Checking license...");
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // Check license expiration state for read-only enforcement banner (sets banner visibility).
        // In client mode, use the license status received from the server (native service isn't initialized).
        var expirationService = Services.GetRequiredService<LicenseExpirationService>();
        if (isClientMode)
        {
            expirationService.CheckLicenseStatusFromServer(_serverLicenseStatus);
        }
        else
        {
            var licensing = Services.GetRequiredService<ILicensingService>();
            expirationService.CheckLicenseStatus(licensing);
        }

        updateStatus?.Invoke("Preparing workspace...");
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // Resolve the ViewModel on a background thread — its constructor is UI-independent
        // but the full DI graph resolution can take 50-200ms which freezes the loading animation.
        var mainVm = await Task.Run(() => Services.GetRequiredService<MainWindowViewModel>());

        // Yield so the animation can tick between VM resolution and XAML parsing
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // Signal callers to stop animations before the heavy UI work begins.
        // new MainWindow() parses ~900 lines of XAML synchronously on the UI thread,
        // which freezes any running animation. Transitioning to a static state first
        // avoids the "frozen shimmer" perception.
        onBeforeTransition?.Invoke();
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // MainWindow + InitializeComponent must run on the UI thread
        var mainWindow = new MainWindow();

#if DEBUG
        mainWindow.AttachDevTools();
#endif

        // Yield between XAML parse and the layout/show pass
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        // Set DataContext separately — binding resolution is a distinct chunk of work
        mainWindow.DataContext = mainVm;

        // Yield between binding resolution and window show
        await Dispatcher.UIThread.InvokeAsync(() => { }, DispatcherPriority.Render);

        desktop.MainWindow = mainWindow;
        mainWindow.Show();

        // Start non-critical background services after the window is visible
        var capturedClientMode = isClientMode;
        _ = Task.Run(async () =>
        {
            try
            {
                Log.Information("Starting deferred background services (client_mode={ClientMode})", capturedClientMode);

                if (!capturedClientMode)
                {
                    // These services are only needed in standalone mode — the server handles them
                    _ = Services.GetRequiredService<IBackupService>();

                    Services.GetRequiredService<IFileEventSyncService>().Start();
                    await Services.GetRequiredService<ISnapshotSyncService>().StartAsync();

                    Services.GetRequiredService<ReminderSchedulerService>().Start();

                    // Start local HTTP API server if enabled
                    if (appSettings.Settings.ApiEnabled)
                    {
                        try
                        {
                            await Services.GetRequiredService<ILocalApiServer>().StartAsync();
                        }
                        catch (Exception apiEx)
                        {
                            Log.Error(apiEx, "Failed to start local API server");
                        }
                    }

                    // Eagerly resolve RAG index service so its auto-init task runs on startup
                    _ = Services.GetRequiredService<RagIndexService>();

                    // Pre-build backlink index so first Info Panel interaction is instant
                    _ = Services.GetRequiredService<BacklinkService>().PreBuildIndexAsync();
                }

                // These run in both modes
                _ = DatasetFileSyncHelper.ScanAndImportAsync(
                    Services.GetRequiredService<IWorkspaceService>(),
                    Services.GetRequiredService<IDatasetService>());

                Services.GetRequiredService<IIpcServer>().Start();

                var bridgePath = FindBridgePath();
                if (bridgePath != null)
                    NativeMessagingRegistrar.Register(bridgePath, appSettings);

                Log.Information("Deferred background services started");
            }
            catch (Exception ex)
            {
                Log.Error(ex, "Failed to start deferred background services");
            }
        });
    }

    /// <summary>
    /// Locates the privstack-bridge binary relative to the app installation.
    /// </summary>
    private static string? FindBridgePath()
    {
        var appDir = AppDomain.CurrentDomain.BaseDirectory;

        // Check common locations relative to the app
        string[] candidates =
        [
            Path.Combine(appDir, "privstack-bridge"),
            Path.Combine(appDir, "privstack-bridge.exe"),
            Path.Combine(appDir, "..", "bridge", "privstack-bridge"),
            Path.Combine(appDir, "..", "bridge", "privstack-bridge.exe"),
        ];

        foreach (var candidate in candidates)
        {
            var resolved = Path.GetFullPath(candidate);
            if (File.Exists(resolved)) return resolved;
        }

        return null;
    }

    private void DisableAvaloniaDataAnnotationValidation()
    {
        // Get an array of plugins to remove
        var dataValidationPluginsToRemove =
            BindingPlugins.DataValidators.OfType<DataAnnotationsValidationPlugin>().ToArray();

        // remove each entry found
        foreach (var plugin in dataValidationPluginsToRemove)
        {
            BindingPlugins.DataValidators.Remove(plugin);
        }
    }
}