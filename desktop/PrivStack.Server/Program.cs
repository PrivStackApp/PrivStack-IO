using PrivStack.Services;

namespace PrivStack.Server;

sealed class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        // Load .env file for local development
        LoadDotEnv();

        // Initialize logging first
        Log.Initialize();

        try
        {
            Log.Information("privstack-server starting with args: {Args}", string.Join(", ", args));

            var options = ParseOptions(args);
            return HeadlessHost.RunAsync(options).GetAwaiter().GetResult();
        }
        catch (Exception ex)
        {
            Log.Fatal(ex, "privstack-server crashed with unhandled exception");
            throw;
        }
        finally
        {
            Log.Shutdown();
        }
    }

    internal static HeadlessOptions ParseOptions(string[] args)
    {
        string? workspace = null;
        int? port = null;
        string? bindAddress = null;
        bool showApiKey = false;
        bool generateApiKey = false;
        bool setup = false;
        bool setupNetwork = false;
        bool setupTls = false;
        bool setupPolicy = false;

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--workspace" when i + 1 < args.Length:
                    workspace = args[++i];
                    break;

                case "--port" when i + 1 < args.Length:
                    if (int.TryParse(args[++i], out var p))
                        port = p;
                    break;

                case "--bind" when i + 1 < args.Length:
                    bindAddress = args[++i];
                    break;

                case "--show-api-key":
                    showApiKey = true;
                    break;

                case "--generate-api-key":
                    generateApiKey = true;
                    break;

                case "--setup":
                    setup = true;
                    break;

                case "--setup-network":
                    setupNetwork = true;
                    break;

                case "--setup-tls":
                    setupTls = true;
                    break;

                case "--setup-policy":
                    setupPolicy = true;
                    break;

                case "--help" or "-h":
                    PrintUsage();
                    Environment.Exit(0);
                    break;
            }
        }

        return new HeadlessOptions
        {
            WorkspaceName = workspace,
            Port = port,
            BindAddress = bindAddress,
            ShowApiKey = showApiKey,
            GenerateApiKey = generateApiKey,
            Setup = setup,
            SetupNetwork = setupNetwork,
            SetupTls = setupTls,
            SetupPolicy = setupPolicy,
        };
    }

    private static void PrintUsage()
    {
        Console.Error.WriteLine("""
            privstack-server — Headless PrivStack API server

            Usage: privstack-server [options]

            Options:
              --setup                First-run interactive setup wizard
              --setup-network        Re-configure network settings only
              --setup-tls            Re-configure TLS settings only
              --setup-policy         Re-configure enterprise policy only
              --workspace <name|id>  Select workspace (default: active workspace)
              --port <port>          Override API port (default: from config or 9720)
              --bind <address>       Override bind address (default: from config or 127.0.0.1)
              --show-api-key         Print API key and exit
              --generate-api-key     Generate new API key and exit
              --help, -h             Show this help

            Environment:
              PRIVSTACK_MASTER_PASSWORD   Master password (avoids interactive prompt)
              PRIVSTACK_DATA_DIR          Override data directory
              PRIVSTACK_LOG_LEVEL         Log level (Verbose, Debug, Information, Warning, Error, Fatal)
            """);
    }

    private static void LoadDotEnv()
    {
        var dir = AppContext.BaseDirectory;
        string? envPath = null;

        for (var d = new DirectoryInfo(dir); d != null; d = d.Parent)
        {
            var candidate = Path.Combine(d.FullName, ".env");
            if (File.Exists(candidate))
            {
                envPath = candidate;
                break;
            }
        }

        if (envPath == null) return;

        foreach (var line in File.ReadLines(envPath))
        {
            var trimmed = line.Trim();
            if (trimmed.Length == 0 || trimmed.StartsWith('#')) continue;

            var eqIndex = trimmed.IndexOf('=');
            if (eqIndex <= 0) continue;

            var key = trimmed[..eqIndex].Trim();
            var value = trimmed[(eqIndex + 1)..].Trim();

            if (Environment.GetEnvironmentVariable(key) == null)
                Environment.SetEnvironmentVariable(key, value);
        }
    }
}
