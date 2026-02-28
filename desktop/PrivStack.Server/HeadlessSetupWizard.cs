using Microsoft.Extensions.DependencyInjection;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Native;

namespace PrivStack.Server;

/// <summary>
/// Interactive first-run setup wizard for the headless server.
/// Creates workspace, sets master password, configures network/TLS, and saves headless config.
/// </summary>
internal static class HeadlessSetupWizard
{
    public static async Task<int> RunAsync()
    {
        ConsoleUi.WriteBanner();

        // Step 1: Workspace
        ConsoleUi.WriteSection("Workspace Setup");
        var workspaceName = ConsoleUi.ReadLine("Workspace name", "Personal");
        ConsoleUi.WriteSuccess($"Workspace: {workspaceName}");

        // Step 2: Master password
        ConsoleUi.WriteSection("Master Password");
        Console.Error.WriteLine("  Your master password encrypts all PrivStack data.");
        Console.Error.WriteLine("  Choose a strong password — it cannot be recovered if lost.");
        Console.Error.WriteLine();

        var password = ConsoleUi.ReadPasswordConfirmed(
            "  Master password: ",
            "  Confirm password: ",
            minLength: 8);

        if (password == null)
        {
            ConsoleUi.WriteError("Password setup cancelled.");
            return 1;
        }

        // Step 3: Unlock method
        ConsoleUi.WriteSection("Unlock Method");
        var unlockChoice = ConsoleUi.MenuSelect(
            "  How should the server unlock on startup?",
            "Password every start (most secure)",
            "OS keyring — store password in system credential manager",
            "Environment variable — read PRIVSTACK_MASTER_PASSWORD");

        var unlockMethod = unlockChoice switch
        {
            0 => UnlockMethod.PasswordEveryStart,
            1 => UnlockMethod.OsKeyring,
            2 => UnlockMethod.EnvironmentVariable,
            _ => UnlockMethod.PasswordEveryStart,
        };

        // Step 4: Network
        ConsoleUi.WriteSection("Network Configuration");
        var bindAddress = ConsoleUi.ReadLine("  Bind address", "127.0.0.1");
        var portStr = ConsoleUi.ReadLine("  Port", "9720");
        var port = int.TryParse(portStr, out var p) ? p : 9720;

        if (bindAddress is not ("127.0.0.1" or "localhost" or "::1"))
        {
            ConsoleUi.WriteWarning($"Binding to {bindAddress} exposes the API to the network.");
            if (!ConsoleUi.YesNo("  Continue?", defaultYes: false))
            {
                bindAddress = "127.0.0.1";
                ConsoleUi.WriteSuccess("Reverted to 127.0.0.1");
            }
        }

        // Step 5: TLS (optional)
        TlsConfig? tlsConfig = null;
        ConsoleUi.WriteSection("TLS Configuration");
        if (ConsoleUi.YesNo("  Enable TLS (HTTPS)?", defaultYes: false))
        {
            var certPath = ConsoleUi.ReadLine("  Certificate path (.pem or .pfx)");
            var keyPath = ConsoleUi.ReadLine("  Private key path (.pem)", "");

            if (File.Exists(certPath))
            {
                tlsConfig = new TlsConfig { Enabled = true, CertPath = certPath, KeyPath = keyPath };
                ConsoleUi.WriteSuccess("TLS configured.");
            }
            else
            {
                ConsoleUi.WriteWarning($"Certificate file not found: {certPath}. TLS disabled.");
            }
        }

        // Now execute the setup
        ConsoleUi.WriteSection("Initializing");

        // Build a temporary DI container for setup
        var provider = ServerServiceRegistration.Configure();
        ServiceProviderAccessor.Services = provider;

        var workspaceService = provider.GetRequiredService<IWorkspaceService>();
        var runtime = provider.GetRequiredService<IPrivStackRuntime>();
        var authService = provider.GetRequiredService<IAuthService>();
        var passwordCache = provider.GetRequiredService<IMasterPasswordCache>();
        var appSettings = provider.GetRequiredService<IAppSettingsService>();

        try
        {
            // Create workspace
            var workspace = workspaceService.CreateWorkspace(workspaceName, null, makeActive: true);
            var resolvedDir = workspaceService.ResolveWorkspaceDir(workspace);
            DataPaths.SetActiveWorkspace(workspace.Id, resolvedDir);
            ConsoleUi.WriteSuccess($"Created workspace: {workspace.Name} ({workspace.Id})");

            // Initialize database
            var dbPath = workspaceService.GetActiveDataPath();
            Directory.CreateDirectory(Path.GetDirectoryName(dbPath)!);
            runtime.Initialize(dbPath);
            ConsoleUi.WriteSuccess("Database initialized.");

            // Set up authentication
            authService.InitializeAuth(password);
            passwordCache.Set(password);
            ConsoleUi.WriteSuccess("Master password configured.");

            // Generate recovery mnemonic
            try
            {
                var mnemonic = authService.SetupRecovery();
                if (!string.IsNullOrEmpty(mnemonic))
                {
                    Console.Error.WriteLine();
                    ConsoleUi.WriteWarning("RECOVERY PHRASE — write this down and store securely:");
                    Console.Error.WriteLine();
                    Console.Error.WriteLine($"    {mnemonic}");
                    Console.Error.WriteLine();
                    ConsoleUi.WriteWarning("This is the ONLY way to recover your data if you forget your password.");
                    Console.Error.WriteLine();
                    ConsoleUi.ReadLine("  Press Enter to continue");
                }
            }
            catch (Exception ex)
            {
                Log.Warning(ex, "Recovery setup failed (non-fatal)");
            }

            // Store in keyring if requested
            if (unlockMethod == UnlockMethod.OsKeyring)
            {
                var keyring = KeyringServiceFactory.Create();
                if (keyring.IsAvailable)
                {
                    keyring.Store("com.privstack.headless", "master-password", password);
                    ConsoleUi.WriteSuccess("Password stored in OS keyring.");
                }
                else
                {
                    ConsoleUi.WriteWarning("OS keyring not available. Falling back to password-every-start.");
                    unlockMethod = UnlockMethod.PasswordEveryStart;
                }
            }

            // Generate API key
            var keyBytes = new byte[32];
            System.Security.Cryptography.RandomNumberGenerator.Fill(keyBytes);
            var apiKey = Convert.ToBase64String(keyBytes)
                .Replace("+", "-").Replace("/", "_").TrimEnd('=');
            appSettings.Settings.ApiEnabled = true;
            appSettings.Settings.ApiPort = port;
            appSettings.Settings.ApiKey = apiKey;
            appSettings.Save();

            // Write setup complete marker
            var settingsPath = Path.Combine(DataPaths.BaseDir, "settings.json");
            var settingsJson = File.Exists(settingsPath) ? File.ReadAllText(settingsPath) : "{}";
            var doc = System.Text.Json.JsonDocument.Parse(settingsJson);
            using var ms = new MemoryStream();
            using (var writer = new System.Text.Json.Utf8JsonWriter(ms, new System.Text.Json.JsonWriterOptions { Indented = true }))
            {
                writer.WriteStartObject();
                foreach (var prop in doc.RootElement.EnumerateObject())
                {
                    if (prop.Name == "SetupComplete") continue;
                    prop.WriteTo(writer);
                }
                writer.WriteBoolean("SetupComplete", true);
                writer.WriteEndObject();
            }
            File.WriteAllText(settingsPath, System.Text.Encoding.UTF8.GetString(ms.ToArray()));

            // Save headless config
            var headlessConfig = new HeadlessConfig
            {
                UnlockMethod = unlockMethod,
                BindAddress = bindAddress,
                Port = port,
                Tls = tlsConfig,
            };
            headlessConfig.Save();

            ConsoleUi.WriteSection("Setup Complete");
            Console.Error.WriteLine($"  API key: {apiKey}");
            Console.Error.WriteLine($"  Endpoint: {(tlsConfig?.Enabled == true ? "https" : "http")}://{bindAddress}:{port}");
            Console.Error.WriteLine();
            ConsoleUi.WriteSuccess("PrivStack server is ready. Restart without --setup to begin.");
            Console.Error.WriteLine();

            // Shut down the temporary runtime
            runtime.Shutdown();

            return 0;
        }
        catch (Exception ex)
        {
            ConsoleUi.WriteError($"Setup failed: {ex.Message}");
            Log.Error(ex, "Setup wizard failed");
            return 1;
        }
    }
}
