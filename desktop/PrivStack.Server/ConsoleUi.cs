namespace PrivStack.Server;

/// <summary>
/// Reusable console prompt helpers for the headless server CLI.
/// All UI output goes to stderr. Machine-readable output (API keys) goes to stdout.
/// </summary>
internal static class ConsoleUi
{
    public static string ReadPassword(string prompt = "Password: ")
    {
        Console.Error.Write(prompt);
        var result = ReadPasswordMasked();
        Console.Error.WriteLine();
        return result;
    }

    public static string? ReadPasswordConfirmed(string prompt = "Password: ", string confirmPrompt = "Confirm password: ", int minLength = 8)
    {
        while (true)
        {
            var password = ReadPassword(prompt);
            if (password.Length < minLength)
            {
                WriteWarning($"Password must be at least {minLength} characters.");
                continue;
            }

            var confirm = ReadPassword(confirmPrompt);
            if (password != confirm)
            {
                WriteWarning("Passwords do not match. Try again.");
                continue;
            }

            return password;
        }
    }

    public static int MenuSelect(string prompt, params string[] options)
    {
        Console.Error.WriteLine(prompt);
        for (int i = 0; i < options.Length; i++)
            Console.Error.WriteLine($"  [{i + 1}] {options[i]}");

        while (true)
        {
            Console.Error.Write($"Choice [1-{options.Length}]: ");
            var line = Console.ReadLine()?.Trim();
            if (int.TryParse(line, out var choice) && choice >= 1 && choice <= options.Length)
                return choice - 1;
            WriteWarning("Invalid choice. Try again.");
        }
    }

    public static string ReadLine(string prompt, string? defaultValue = null)
    {
        if (defaultValue != null)
            Console.Error.Write($"{prompt} [{defaultValue}]: ");
        else
            Console.Error.Write($"{prompt}: ");

        var line = Console.ReadLine()?.Trim();
        return string.IsNullOrEmpty(line) ? (defaultValue ?? "") : line;
    }

    public static bool YesNo(string prompt, bool defaultYes = true)
    {
        var hint = defaultYes ? "[Y/n]" : "[y/N]";
        Console.Error.Write($"{prompt} {hint}: ");
        var line = Console.ReadLine()?.Trim().ToLowerInvariant();
        return line switch
        {
            "y" or "yes" => true,
            "n" or "no" => false,
            _ => defaultYes,
        };
    }

    public static void WriteSuccess(string message)
        => WriteColored($"  [OK] {message}", ConsoleColor.Green);

    public static void WriteWarning(string message)
        => WriteColored($"  [!] {message}", ConsoleColor.Yellow);

    public static void WriteError(string message)
        => WriteColored($"  [ERROR] {message}", ConsoleColor.Red);

    public static void WriteBanner()
    {
        Console.Error.WriteLine();
        Console.Error.WriteLine("  PrivStack Server Setup");
        Console.Error.WriteLine("  =====================");
        Console.Error.WriteLine();
    }

    public static void WriteSection(string title)
    {
        Console.Error.WriteLine();
        Console.Error.WriteLine($"--- {title} ---");
        Console.Error.WriteLine();
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

    private static void WriteColored(string message, ConsoleColor color)
    {
        var prev = Console.ForegroundColor;
        Console.ForegroundColor = color;
        Console.Error.WriteLine(message);
        Console.ForegroundColor = prev;
    }
}
