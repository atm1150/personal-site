using Aspire.Hosting;

namespace Portfolio.Orchestrator.Tests;

/// <summary>
/// The one wiring-level behavior worth a unit test: publish mode must refuse loudly.
/// (The run-mode happy path - endpoints, env injection, health check - is deliberately
/// not unit-tested; it is exercised for real by running the orchestrated stack, and
/// annotation-by-annotation assertions would only mirror the implementation.)
///
/// Run mode itself is never constructed here: <c>DistributedApplication.CreateBuilder([])</c>
/// defaults to run mode, and <see cref="LeptosHostingExtensions.AddLeptosServerApp"/> runs
/// its toolchain preflight (cargo, cargo-leptos, rustup, cc) only in that mode - a CI
/// container with no Rust toolchain would fail every such test before it reached the
/// behavior under test. The SITE_TLS precedence decision is instead tested directly
/// against <see cref="LeptosHostingExtensions.ResolveServeTls"/>, the pure function the
/// run-mode branch calls: no builder, no toolchain, no environment access.
///
/// Coverage gap accepted alongside this: no automated test asserts that the resolved
/// value is actually injected into the site resource's environment (that would require
/// constructing run mode, which is exactly what the preflight makes untestable here).
/// That wiring is exercised manually by running the orchestrated stack.
/// </summary>
public class AddLeptosServerAppTests
{
    /// <summary>
    /// Without the guard, <c>aspire publish</c> would emit a manifest containing a bare
    /// <c>cargo</c> executable with no args or endpoints - a broken artifact discovered
    /// at deploy time instead of at publish time.
    /// </summary>
    [Fact]
    public void PublishMode_Throws()
    {
        // Config is read (and must be readable) before the mode branch, so the fixture
        // needs a well-formed workspace Cargo.toml on disk.
        var workspace = Directory.CreateTempSubdirectory("leptos-publish-test-");
        try
        {
            File.WriteAllText(Path.Combine(workspace.FullName, "Cargo.toml"),
                """
                [workspace]

                [[workspace.metadata.leptos]]
                site-addr = "127.0.0.1:4000"
                reload-port = 4001
                """);

            // "--operation publish" is the switch the aspire CLI itself passes to put an
            // AppHost into publish mode.
            var builder = DistributedApplication.CreateBuilder(["--operation", "publish"]);

            // workingDirectory is absolute here, so it survives the AppHostDirectory
            // combine unchanged regardless of where the test host runs.
            var ex = Assert.Throws<NotSupportedException>(
                () => builder.AddLeptosServerApp("site", workspace.FullName));

            Assert.Contains("does not support publish mode", ex.Message);
        }
        finally
        {
            workspace.Delete(recursive: true);
        }
    }

    /// <summary>
    /// A caller-supplied healthPath goes through the same validating ReadyPath type as
    /// TOML-sourced values. Validation happens before the run/publish branch, so a
    /// publish-mode builder proves it without needing cargo on the test host: the
    /// invalid path must throw before the publish-mode NotSupportedException would.
    /// </summary>
    [Fact]
    public void InvalidHealthPathOverride_Throws()
    {
        var workspace = Directory.CreateTempSubdirectory("leptos-healthpath-test-");
        try
        {
            File.WriteAllText(Path.Combine(workspace.FullName, "Cargo.toml"),
                """
                [workspace]

                [[workspace.metadata.leptos]]
                site-addr = "127.0.0.1:4000"
                reload-port = 4001
                """);

            var builder = DistributedApplication.CreateBuilder(["--operation", "publish"]);

            var ex = Assert.Throws<InvalidOperationException>(
                () => builder.AddLeptosServerApp("site", workspace.FullName, healthPath: "no-leading-slash"));

            Assert.Contains("must be non-empty and start with '/'", ex.Message);
        }
        finally
        {
            workspace.Delete(recursive: true);
        }
    }

    /// <summary>
    /// Null and empty are one equivalence class - undeclared - so both fall through to
    /// the workspace default: true means "serve TLS". An empty value is not a
    /// declaration and must not shadow the default with some other outcome.
    /// </summary>
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    public void ResolveServeTls_AmbientUndeclared_WorkspaceTrue_ReturnsTrue(string? ambientSiteTls)
    {
        Assert.True(LeptosHostingExtensions.ResolveServeTls(ambientSiteTls, workspaceDefault: true));
    }

    /// <summary>
    /// The undeclared fall-through with a workspace default of false - the ambient
    /// escape hatch is not the only way to turn TLS off.
    /// </summary>
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    public void ResolveServeTls_AmbientUndeclared_WorkspaceFalse_ReturnsFalse(string? ambientSiteTls)
    {
        Assert.False(LeptosHostingExtensions.ResolveServeTls(ambientSiteTls, workspaceDefault: false));
    }

    /// <summary>
    /// An ambient "off" must win over the workspace default - this is the direction that
    /// actually exercises the precedence chain: it goes red if the chain were inverted
    /// (workspace winning over ambient) or if the ambient branch were deleted so the
    /// function just returned <paramref name="workspaceDefault"/> unconditionally.
    /// </summary>
    [Fact]
    public void ResolveServeTls_AmbientOff_WorkspaceTrue_ReturnsFalse()
    {
        Assert.False(LeptosHostingExtensions.ResolveServeTls("off", workspaceDefault: true));
    }

    /// <summary>
    /// The other direction of the same override: ambient "on" must win even when the
    /// workspace declares false, so neither arm of the ambient branch can be silently
    /// dropped.
    /// </summary>
    [Fact]
    public void ResolveServeTls_AmbientOn_WorkspaceFalse_ReturnsTrue()
    {
        Assert.True(LeptosHostingExtensions.ResolveServeTls("on", workspaceDefault: false));
    }

    /// <summary>
    /// The accepted vocabulary is case-insensitive in both directions - matching the
    /// server's own case-insensitive parsing of the same variable.
    /// </summary>
    [Theory]
    [InlineData("ON")]
    [InlineData("On")]
    public void ResolveServeTls_AmbientOnUppercase_ReturnsTrue(string ambient)
    {
        Assert.True(LeptosHostingExtensions.ResolveServeTls(ambient, workspaceDefault: false));
    }

    /// <summary>
    /// Case-insensitivity applies to the "off" arm too, not only "on".
    /// </summary>
    [Theory]
    [InlineData("OFF")]
    [InlineData("Off")]
    public void ResolveServeTls_AmbientOffUppercase_ReturnsFalse(string ambient)
    {
        Assert.False(LeptosHostingExtensions.ResolveServeTls(ambient, workspaceDefault: true));
    }

    /// <summary>
    /// Anything outside the "on"/"off" vocabulary must be rejected rather than silently
    /// reinterpreted - the AppHost previously treated any non-"off" value as "on", so
    /// SITE_TLS=false disagreed with the server (which rejects it outright) about what it
    /// meant. One documented vocabulary, one acceptance rule.
    /// </summary>
    [Theory]
    [InlineData("false")]
    [InlineData("0")]
    [InlineData("no")]
    [InlineData("disabled")]
    public void ResolveServeTls_AmbientOutsideVocabulary_Throws(string ambient)
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => LeptosHostingExtensions.ResolveServeTls(ambient, workspaceDefault: false));

        Assert.Contains(ambient, ex.Message);
        Assert.Contains("'on' or 'off'", ex.Message);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void UsesAmbientPublicBaseUrl_Undeclared_ReturnsFalse(string? ambient)
    {
        Assert.False(LeptosHostingExtensions.UsesAmbientPublicBaseUrl(ambient));
    }

    [Fact]
    public void UsesAmbientPublicBaseUrl_Declared_ReturnsTrue()
    {
        Assert.True(LeptosHostingExtensions.UsesAmbientPublicBaseUrl("https://example.test"));
    }
}
