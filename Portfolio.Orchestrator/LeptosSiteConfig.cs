using System.Globalization;
using System.Net;
using System.Text.Json.Serialization;
using Tomlyn;

namespace Aspire.Hosting;

/// <summary>
/// The TCP port the Leptos site serves HTTP on, parsed from <c>site-addr</c> in the
/// workspace <c>Cargo.toml</c>. Always valid once constructed: the constructor is the
/// only producer and it enforces the port range.
/// </summary>
/// <remarks>
/// <para>
/// <see cref="SitePort"/> and <see cref="ReloadPort"/> are distinct wrapper types so that
/// handing one port to an API expecting the other is a compile error, not a runtime
/// surprise. There are deliberately no implicit conversions to or from <see cref="int"/>:
/// both types would widen to the same <c>int</c>, which would silently re-enable exactly
/// that mix-up at call sites like <c>WithHttpEndpoint(port: …)</c>. Crossing back into an
/// <c>int</c>-typed API is always explicit via <see cref="Value"/>.
/// </para>
/// <para>
/// One hole C# structs leave open: <c>default(SitePort)</c> bypasses the constructor and
/// holds 0. Nothing here produces a <c>default</c> instance - <see cref="LeptosSiteConfig"/>
/// parsing is the sole source of these values.
/// </para>
/// </remarks>
internal readonly record struct SitePort
{
    /// <summary>The validated port number, always within 1-65535.</summary>
    public int Value { get; }

    /// <exception cref="InvalidOperationException">The value is outside 1-65535.</exception>
    public SitePort(int value)
    {
        if (value is < 1 or > 65535)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: site port must be within 1-65535; got {value}.");
        }

        Value = value;
    }

    /// <summary>The bare port number (no record type name), so the value can be embedded
    /// directly in addresses and environment variables.</summary>
    public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
}

/// <summary>
/// The TCP port cargo-leptos serves its hot-reload websocket on, from <c>reload-port</c>
/// in the workspace <c>Cargo.toml</c>. See <see cref="SitePort"/> for why the two ports
/// are distinct types with no implicit <see cref="int"/> conversions.
/// </summary>
internal readonly record struct ReloadPort
{
    /// <summary>The validated port number, always within 1-65535.</summary>
    public int Value { get; }

    /// <exception cref="InvalidOperationException">The value is outside 1-65535.</exception>
    public ReloadPort(int value)
    {
        if (value is < 1 or > 65535)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: reload port must be within 1-65535; got {value}.");
        }

        Value = value;
    }

    /// <summary>The bare port number (no record type name), so the value can be embedded
    /// directly in addresses and environment variables.</summary>
    public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
}

/// <summary>
/// The TCP port for the site server's auxiliary plain-http health listener, from
/// <c>health-port</c> in <c>[workspace.metadata.orchestrator]</c>. On TLS runs the
/// AppHost injects <c>HEALTH_ADDR</c> from this port and probes readiness there, so
/// the probe never has to trust the site's certificate. See <see cref="SitePort"/>
/// for the conventions the port wrapper types share.
/// </summary>
internal readonly record struct HealthPort
{
    /// <summary>
    /// Default, used when the workspace does not declare <c>health-port</c> - the
    /// next port after the site (4000) and reload (4001) defaults.
    /// </summary>
    public static HealthPort Default { get; } = new(4002);

    /// <summary>The validated port number, always within 1-65535.</summary>
    public int Value { get; }

    /// <exception cref="InvalidOperationException">The value is outside 1-65535.</exception>
    public HealthPort(int value)
    {
        if (value is < 1 or > 65535)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: health port must be within 1-65535; got {value}.");
        }

        Value = value;
    }

    /// <summary>The bare port number (no record type name), so the value can be embedded
    /// directly in addresses and environment variables.</summary>
    public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
}

/// <summary>
/// The readiness path the orchestrator probes, from <c>ready-path</c> in
/// <c>[workspace.metadata.orchestrator]</c> or from a caller-supplied override.
/// Always valid once constructed; see <see cref="SitePort"/> for the conventions
/// these wrapper types share.
/// </summary>
internal readonly record struct ReadyPath
{
    /// <summary>
    /// Default option, used when the workspace does not declare
    /// <c>ready-path</c>. The site's server (<c>site/server/src/main.rs</c>) registers
    /// the same path and pins it to the workspace metadata with a test, so the two
    /// sides cannot drift silently.
    /// </summary>
    public static ReadyPath Default { get; } = new("/readyz");

    /// <summary>The validated path, always starting with a slash.</summary>
    public string Value { get; }

    /// <exception cref="InvalidOperationException">The value is null, empty, or does not start with a slash.</exception>
    public ReadyPath(string value)
    {
        if (string.IsNullOrWhiteSpace(value) || !value.StartsWith('/'))
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: ready-path must be non-empty and start with '/'; got '{value}'.");
        }

        Value = value;
    }

    /// <summary>The bare path (no record type name), so the value can be embedded
    /// directly in URLs and environment variables.</summary>
    public override string ToString() => Value;
}

/// <summary>
/// The validated subset of the cargo-leptos workspace configuration that the orchestrator
/// consumes. Instances exist only after successful validation: <see cref="ReadFrom"/> and
/// <see cref="Parse"/> either return a fully-checked value or throw with a message naming
/// the offending key, so a malformed workspace fails at AppHost startup instead of
/// surfacing later as a resource that never starts or never turns healthy.
/// </summary>
internal sealed record LeptosSiteConfig
{
    /// <summary>Port from <c>site-addr</c>. The host half of <c>site-addr</c> is standalone-dev
    /// behavior; orchestrated runs override it (see the LEPTOS_SITE_ADDR injection).</summary>
    public SitePort SitePort { get; }

    /// <summary>Port from <c>reload-port</c> - the hot-reload websocket.</summary>
    public ReloadPort ReloadPort { get; }

    /// <summary>Readiness path the AppHost probes, from <c>[workspace.metadata.orchestrator]</c>
    /// <c>ready-path</c>; defaults to <see cref="ReadyPath.Default"/>.</summary>
    public ReadyPath ReadyPath { get; }

    /// <summary>Port for the auxiliary health listener, from
    /// <c>[workspace.metadata.orchestrator]</c> <c>health-port</c>; defaults to
    /// <see cref="HealthPort.Default"/>. Used on TLS runs only.</summary>
    public HealthPort HealthPort { get; }

    private LeptosSiteConfig(SitePort sitePort, ReloadPort reloadPort, ReadyPath readyPath, HealthPort healthPort)
    {
        if (sitePort.Value == reloadPort.Value
            || sitePort.Value == healthPort.Value
            || reloadPort.Value == healthPort.Value)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: site port {sitePort}, reload port {reloadPort}, and health port "
                + $"{healthPort} must be distinct.");
        }
        SitePort = sitePort;
        ReloadPort = reloadPort;
        ReadyPath = readyPath;
        HealthPort = healthPort;
    }

    /// <summary>Reads and validates the orchestrator-relevant config from a workspace
    /// <c>Cargo.toml</c> on disk.</summary>
    public static LeptosSiteConfig ReadFrom(string cargoTomlPath)
    {
        if (!File.Exists(cargoTomlPath))
        {
            throw new InvalidOperationException($"AddLeptosServerApp: Cargo.toml not found at '{cargoTomlPath}'.");
        }

        return Parse(File.ReadAllText(cargoTomlPath), cargoTomlPath);
    }

    /// <summary>
    /// Parses and validates the orchestrator-relevant config from TOML text. All
    /// validation lives here (rather than in <see cref="ReadFrom"/>) so it can run
    /// against in-memory documents as well as files.
    /// </summary>
    /// <param name="cargoToml">The workspace <c>Cargo.toml</c> content.</param>
    /// <param name="source">Where the content came from, for error messages.</param>
    public static LeptosSiteConfig Parse(string cargoToml, string source)
    {
        CargoToml cargo;
        try
        {
            cargo = TomlSerializer.Deserialize<CargoToml>(cargoToml)
                ?? throw new InvalidOperationException($"AddLeptosServerApp: failed to parse '{source}'.");
        }
        catch (TomlException ex)
        {
            throw new InvalidOperationException($"AddLeptosServerApp: failed to parse '{source}': {ex.Message}", ex);
        }

        // get the leptos data
        var leptos = cargo.Workspace?.Metadata?.Leptos is { Count: > 0 } sections
            ? sections[0]
            : throw new InvalidOperationException(
                $"AddLeptosServerApp: '[[workspace.metadata.leptos]]' section missing in '{source}'.");

        var reloadPort = leptos.ReloadPort
            ?? throw new InvalidOperationException(
                $"AddLeptosServerApp: 'reload-port' missing from [[workspace.metadata.leptos]] in '{source}'.");

        var orchestrator = cargo.Workspace?.Metadata?.Orchestrator;

        return new LeptosSiteConfig(
            ParseSitePort(leptos.SiteAddr, source),
            new ReloadPort(reloadPort),
            ParseReadyPath(orchestrator?.ReadyPath, source),
            orchestrator?.HealthPort is int healthPort ? new HealthPort(healthPort) : HealthPort.Default);
    }

    /// <summary>Extracts the validated port from a <c>site-addr</c> value.</summary>
    private static SitePort ParseSitePort(string? siteAddr, string source)
    {
        if (string.IsNullOrWhiteSpace(siteAddr))
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: 'site-addr' missing from [[workspace.metadata.leptos]] in '{source}'.");
        }

        // IPEndPoint.Parse handles both IPv4 (127.0.0.1:4000) and IPv6 ([::1]:4000)
        // literals and rejects hostnames - the same input domain as the Rust SocketAddr
        // that cargo-leptos parses site-addr into, so anything accepted here is also
        // valid for the site itself.
        IPEndPoint endpoint;
        try
        {
            endpoint = IPEndPoint.Parse(siteAddr);
        }
        catch (FormatException ex)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: could not parse site-addr '{siteAddr}' in '{source}' as IP:port "
                + "(hostnames are not valid here; cargo-leptos requires an IP literal such as 127.0.0.1:4000).",
                ex);
        }

        // Trap: IPEndPoint.Parse accepts a bare IP with no port and reports Port = 0
        // instead of throwing, so a missing port would otherwise slip through as 0.
        // (The SitePort constructor would also reject 0, as defense in depth; catching
        // it here gives the accurate "no port" message.)
        if (endpoint.Port == 0)
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: site-addr '{siteAddr}' in '{source}' must include a non-zero port.");
        }

        return new SitePort(endpoint.Port);
    }

    /// <summary>Validates an optional <c>ready-path</c> value, defaulting when absent.</summary>
    private static ReadyPath ParseReadyPath(string? readyPath, string source)
    {
        if (readyPath is null)
        {
            return ReadyPath.Default;
        }

        // A relative or blank path would make the health probe URL nonsense and leave
        // the resource permanently unhealthy with no clue why - fail loudly instead.
        // (The ReadyPath constructor would also reject this, as defense in depth;
        // checking here lets the message name the file the bad value came from.)
        if (string.IsNullOrWhiteSpace(readyPath) || !readyPath.StartsWith('/'))
        {
            throw new InvalidOperationException(
                $"AddLeptosServerApp: ready-path '{readyPath}' in '{source}' must start with '/'.");
        }

        return new ReadyPath(readyPath);
    }

    // Below: the Tomlyn deserialization targets. Tomlyn needs settable, nullable
    // properties to hold whatever the file happens to contain, so these DTOs stay
    // private and unvalidated; the outer record above is the only shape the rest of
    // the code ever sees. Unmapped Cargo.toml keys (package, dependencies, profile,
    // workspace.members, …) are ignored by Tomlyn.

    private sealed class CargoToml
    {
        [JsonPropertyName("workspace")] public CargoWorkspace? Workspace { get; set; }
    }

    private sealed class CargoWorkspace
    {
        [JsonPropertyName("metadata")] public CargoMetadata? Metadata { get; set; }
    }

    private sealed class CargoMetadata
    {
        [JsonPropertyName("leptos")] public List<LeptosMetadata>? Leptos { get; set; }
        [JsonPropertyName("orchestrator")] public OrchestratorMetadata? Orchestrator { get; set; }
    }

    /// <summary>Subset of <c>[[workspace.metadata.leptos]]</c> (cargo-leptos's own config)
    /// that the orchestrator reads. Later: output-name, site-root (drive the Containerfile
    /// COPY paths).</summary>
    private sealed class LeptosMetadata
    {
        [JsonPropertyName("site-addr")] public string? SiteAddr { get; set; }
        [JsonPropertyName("reload-port")] public int? ReloadPort { get; set; }
    }

    /// <summary>The <c>[workspace.metadata.orchestrator]</c> table - contract values this
    /// AppHost defines, kept in the workspace Cargo.toml so site and orchestrator share
    /// one source of truth.</summary>
    private sealed class OrchestratorMetadata
    {
        [JsonPropertyName("ready-path")] public string? ReadyPath { get; set; }
        [JsonPropertyName("health-port")] public int? HealthPort { get; set; }
    }
}
