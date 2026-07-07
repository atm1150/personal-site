using Aspire.Hosting;

namespace Portfolio.Orchestrator.Tests;

/// <summary>
/// Pins the constructor invariant of <see cref="ReadyPath"/>: any instance that exists
/// holds an absolute path. Same contract shape as the port newtypes.
/// </summary>
public class ReadyPathTests
{
    [Theory]
    [InlineData("/readyz")]
    [InlineData("/healthz")]
    [InlineData("/api/ready")]
    public void ReadyPath_AcceptsAbsolutePaths(string value)
    {
        Assert.Equal(value, new ReadyPath(value).Value);
    }

    [Theory]
    [InlineData("readyz")]
    [InlineData("")]
    [InlineData("   ")]
    public void ReadyPath_RejectsRelativeOrBlankPaths(string value)
    {
        var ex = Assert.Throws<InvalidOperationException>(() => new ReadyPath(value));
        Assert.Contains("must be non-empty and start with '/'", ex.Message);
    }

    // The default is constructed through the validating constructor like any other
    // value, and its literal is the half of the cross-language contract that the
    // Rust side pins to the workspace metadata with its own test.
    [Fact]
    public void Default_IsReadyz()
    {
        Assert.Equal("/readyz", ReadyPath.Default.Value);
    }

    // ToString feeds URLs and log output, so it must be the bare path, not the
    // record's default "ReadyPath { Value = /readyz }" rendering.
    [Fact]
    public void ReadyPath_ToString_IsBarePath()
    {
        Assert.Equal("/readyz", new ReadyPath("/readyz").ToString());
    }
}
