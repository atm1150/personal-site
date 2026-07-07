using Aspire.Hosting;

namespace Portfolio.Orchestrator.Tests;

/// <summary>
/// Exercises <see cref="LeptosSiteConfig.Parse"/> - the validation boundary between the
/// cargo-leptos workspace's Cargo.toml and the orchestrator. Everything here feeds the
/// same guarantee: a <see cref="LeptosSiteConfig"/> instance either reflects a
/// well-formed workspace or never comes into existence.
/// </summary>
public class LeptosSiteConfigTests
{
    private const string Source = "<test>";

    /// <summary>A structurally complete workspace document the individual tests mutate.</summary>
    private static string Toml(
        string leptosSection = "[[workspace.metadata.leptos]]\nsite-addr = \"127.0.0.1:4000\"\nreload-port = 4001",
        string orchestratorSection = "")
        => $"""
            [workspace]
            members = ["app", "server"]

            {leptosSection}

            {orchestratorSection}
            """;

    [Fact]
    public void Parse_ValidWorkspace_YieldsPortsAndDefaultReadyPath()
    {
        var config = LeptosSiteConfig.Parse(Toml(), Source);

        Assert.Equal(4000, config.SitePort.Value);
        Assert.Equal(4001, config.ReloadPort.Value);
        Assert.Equal("/readyz", config.ReadyPath.Value);
    }

    [Fact]
    public void Parse_Ipv6SiteAddr_YieldsPort()
    {
        var config = LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"[::1]:4000\"\nreload-port = 4001"), Source);

        Assert.Equal(4000, config.SitePort.Value);
    }

    // The IPEndPoint.Parse trap: a bare IP with no port parses *successfully* with
    // Port = 0 instead of throwing. This test is what guarantees a missing port can
    // never slip through as port 0.
    [Fact]
    public void Parse_SiteAddrWithoutPort_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"127.0.0.1\"\nreload-port = 4001"), Source));

        Assert.Contains("must include a non-zero port", ex.Message);
    }

    // cargo-leptos parses site-addr into a Rust SocketAddr, which accepts IP literals
    // only - so the orchestrator must reject hostnames too, or it would accept a value
    // the site itself cannot start with.
    [Fact]
    public void Parse_HostnameSiteAddr_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"localhost:4000\"\nreload-port = 4001"), Source));

        Assert.Contains("could not parse site-addr 'localhost:4000'", ex.Message);
    }

    [Fact]
    public void Parse_GarbageSiteAddr_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"not an address\"\nreload-port = 4001"), Source));

        Assert.Contains("could not parse site-addr", ex.Message);
    }

    [Fact]
    public void Parse_MissingLeptosSection_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => LeptosSiteConfig.Parse(Toml(leptosSection: ""), Source));

        Assert.Contains("'[[workspace.metadata.leptos]]' section missing", ex.Message);
    }

    [Fact]
    public void Parse_MissingSiteAddr_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nreload-port = 4001"), Source));

        Assert.Contains("'site-addr' missing", ex.Message);
    }

    [Fact]
    public void Parse_MissingReloadPort_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"127.0.0.1:4000\""), Source));

        Assert.Contains("'reload-port' missing", ex.Message);
    }

    // Out-of-range values reach the newtype constructor and die there - the config
    // record can only ever hold validated ports.
    [Fact]
    public void Parse_ReloadPortOutOfRange_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"127.0.0.1:4000\"\nreload-port = 70000"), Source));

        Assert.Contains("reload port must be within 1-65535", ex.Message);
    }

    // Cross-field invariant: the two ports are individually valid but can't collide,
    // or the reload websocket would fail to bind at runtime with no hint why.
    [Fact]
    public void Parse_SiteAndReloadPortEqual_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            leptosSection: "[[workspace.metadata.leptos]]\nsite-addr = \"127.0.0.1:4000\"\nreload-port = 4000"), Source));

        Assert.Contains("must be distinct", ex.Message);
    }

    [Fact]
    public void Parse_UnparseableToml_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => LeptosSiteConfig.Parse("= this is not toml =", Source));

        Assert.Contains("failed to parse", ex.Message);
    }

    [Fact]
    public void Parse_DeclaredReadyPath_IsUsed()
    {
        var config = LeptosSiteConfig.Parse(Toml(
            orchestratorSection: "[workspace.metadata.orchestrator]\nready-path = \"/healthz\""), Source);

        Assert.Equal("/healthz", config.ReadyPath.Value);
    }

    // A relative ready-path would produce a nonsense probe URL and a resource that
    // sits unhealthy forever with no error - reject it at startup instead.
    [Fact]
    public void Parse_ReadyPathWithoutLeadingSlash_Throws()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.Parse(Toml(
            orchestratorSection: "[workspace.metadata.orchestrator]\nready-path = \"healthz\""), Source));

        Assert.Contains("must start with '/'", ex.Message);
    }

    [Fact]
    public void ReadFrom_MissingFile_Throws()
    {
        var missing = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName(), "Cargo.toml");

        var ex = Assert.Throws<InvalidOperationException>(() => LeptosSiteConfig.ReadFrom(missing));

        Assert.Contains("Cargo.toml not found", ex.Message);
    }
}
