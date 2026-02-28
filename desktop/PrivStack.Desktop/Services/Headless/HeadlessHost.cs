using Microsoft.Extensions.DependencyInjection;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services.Abstractions;
using PrivStack.Desktop.Services.Api;
using PrivStack.Desktop.Services.Plugin;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop.Services.Headless;

/// <summary>
/// Orchestrates PrivStack startup in headless (API-only) mode.
/// Skips Avalonia entirely — initializes core services, plugins, and the API server,
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
        // Validate setup is complete
        if (!SetupWizardViewModel.IsSetupComplete())
        {
            WriteError("PrivStack setup is not complete. Run the desktop app first to complete initial setup.");
            return ExitConfigError;
        }

        // Build headless DI container
        Log.Information("Building headless service container");
        var provider = ServiceRegistration.ConfigureHeadless();
        App.Services = provider;

        var workspaceService = provider.GetRequiredService<IWorkspaceService>();
        if (!workspaceService.HasWorkspaces)
        {
            WriteError("No workspaces found. Run the desktop app first to create a workspace.");
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

        // Authenticate
        var authService = provider.GetRequiredService<IAuthService>();
        var passwordCache = provider.GetRequiredService<IMasterPasswordCache>();

        var password = AcquireMasterPassword();
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

        if (options.Port.HasValue)
            appSettings.Settings.ApiPort = options.Port.Value;

        apiServer.BindAddress = options.BindAddress;

        // Security warning for non-localhost binding
        if (options.BindAddress is not ("127.0.0.1" or "localhost" or "::1"))
        {
            Console.Error.WriteLine($"[privstack] WARNING: Binding to {options.BindAddress} exposes the API to the network.");
            Console.Error.WriteLine("[privstack] WARNING: Ensure proper firewall rules and API key secrecy.");
        }

        // Start API server
        try
        {
            await apiServer.StartAsync();
        }
        catch (System.Net.Sockets.SocketException ex) when (ex.SocketErrorCode == System.Net.Sockets.SocketError.AddressAlreadyInUse)
        {
            WriteError($"Port {appSettings.Settings.ApiPort} is already in use.");
            return ExitPortInUse;
        }

        // Print ready banner
        var port = appSettings.Settings.ApiPort;
        var apiKey = appSettings.Settings.ApiKey;
        Console.Error.WriteLine($"[privstack] API server listening on http://{options.BindAddress}:{port}");
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

    private static Models.Workspace? ResolveWorkspace(IWorkspaceService workspaceService, string? nameOrId)
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

    private static string? AcquireMasterPassword()
    {
        // Try env var first
        var envPassword = Environment.GetEnvironmentVariable("PRIVSTACK_MASTER_PASSWORD");
        if (!string.IsNullOrEmpty(envPassword))
        {
            // Clear env var for security
            Environment.SetEnvironmentVariable("PRIVSTACK_MASTER_PASSWORD", null);
            return envPassword;
        }

        // Prompt via stdin
        if (!Console.IsInputRedirected)
        {
            Console.Error.Write("[privstack] Master password: ");
            var password = ReadPasswordMasked();
            Console.Error.WriteLine();
            return string.IsNullOrEmpty(password) ? null : password;
        }

        // Redirected stdin — read a line
        var line = Console.ReadLine();
        return string.IsNullOrEmpty(line) ? null : line.Trim();
    }

    private static string ReadPasswordMasked()
    {
        var chars = new List<char>();
        while (true)
        {
            var keyInfo = Console.ReadKey(intercept: true);
            if (keyInfo.Key == ConsoleKey.Enter)
                break;
            if (keyInfo.Key == ConsoleKey.Backspace && chars.Count > 0)
            {
                chars.RemoveAt(chars.Count - 1);
                Console.Error.Write("\b \b");
            }
            else if (!char.IsControl(keyInfo.KeyChar))
            {
                chars.Add(keyInfo.KeyChar);
                Console.Error.Write('*');
            }
        }
        return new string(chars.ToArray());
    }

    private static void WriteError(string message)
    {
        Console.Error.WriteLine($"[privstack] Error: {message}");
    }
}
