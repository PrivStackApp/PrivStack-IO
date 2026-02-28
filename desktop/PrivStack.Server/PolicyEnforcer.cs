using System.Net;
using System.Text.Json;
using Microsoft.AspNetCore.Http;
using PrivStack.Services;
using PrivStack.Services.Abstractions;
using PrivStack.Services.Api;

namespace PrivStack.Server;

/// <summary>
/// Enforces enterprise policy rules at three points:
/// 1. Plugin loading — allowlist/blocklist via WorkspacePluginConfig
/// 2. Network access — CIDR-based IP filtering middleware
/// 3. API security — TLS requirement check at startup
/// </summary>
internal static class PolicyEnforcer
{
    private static readonly Serilog.ILogger _log = Serilog.Log.ForContext(typeof(PolicyEnforcer));

    /// <summary>
    /// Validates startup requirements. Returns an error message if the policy blocks startup, null otherwise.
    /// </summary>
    public static string? ValidateStartupRequirements(EnterprisePolicy policy, TlsOptions? tlsOptions)
    {
        if (policy.Api.RequireTls && tlsOptions == null)
        {
            return "Enterprise policy requires TLS, but TLS is not configured. Run --setup-tls to configure HTTPS.";
        }

        return null;
    }

    /// <summary>
    /// Applies plugin restrictions from the enterprise policy to the workspace plugin config.
    /// Modifies DisabledPlugins based on the policy's allowlist/blocklist mode.
    /// </summary>
    public static void ApplyPluginPolicy(EnterprisePolicy policy, IAppSettingsService appSettings)
    {
        if (policy.Plugins.Mode == "disabled" || policy.Plugins.List.Count == 0)
            return;

        var wsConfig = appSettings.GetWorkspacePluginConfig();

        if (policy.Plugins.Mode == "allowlist")
        {
            // Only plugins in the list are allowed — set whitelist mode
            wsConfig.EnabledPlugins = [..policy.Plugins.List];
            _log.Information("Enterprise policy: allowlist mode — enabled plugins: {Plugins}",
                string.Join(", ", policy.Plugins.List));
        }
        else if (policy.Plugins.Mode == "blocklist")
        {
            // Merge policy-blocked plugins into DisabledPlugins
            foreach (var pluginId in policy.Plugins.List)
            {
                wsConfig.DisabledPlugins.Add(pluginId);
            }
            _log.Information("Enterprise policy: blocklist mode — disabled plugins: {Plugins}",
                string.Join(", ", policy.Plugins.List));
        }

        appSettings.Save();
    }

    /// <summary>
    /// Creates ASP.NET Core middleware that enforces network CIDR restrictions.
    /// Blocks requests from IPs outside the allowed CIDR ranges.
    /// </summary>
    public static Func<HttpContext, RequestDelegate, Task> CreateNetworkMiddleware(EnterprisePolicy policy)
    {
        if (policy.Network.AllowedCidrs.Count == 0)
            return (ctx, next) => next(ctx); // No restrictions — pass through

        var networks = new List<(IPAddress Network, int PrefixLength)>();
        foreach (var cidr in policy.Network.AllowedCidrs)
        {
            var parts = cidr.Split('/');
            if (parts.Length == 2 && IPAddress.TryParse(parts[0], out var addr) && int.TryParse(parts[1], out var prefix))
            {
                networks.Add((addr, prefix));
            }
            else
            {
                _log.Warning("Invalid CIDR in enterprise policy: {Cidr}", cidr);
            }
        }

        _log.Information("Enterprise policy: network access restricted to {Count} CIDR range(s)", networks.Count);

        return async (context, next) =>
        {
            var remoteIp = context.Connection.RemoteIpAddress;
            if (remoteIp == null)
            {
                context.Response.StatusCode = 403;
                context.Response.ContentType = "application/json";
                await context.Response.WriteAsync("""{"error":"Forbidden: no remote IP"}""");
                return;
            }

            // Map IPv6-mapped IPv4 back to IPv4 for matching
            if (remoteIp.IsIPv4MappedToIPv6)
                remoteIp = remoteIp.MapToIPv4();

            var allowed = false;
            foreach (var (network, prefixLength) in networks)
            {
                if (IsInSubnet(remoteIp, network, prefixLength))
                {
                    allowed = true;
                    break;
                }
            }

            if (!allowed)
            {
                _log.Warning("Enterprise policy: blocked request from {RemoteIp}", remoteIp);
                context.Response.StatusCode = 403;
                context.Response.ContentType = "application/json";
                await context.Response.WriteAsync(
                    JsonSerializer.Serialize(new { error = "Forbidden: IP not in allowed range" }));
                return;
            }

            await next(context);
        };
    }

    /// <summary>
    /// Checks if an IP address is within a CIDR subnet.
    /// </summary>
    private static bool IsInSubnet(IPAddress address, IPAddress network, int prefixLength)
    {
        var addrBytes = address.GetAddressBytes();
        var netBytes = network.GetAddressBytes();

        if (addrBytes.Length != netBytes.Length)
        {
            // IPv4 vs IPv6 mismatch — try mapping
            if (address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetworkV6 &&
                network.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork)
            {
                if (address.IsIPv4MappedToIPv6)
                    addrBytes = address.MapToIPv4().GetAddressBytes();
                else
                    return false;
            }
            else
            {
                return false;
            }
        }

        var fullBytes = prefixLength / 8;
        var remainingBits = prefixLength % 8;

        for (int i = 0; i < fullBytes; i++)
        {
            if (addrBytes[i] != netBytes[i])
                return false;
        }

        if (remainingBits > 0 && fullBytes < addrBytes.Length)
        {
            var mask = (byte)(0xFF << (8 - remainingBits));
            if ((addrBytes[fullBytes] & mask) != (netBytes[fullBytes] & mask))
                return false;
        }

        return true;
    }
}
