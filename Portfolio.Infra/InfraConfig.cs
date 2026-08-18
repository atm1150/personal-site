using System.Net;

namespace Portfolio.Infra;

/// <summary>
/// Reads and validates apply-time environment variables before any Pulumi resource is declared.
/// Pure static class: no Pulumi dependency, fully unit-testable.
/// </summary>
internal static class InfraConfig
{
    internal const string AdminIpVar = "INFRA_ADMIN_IP";
    internal const string SshPublicKeyVar = "INFRA_SSH_PUBLIC_KEY";

    /// <summary>
    /// Returns the admin IPv4 address from <see cref="AdminIpVar"/>.
    /// Throws <see cref="InvalidOperationException"/> if unset or not a valid IPv4 address.
    /// </summary>
    internal static string RequireAdminIp()
    {
        var raw = Environment.GetEnvironmentVariable(AdminIpVar);

        if (string.IsNullOrWhiteSpace(raw))
            throw new InvalidOperationException(
                $"Environment variable {AdminIpVar} is required but not set.");

        if (!IPAddress.TryParse(raw, out var addr) || addr.AddressFamily != System.Net.Sockets.AddressFamily.InterNetwork)
            throw new InvalidOperationException(
                $"Environment variable {AdminIpVar} must be a valid IPv4 address; got: '{raw}'.");

        return raw;
    }

    /// <summary>
    /// Returns the SSH public key text from <see cref="SshPublicKeyVar"/>.
    /// Throws <see cref="InvalidOperationException"/> if unset or empty.
    /// </summary>
    internal static string RequireSshPublicKey()
    {
        var raw = Environment.GetEnvironmentVariable(SshPublicKeyVar);

        if (string.IsNullOrWhiteSpace(raw))
            throw new InvalidOperationException(
                $"Environment variable {SshPublicKeyVar} is required but not set.");

        return raw;
    }
}
