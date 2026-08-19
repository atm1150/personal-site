using Portfolio.Infra;

namespace Portfolio.Infra.Tests;

/// <summary>
/// Tests for <see cref="InfraConfig"/> - pure env-var reading and validation,
/// no Pulumi machinery involved.
/// </summary>
public class InfraConfigTests : IDisposable
{
    // Stash and restore env vars so tests don't bleed into each other.
    private readonly string? _savedAdminIp;
    private readonly string? _savedSshKey;
    private readonly string? _savedDeploySshKey;

    public InfraConfigTests()
    {
        _savedAdminIp = Environment.GetEnvironmentVariable(InfraConfig.AdminIpVar);
        _savedSshKey = Environment.GetEnvironmentVariable(InfraConfig.SshPublicKeyVar);
        _savedDeploySshKey = Environment.GetEnvironmentVariable(InfraConfig.DeploySshPublicKeyVar);
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, null);
        Environment.SetEnvironmentVariable(InfraConfig.SshPublicKeyVar, null);
        Environment.SetEnvironmentVariable(InfraConfig.DeploySshPublicKeyVar, null);
    }

    public void Dispose()
    {
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, _savedAdminIp);
        Environment.SetEnvironmentVariable(InfraConfig.SshPublicKeyVar, _savedSshKey);
        Environment.SetEnvironmentVariable(InfraConfig.DeploySshPublicKeyVar, _savedDeploySshKey);
    }

    // --- RequireAdminIp ---

    [Fact]
    public void RequireAdminIp_WhenUnset_ThrowsNamingVariable()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireAdminIp());

        Assert.Contains(InfraConfig.AdminIpVar, ex.Message);
    }

    [Fact]
    public void RequireAdminIp_WhenEmpty_ThrowsNamingVariable()
    {
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, "");

        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireAdminIp());

        Assert.Contains(InfraConfig.AdminIpVar, ex.Message);
    }

    [Fact]
    public void RequireAdminIp_WhenNotAnIpAddress_ThrowsNamingVariable()
    {
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, "not-an-ip");

        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireAdminIp());

        Assert.Contains(InfraConfig.AdminIpVar, ex.Message);
    }

    [Fact]
    public void RequireAdminIp_WhenIpv6_ThrowsNamingVariable()
    {
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, "2001:db8::1");

        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireAdminIp());

        Assert.Contains(InfraConfig.AdminIpVar, ex.Message);
    }

    [Fact]
    public void RequireAdminIp_WhenValidIpv4_ReturnsAddress()
    {
        Environment.SetEnvironmentVariable(InfraConfig.AdminIpVar, "192.0.2.1");

        var result = InfraConfig.RequireAdminIp();

        Assert.Equal("192.0.2.1", result);
    }

    // --- RequireSshPublicKey ---

    [Fact]
    public void RequireSshPublicKey_WhenUnset_ThrowsNamingVariable()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireSshPublicKey());

        Assert.Contains(InfraConfig.SshPublicKeyVar, ex.Message);
    }

    [Fact]
    public void RequireSshPublicKey_WhenEmpty_ThrowsNamingVariable()
    {
        Environment.SetEnvironmentVariable(InfraConfig.SshPublicKeyVar, "");

        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireSshPublicKey());

        Assert.Contains(InfraConfig.SshPublicKeyVar, ex.Message);
    }

    [Fact]
    public void RequireSshPublicKey_WhenSet_ReturnsValue()
    {
        var key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITest key-comment";
        Environment.SetEnvironmentVariable(InfraConfig.SshPublicKeyVar, key);

        var result = InfraConfig.RequireSshPublicKey();

        Assert.Equal(key, result);
    }

    // --- RequireDeploySshPublicKey ---

    [Fact]
    public void RequireDeploySshPublicKey_WhenUnset_ThrowsNamingVariable()
    {
        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireDeploySshPublicKey());

        Assert.Contains(InfraConfig.DeploySshPublicKeyVar, ex.Message);
    }

    [Fact]
    public void RequireDeploySshPublicKey_WhenEmpty_ThrowsNamingVariable()
    {
        Environment.SetEnvironmentVariable(InfraConfig.DeploySshPublicKeyVar, "");

        var ex = Assert.Throws<InvalidOperationException>(() => InfraConfig.RequireDeploySshPublicKey());

        Assert.Contains(InfraConfig.DeploySshPublicKeyVar, ex.Message);
    }

    [Fact]
    public void RequireDeploySshPublicKey_WhenSet_ReturnsValue()
    {
        var key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDeploy ci-deploy";
        Environment.SetEnvironmentVariable(InfraConfig.DeploySshPublicKeyVar, key);

        var result = InfraConfig.RequireDeploySshPublicKey();

        Assert.Equal(key, result);
    }
}
