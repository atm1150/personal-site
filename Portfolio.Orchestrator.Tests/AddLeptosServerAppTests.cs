using Aspire.Hosting;

namespace Portfolio.Orchestrator.Tests;

/// <summary>
/// The one wiring-level behavior worth a unit test: publish mode must refuse loudly.
/// (The run-mode happy path - endpoints, env injection, health check - is deliberately
/// not unit-tested; it is exercised for real by running the orchestrated stack, and
/// annotation-by-annotation assertions would only mirror the implementation.)
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
}
