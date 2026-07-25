using Aspire.Hosting;

namespace Portfolio.Orchestrator.Tests;

/// <summary>
/// Pins the constructor invariants of the port wrapper types: any instance that exists
/// was validated. (The other half of their contract - that a <see cref="SitePort"/> can't
/// be passed where a <see cref="ReloadPort"/> is expected - is enforced by the compiler
/// and therefore has no runtime test.)
/// </summary>
public class PortNewtypeTests
{
    [Theory]
    [InlineData(1)]
    [InlineData(4000)]
    [InlineData(65535)]
    public void SitePort_AcceptsValuesInRange(int value)
    {
        Assert.Equal(value, new SitePort(value).Value);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    [InlineData(65536)]
    public void SitePort_RejectsValuesOutOfRange(int value)
    {
        var ex = Assert.Throws<InvalidOperationException>(() => new SitePort(value));
        Assert.Contains("site port must be within 1-65535", ex.Message);
    }

    [Theory]
    [InlineData(1)]
    [InlineData(4001)]
    [InlineData(65535)]
    public void ReloadPort_AcceptsValuesInRange(int value)
    {
        Assert.Equal(value, new ReloadPort(value).Value);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    [InlineData(65536)]
    public void ReloadPort_RejectsValuesOutOfRange(int value)
    {
        var ex = Assert.Throws<InvalidOperationException>(() => new ReloadPort(value));
        Assert.Contains("reload port must be within 1-65535", ex.Message);
    }

    [Theory]
    [InlineData(1)]
    [InlineData(4002)]
    [InlineData(65535)]
    public void HealthPort_AcceptsValuesInRange(int value)
    {
        Assert.Equal(value, new HealthPort(value).Value);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    [InlineData(65536)]
    public void HealthPort_RejectsValuesOutOfRange(int value)
    {
        var ex = Assert.Throws<InvalidOperationException>(() => new HealthPort(value));
        Assert.Contains("health port must be within 1-65535", ex.Message);
    }

    // The ToString overrides feed environment variables and addresses, so the format is
    // load-bearing: it must be the bare number, not the record's default
    // "SitePort { Value = 4000 }" rendering.
    [Fact]
    public void SitePort_ToString_IsBareNumber()
    {
        Assert.Equal("4000", new SitePort(4000).ToString());
    }

    [Fact]
    public void ReloadPort_ToString_IsBareNumber()
    {
        Assert.Equal("4001", new ReloadPort(4001).ToString());
    }

    [Fact]
    public void HealthPort_ToString_IsBareNumber()
    {
        Assert.Equal("4002", new HealthPort(4002).ToString());
    }
}
