using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Native;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Api;
using PrivStack.Services.Models;
using PrivStack.Services.Plugin;

namespace PrivStack.Server;

/// <summary>
/// Orchestrates PrivStack startup in headless (API-only) mode.
/// No Avalonia — initializes core services, plugins, and the API server,
/// then blocks until SIGTERM/SIGINT.
/// </summary>
internal static class HeadlessHost
{
    // Exit codes
    private const int ExitSuccess = 0;
    private const int ExitConfigError = 1;
    private const int ExitAuthError = 2;
    private const int ExitPortInUse = 3;
    private const int ExitDbLocked = 4;

    public static async Task<int> RunAsync(HeadlessOptions options)
    {
        try
        {
            return await RunCoreAsync(options);
        }
        catch (Exception ex)
        {
            Log.Fatal(ex, "Headless mode crashed");
            return ExitConfigError;
        }
    }

    private static async Task<int> RunCoreAsync(HeadlessOptions options)
    {
        // Check setup — uses the same settings.json that the desktop wizard writes
        if (!IsSetupComplete())
        {
            if (!Console.IsInputRedirected && options.Setup)
            {
                // Interactive terminal + --setup → run the setup wizard
                var wizardResult = await HeadlessSetupWizard.RunAsync();
                if (wizardResult != 0) return wizardResult;
            }
            else if (!Console.IsInputRedirected)
            {
                WriteError("PrivStack is not set up. Run with --setup for first-time configuration.");
                return ExitConfigError;
            }
            else
            {
                WriteError("PrivStack is not set up and stdin is redirected. Run interactively with --setup first.");
                return ExitConfigError;
            }
        }
        else if (options.Setup)
        {
            // Re-run setup even though already complete
            var wizardResult = await HeadlessSetupWizard.RunAsync();
            if (wizardResult != 0) return wizardResult;
        }

        // Build headless DI container
        Log.Information("Building headless service container");
        var provider = ServerServiceRegistration.Configure();
        ServiceProviderAccessor.Services = provider;

        var workspaceService = provider.GetRequiredService<IWorkspaceService>();
        if (!workspaceService.HasWorkspaces)
        {
            WriteError("No workspaces found. Run with --setup to create one.");
            return ExitConfigError;
        }

        // Resolve workspace
        var workspace = ResolveWorkspace(workspaceService, options.WorkspaceName);
        if (workspace == null)
        {
            if (options.WorkspaceName != null)
                WriteError($"Workspace not found: '{options.WorkspaceName}'");
            else
                WriteError("No active workspace. Specify --workspace <name>.");
            return ExitConfigError;
        }

        // Set up workspace paths
        var resolvedDir = workspaceService.ResolveWorkspaceDir(workspace);
        DataPaths.SetActiveWorkspace(workspace.Id, resolvedDir);
        Log.ReconfigureForWorkspace(workspace.Id);

        var dbPath = workspaceService.GetActiveDataPath();
        var dir = Path.GetDirectoryName(dbPath)!;
        Directory.CreateDirectory(dir);

        // Initialize Rust core
        Log.Information("Initializing native runtime for workspace: {Name}", workspace.Name);
        try
        {
            provider.GetRequiredService<IPrivStackRuntime>().Initialize(dbPath);
        }
        catch (Exception ex) when (ex.Message.Contains("locked", StringComparison.OrdinalIgnoreCase) ||
                                    ex.Message.Contains("busy", StringComparison.OrdinalIgnoreCase))
        {
            WriteError($"Database is locked — is another PrivStack instance running? ({ex.Message})");
            return ExitDbLocked;
        }

        // Load headless config
        var headlessConfig = HeadlessConfig.Load();

        // Authenticate
        var authService = provider.GetRequiredService<IAuthService>();
        var passwordCache = provider.GetRequiredService<IMasterPasswordCache>();

        var password = AcquireMasterPassword(headlessConfig);
        if (password == null)
        {
            WriteError("No master password provided. Set PRIVSTACK_MASTER_PASSWORD or provide via stdin.");
            return ExitAuthError;
        }

        try
        {
            authService.UnlockApp(password);
            passwordCache.Set(password);
        }
        catch (Exception ex)
        {
            WriteError($"Authentication failed: {ex.Message}");
            return ExitAuthError;
        }

        // Handle utility commands (exit early)
        var appSettings = provider.GetRequiredService<IAppSettingsService>();

        if (options.ShowApiKey)
        {
            var key = appSettings.Settings.ApiKey;
            if (string.IsNullOrEmpty(key))
            {
                WriteError("No API key configured. Use --generate-api-key to create one.");
                return ExitConfigError;
            }
            Console.WriteLine(key);
            return ExitSuccess;
        }

        if (options.GenerateApiKey)
        {
            var keyBytes = new byte[32];
            System.Security.Cryptography.RandomNumberGenerator.Fill(keyBytes);
            var newKey = Convert.ToBase64String(keyBytes)
                .Replace("+", "-").Replace("/", "_").TrimEnd('=');
            appSettings.Settings.ApiKey = newKey;
            appSettings.Save();
            Console.WriteLine(newKey);
            return ExitSuccess;
        }

        // Discover and initialize plugins
        Log.Information("Discovering and initializing plugins");
        var pluginRegistry = provider.GetRequiredService<IPluginRegistry>();
        pluginRegistry.DiscoverAndInitialize();
        Log.Information("Plugin initialization complete");

        // Configure API server
        var apiServer = provider.GetRequiredService<ILocalApiServer>();

        var bindAddress = options.BindAddress ?? headlessConfig.BindAddress;
        var port = options.Port ?? headlessConfig.Port;

        appSettings.Settings.ApiPort = port;
        apiServer.BindAddress = bindAddress;

        // TLS config will be applied in Phase 3 when ILocalApiServer.TlsConfig is added

        // Security warning for non-localhost binding
        if (bindAddress is not ("127.0.0.1" or "localhost" or "::1"))
        {
            Console.Error.WriteLine($"[privstack] WARNING: Binding to {bindAddress} exposes the API to the network.");
            Console.Error.WriteLine("[privstack] WARNING: Ensure proper firewall rules and API key secrecy.");
        }

        // Start API server
        try
        {
            await apiServer.StartAsync();
        }
        catch (System.Net.Sockets.SocketException ex) when (ex.SocketErrorCode == System.Net.Sockets.SocketError.AddressAlreadyInUse)
        {
            WriteError($"Port {port} is already in use.");
            return ExitPortInUse;
        }

        // Print ready banner
        var apiKey = appSettings.Settings.ApiKey;
        var protocol = headlessConfig.Tls is { Enabled: true } ? "https" : "http";
        Console.Error.WriteLine($"[privstack] API server listening on {protocol}://{bindAddress}:{port}");
        Console.Error.WriteLine($"[privstack] Workspace: {workspace.Name} ({workspace.Id})");
        Console.Error.WriteLine($"[privstack] API key: {apiKey}");
        Console.Error.WriteLine("[privstack] Press Ctrl+C to stop.");

        // Block until SIGTERM/SIGINT
        var shutdownCts = new CancellationTokenSource();

        Console.CancelKeyPress += (_, e) =>
        {
            e.Cancel = true;
            shutdownCts.Cancel();
        };

        AppDomain.CurrentDomain.ProcessExit += (_, _) =>
        {
            shutdownCts.Cancel();
        };

        try
        {
            await Task.Delay(Timeout.Infinite, shutdownCts.Token);
        }
        catch (OperationCanceledException)
        {
            // Expected — shutdown signal received
        }

        // Graceful shutdown
        Console.Error.WriteLine("[privstack] Shutting down...");
        await apiServer.StopAsync();
        appSettings.Flush();
        provider.GetRequiredService<IPrivStackRuntime>().Shutdown();
        Log.Information("Headless mode shutdown complete");

        return ExitSuccess;
    }

    internal static bool IsSetupComplete()
    {
        var settingsPath = Path.Combine(DataPaths.BaseDir, "settings.json");
        if (!File.Exists(settingsPath)) return false;

        try
        {
            var json = System.Text.Json.JsonDocument.Parse(File.ReadAllText(settingsPath));
            return json.RootElement.TryGetProperty("SetupComplete", out var prop) && prop.GetBoolean();
        }
        catch
        {
            return false;
        }
    }

    private static Workspace? ResolveWorkspace(IWorkspaceService workspaceService, string? nameOrId)
    {
        if (nameOrId == null)
            return workspaceService.GetActiveWorkspace();

        var workspaces = workspaceService.ListWorkspaces();

        // Try exact ID match first
        var byId = workspaces.FirstOrDefault(w =>
            w.Id.Equals(nameOrId, StringComparison.OrdinalIgnoreCase));
        if (byId != null) return byId;

        // Try case-insensitive name match
        var byName = workspaces.FirstOrDefault(w =>
            w.Name.Equals(nameOrId, StringComparison.OrdinalIgnoreCase));
        return byName;
    }

    private static string? AcquireMasterPassword(HeadlessConfig config)
    {
        // Method 1: Environment variable
        var envPassword = Environment.GetEnvironmentVariable("PRIVSTACK_MASTER_PASSWORD");
        if (!string.IsNullOrEmpty(envPassword))
        {
            // Clear env var for security
            Environment.SetEnvironmentVariable("PRIVSTACK_MASTER_PASSWORD", null);
            return envPassword;
        }

        // Method 2: OS keyring (if configured)
        if (config.UnlockMethod == UnlockMethod.OsKeyring)
        {
            var keyring = KeyringServiceFactory.Create();
            if (keyring.IsAvailable)
            {
                var stored = keyring.Retrieve("com.privstack.headless", "master-password");
                if (stored != null) return stored;
            }
            // Fall through to prompt if keyring unavailable or empty
        }

        // Method 3: EnvironmentVariable mode — fail if not set
        if (config.UnlockMethod == UnlockMethod.EnvironmentVariable)
        {
            return null; // Already tried env var above
        }

        // Method 4: Interactive stdin prompt
        if (!Console.IsInputRedirected)
        {
            Console.Error.Write("[privstack] Master password: ");
            var password = ConsoleUi.ReadPassword();
            Console.Error.WriteLine();
            return string.IsNullOrEmpty(password) ? null : password;
        }

        // Method 5: Redirected stdin — read a line
        var line = Console.ReadLine();
        return string.IsNullOrEmpty(line) ? null : line.Trim();
    }

    private static void WriteError(string message)
    {
        Console.Error.WriteLine($"[privstack] Error: {message}");
    }
}
