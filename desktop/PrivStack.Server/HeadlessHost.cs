using System.Net;
using LettuceEncrypt;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Server.Kestrel.Core;
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
        else if (options.SetupNetwork)
        {
            // Re-configure network settings only
            ConsoleUi.WriteSection("Network Configuration");
            var existingConfig = HeadlessConfig.Load();
            var bindAddr = ConsoleUi.ReadLine("  Bind address", existingConfig.BindAddress);
            var portStr = ConsoleUi.ReadLine("  Port", existingConfig.Port.ToString());
            var portVal = int.TryParse(portStr, out var pv) ? pv : existingConfig.Port;

            if (bindAddr is not ("127.0.0.1" or "localhost" or "::1"))
            {
                ConsoleUi.WriteWarning($"Binding to {bindAddr} exposes the API to the network.");
                if (!ConsoleUi.YesNo("  Continue?", defaultYes: false))
                {
                    bindAddr = existingConfig.BindAddress;
                    ConsoleUi.WriteSuccess($"Reverted to {bindAddr}");
                }
            }

            existingConfig.BindAddress = bindAddr;
            existingConfig.Port = portVal;
            existingConfig.Save();
            ConsoleUi.WriteSuccess($"Network configuration saved: {bindAddr}:{portVal}. Restart the server to apply.");
            return ExitSuccess;
        }
        else if (options.SetupTls)
        {
            // Re-configure TLS only
            ConsoleUi.WriteSection("TLS Configuration");
            var existingConfig = HeadlessConfig.Load();
            var tlsCfg = HeadlessSetupWizard.ConfigureTlsInteractive();
            existingConfig.Tls = tlsCfg;
            existingConfig.Save();
            ConsoleUi.WriteSuccess("TLS configuration saved. Restart the server to apply.");
            return ExitSuccess;
        }
        else if (options.SetupPolicy)
        {
            // Re-configure enterprise policy only
            ConsoleUi.WriteSection("Enterprise Policy");
            var existingConfig = HeadlessConfig.Load();
            var policyPath = ConsoleUi.ReadLine("  Policy file path (.toml)", existingConfig.PolicyPath ?? "");
            existingConfig.PolicyPath = string.IsNullOrEmpty(policyPath) ? null : policyPath;
            existingConfig.Save();

            if (!string.IsNullOrEmpty(policyPath) && File.Exists(policyPath))
            {
                try
                {
                    var policy = EnterprisePolicy.LoadFromFile(policyPath);
                    ConsoleUi.WriteSuccess($"Policy loaded and validated: {policyPath}");
                    if (policy!.Plugins.Mode != "disabled")
                        Console.Error.WriteLine($"    Plugins: {policy.Plugins.Mode} [{string.Join(", ", policy.Plugins.List)}]");
                    if (policy.Network.AllowedCidrs.Count > 0)
                        Console.Error.WriteLine($"    Network: {policy.Network.AllowedCidrs.Count} CIDR range(s)");
                    if (policy.Api.RequireTls)
                        Console.Error.WriteLine("    API: TLS required");
                    if (policy.Audit.Enabled)
                        Console.Error.WriteLine($"    Audit: enabled → {policy.Audit.LogPath}");
                }
                catch (Exception ex)
                {
                    ConsoleUi.WriteError($"Policy validation failed: {ex.Message}");
                    return ExitConfigError;
                }
            }
            else if (!string.IsNullOrEmpty(policyPath))
            {
                ConsoleUi.WriteWarning($"Policy file not found: {policyPath}");
            }
            else
            {
                ConsoleUi.WriteSuccess("Enterprise policy disabled.");
            }

            return ExitSuccess;
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

        // Load enterprise policy (if configured)
        EnterprisePolicy? enterprisePolicy = null;
        AuditLogger? auditLogger = null;

        if (!string.IsNullOrEmpty(headlessConfig.PolicyPath))
        {
            try
            {
                enterprisePolicy = EnterprisePolicy.LoadFromFile(headlessConfig.PolicyPath);
                if (enterprisePolicy != null)
                {
                    Log.Information("Loaded enterprise policy from {Path}", headlessConfig.PolicyPath);

                    // Apply plugin restrictions before discovery
                    PolicyEnforcer.ApplyPluginPolicy(enterprisePolicy, appSettings);
                }
                else
                {
                    Log.Warning("Enterprise policy file not found: {Path}", headlessConfig.PolicyPath);
                }
            }
            catch (Exception ex)
            {
                WriteError($"Enterprise policy error: {ex.Message}");
                return ExitConfigError;
            }
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

        // Apply TLS configuration
        var tlsOptions = headlessConfig.ToTlsOptions();
        if (tlsOptions != null)
        {
            apiServer.TlsOptions = tlsOptions;

            if (tlsOptions.Mode == TlsMode.LetsEncrypt)
            {
                ConfigureLetsEncrypt(apiServer, tlsOptions, bindAddress, port);
            }
        }

        // Enforce enterprise policy startup requirements
        if (enterprisePolicy != null)
        {
            var policyError = PolicyEnforcer.ValidateStartupRequirements(enterprisePolicy, tlsOptions);
            if (policyError != null)
            {
                WriteError(policyError);
                return ExitConfigError;
            }

            // Set up network CIDR middleware
            if (enterprisePolicy.Network.AllowedCidrs.Count > 0)
            {
                var networkMiddleware = PolicyEnforcer.CreateNetworkMiddleware(enterprisePolicy);
                var existingOnConfigureApp = ((LocalApiServer)apiServer).OnConfigureApp;
                ((LocalApiServer)apiServer).OnConfigureApp = app =>
                {
                    existingOnConfigureApp?.Invoke(app);
                    app.Use(next => ctx => networkMiddleware(ctx, next));
                };
            }

            // Set up audit logging
            if (enterprisePolicy.Audit is { Enabled: true, LogPath: not null })
            {
                auditLogger = new AuditLogger(enterprisePolicy.Audit);
                auditLogger.LogPolicyEvent("startup", $"Enterprise policy loaded from {headlessConfig.PolicyPath}");

                var auditMiddleware = auditLogger.CreateMiddleware();
                var existingOnConfigureApp2 = ((LocalApiServer)apiServer).OnConfigureApp;
                ((LocalApiServer)apiServer).OnConfigureApp = app =>
                {
                    existingOnConfigureApp2?.Invoke(app);
                    app.Use(next => ctx => auditMiddleware(ctx, next));
                };
            }
        }

        // Security warning for non-localhost binding
        if (bindAddress is not ("127.0.0.1" or "localhost" or "::1"))
        {
            Console.Error.WriteLine($"[privstack] WARNING: Binding to {bindAddress} exposes the API to the network.");
            if (tlsOptions == null)
                Console.Error.WriteLine("[privstack] WARNING: TLS is not enabled. Run --setup-tls to configure HTTPS.");
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
        if (enterprisePolicy != null)
            Console.Error.WriteLine($"[privstack] Enterprise policy: {headlessConfig.PolicyPath}");
        if (auditLogger != null)
            Console.Error.WriteLine($"[privstack] Audit log: {enterprisePolicy!.Audit.LogPath}");
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
        auditLogger?.LogPolicyEvent("shutdown", "Server shutting down");
        auditLogger?.Dispose();
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

    /// <summary>
    /// Configures LettuceEncrypt for automatic ACME certificate provisioning.
    /// Sets up Kestrel to listen on HTTPS (configured port) + HTTP port 80 for ACME challenges.
    /// </summary>
    private static void ConfigureLetsEncrypt(ILocalApiServer apiServer, TlsOptions tls, string bindAddress, int port)
    {
        var server = (LocalApiServer)apiServer;
        var domain = tls.Domain!;
        var email = tls.Email!;
        var certStorePath = tls.CertStorePath ?? Path.Combine(DataPaths.BaseDir, "certs");
        Directory.CreateDirectory(certStorePath);

        server.OnConfigureServices = services =>
        {
            services.AddLettuceEncrypt(opts =>
            {
                opts.AcceptTermsOfService = tls.AcceptTermsOfService;
                opts.DomainNames = [domain];
                opts.EmailAddress = email;
                opts.UseStagingServer = tls.UseStaging;
            }).PersistDataToDirectory(new DirectoryInfo(certStorePath), null);
        };

        server.OnConfigureKestrel = k =>
        {
            var addr = bindAddress is "0.0.0.0"
                ? IPAddress.Any
                : IPAddress.Parse(bindAddress);

            // HTTPS on configured port — LettuceEncrypt provides the certificate
            k.Listen(addr, port, lo =>
            {
                lo.UseHttps(h => h.UseLettuceEncrypt(k.ApplicationServices));
            });

            // HTTP on port 80 for ACME HTTP-01 challenges (required for domain validation)
            if (port != 80)
            {
                k.Listen(addr, 80);
            }
        };

        Log.Information("Let's Encrypt configured for domain {Domain} (staging: {Staging})",
            domain, tls.UseStaging);
    }

    private static void WriteError(string message)
    {
        Console.Error.WriteLine($"[privstack] Error: {message}");
    }
}
