using System.Diagnostics;

namespace Aspire.Hosting;

/// <summary>
/// A cargo-leptos SSR server application.
/// </summary>
/// <param name="name">The name of the resource.</param>
/// <param name="workingDirectory">The cargo-leptos workspace directory the command runs in.</param>
public class LeptosServerResource(string name, string workingDirectory)
    : ExecutableResource(name, CargoCommand, workingDirectory), IResourceWithServiceDiscovery
{
    /// <summary>The executable this resource spawns. Shared with the toolchain preflight in
    /// <see cref="LeptosHostingExtensions"/> so the guard always probes the command
    /// actually run. (Named distinctly from the inherited <c>ExecutableResource.Command</c>
    /// property, which reports the same value on instances.)</summary>
    internal const string CargoCommand = "cargo";
}

/// <summary>
/// Extension methods for hosting cargo-leptos applications.
/// </summary>
public static class LeptosHostingExtensions
{
    // WithHttpEndpoint names its endpoint after the scheme ("http") when no name is
    // given; GetEndpoint later looks the endpoint up by that name. Naming it explicitly
    // in both places makes the coupling visible instead of relying on the default.
    private const string HttpEndpointName = "http";
    private const string HttpsEndpointName = "https";
    private const string ReloadEndpointName = "reload";
    private const string HealthEndpointName = "health";

    /// <summary>Escape hatch: <c>SITE_TLS=off</c> in the AppHost's environment restores
    /// the plain-http wiring (site served over http, readiness probed on the site
    /// endpoint, no certificate or health-listener env injected).</summary>
    private const string TlsSwitchEnvVar = "SITE_TLS";

    /// <summary>
    /// Adds a Leptos SSR server application to the distributed application, run via <c>cargo leptos watch</c>.
    /// </summary>
    /// <param name="builder">The distributed application builder.</param>
    /// <param name="name">The name of the resource.</param>
    /// <param name="workingDirectory">The cargo-leptos workspace directory, relative to the AppHost directory. Its <c>Cargo.toml</c> is the source of truth for the site and reload ports.</param>
    /// <param name="healthPath">The readiness path Aspire probes; must start with <c>/</c>.
    /// Defaults to the workspace's <c>[workspace.metadata.orchestrator]</c> <c>ready-path</c>
    /// (itself defaulting to <c>/readyz</c>, the route registered in <c>site/server/src/main.rs</c>).</param>
    /// <param name="args">Additional arguments appended after <c>leptos watch</c> (e.g. <c>--release</c>).</param>
    public static IResourceBuilder<LeptosServerResource> AddLeptosServerApp(
        this IDistributedApplicationBuilder builder,
        [ResourceName] string name,
        string workingDirectory,
        string? healthPath = null,
        string[]? args = null)
    {
        ArgumentNullException.ThrowIfNull(builder);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentException.ThrowIfNullOrWhiteSpace(workingDirectory);

        workingDirectory = Path.GetFullPath(Path.Combine(builder.AppHostDirectory, workingDirectory));

        // The cargo-leptos workspace is the single source of truth for the site's
        // ports and readiness path: read them from Cargo.toml so orchestrated and
        // standalone runs cannot diverge.
        var config = LeptosSiteConfig.ReadFrom(Path.Combine(workingDirectory, "Cargo.toml"));

        // Caller overrides pass through the same validating type as TOML-sourced values,
        // so every path the probe can use is a ReadyPath - there is no raw-string route
        // to an invalid probe URL.
        var readyPath = healthPath is null ? config.ReadyPath : new ReadyPath(healthPath);

        var resource = new LeptosServerResource(name, workingDirectory);
        var resourceBuilder = builder.AddResource(resource);

        if (builder.ExecutionContext.IsRunMode)
        {
            // Fail early with one message naming every missing prerequisite (and its
            // fix) rather than letting the first gap surface as a confusing spawn
            // error or a mid-build failure in the resource logs.
            RunToolchainPreflight(RunModePreflight);

            // TLS is the run-mode default so the everyday stack proves the https code
            // path; SITE_TLS=off restores the previous plain-http wiring (and with it
            // hot reload from non-localhost clients, which mixed-content rules would
            // otherwise block).
            var serveTls = !string.Equals(
                Environment.GetEnvironmentVariable(TlsSwitchEnvVar), "off", StringComparison.OrdinalIgnoreCase);

            resourceBuilder
                .WithArgs(["leptos", "watch", .. args ?? []])
                // Declare the hot-reload websocket so Aspire's allocator accounts
                // for the port. Dev-only, gated out of any published artifact.
                .WithEndpoint(name: ReloadEndpointName, port: config.ReloadPort.Value, isProxied: false)
                .WithOtlpExporter();

            if (serveTls)
            {
                // The certificate APIs are experimental (ASPIRECERTIFICATES001) and may
                // change shape in a future Aspire release; this is a dev-only code path,
                // so tracking that churn is acceptable.
#pragma warning disable ASPIRECERTIFICATES001
                resourceBuilder
                    // Unproxied so the cargo-leptos process owns the port itself; the
                    // LEPTOS_SITE_ADDR override below binds it to all interfaces.
                    .WithHttpsEndpoint(port: config.SitePort.Value, name: HttpsEndpointName, isProxied: false)
                    .WithExternalHttpEndpoints()
                    // The auxiliary plain-http /readyz listener (HEALTH_ADDR below): the
                    // probe goes there so it never has to trust the dev certificate.
                    .WithHttpEndpoint(port: config.HealthPort.Value, name: HealthEndpointName, isProxied: false)
                    .WithHttpHealthCheck(readyPath.Value, endpointName: HealthEndpointName)
                    // The server terminates TLS itself, from PEM paths in env. Aspire
                    // materializes the ASP.NET Core developer certificate as PEM files
                    // and this callback maps their paths onto the server's env contract.
                    .WithHttpsDeveloperCertificate()
                    .WithHttpsCertificateConfiguration(ctx =>
                    {
                        ctx.EnvironmentVariables["TLS_CERT_PATH"] = ctx.CertificatePath;
                        ctx.EnvironmentVariables["TLS_KEY_PATH"] = ctx.KeyPath;
                        return Task.CompletedTask;
                    });
#pragma warning restore ASPIRECERTIFICATES001
            }
            else
            {
                resourceBuilder
                    .WithHttpEndpoint(port: config.SitePort.Value, name: HttpEndpointName, isProxied: false)
                    .WithExternalHttpEndpoints()
                    .WithHttpHealthCheck(readyPath.Value);
            }

            resourceBuilder.WithEnvironment(context =>
            {
                // Sourcing the values from the declared endpoints (rather than the
                // parsed config) keeps a single chain of truth: Cargo.toml -> endpoint
                // annotation -> environment variable.
                var site = resource.GetEndpoint(serveTls ? HttpsEndpointName : HttpEndpointName);
                var reload = resource.GetEndpoint(ReloadEndpointName);
                context.EnvironmentVariables["LEPTOS_SITE_ADDR"] =
                    ReferenceExpression.Create($"0.0.0.0:{site.Property(EndpointProperty.Port)}");
                context.EnvironmentVariables["LEPTOS_RELOAD_PORT"] =
                    ReferenceExpression.Create($"{reload.Property(EndpointProperty.Port)}");
                if (serveTls)
                {
                    // Loopback-only: the probe runs on this host, and nothing else
                    // should reach the unauthenticated plain-http surface.
                    var health = resource.GetEndpoint(HealthEndpointName);
                    context.EnvironmentVariables["HEALTH_ADDR"] =
                        ReferenceExpression.Create($"127.0.0.1:{health.Property(EndpointProperty.Port)}");
                }
            });
        }
        else
        {
            // Publish mode (containerized release build) is not implemented yet; fail
            // loudly rather than emit a manifest with a half-configured resource.
            throw new NotSupportedException("AddLeptosServerApp does not support publish mode yet.");
        }

        return resourceBuilder;
    }

    /// <summary>
    /// One dev-environment prerequisite: a probe command whose success (and optionally
    /// whose output) proves the tool is usable, plus the command that fixes it.
    /// </summary>
    /// <param name="Description">What is being checked, as shown in failure output.</param>
    /// <param name="FileName">The probe executable.</param>
    /// <param name="Args">Arguments for the probe.</param>
    /// <param name="Remedy">The command a developer runs to satisfy the check.</param>
    /// <param name="OutputPredicate">Optional test applied to the probe's stdout; when null,
    /// a zero exit code alone passes the check.</param>
    /// <param name="WarnWhenProbeUnavailable">When the probe executable itself is a different
    /// tool than the subject (e.g. probing targets via rustup), its absence downgrades the
    /// check to a console warning instead of a failure: the subject may still be fine.</param>
    private sealed record ToolchainCheck(
        string Description,
        string FileName,
        string[] Args,
        string Remedy,
        Func<string, bool>? OutputPredicate = null,
        bool WarnWhenProbeUnavailable = false);

    // Admission rule for this list: a check earns its slot only when the failure it
    // converts is (a) common on a fresh dev machine, (b) confusing when it surfaces
    // downstream, and (c) cheap to detect here. Project-hygiene gates (cargo audit,
    // clippy, fmt) belong to CI or their own dashboard resources, not to startup.
    private static readonly ToolchainCheck[] RunModePreflight =
    [
        new("cargo (Rust toolchain)",
            LeptosServerResource.CargoCommand, ["--version"],
            Remedy: "install rustup (https://rustup.rs)"),
        new("cargo-leptos",
            LeptosServerResource.CargoCommand, ["leptos", "--version"],
            Remedy: "cargo install cargo-leptos --locked"),
        new("wasm32-unknown-unknown compilation target",
            "rustup", ["target", "list", "--installed"],
            Remedy: "rustup target add wasm32-unknown-unknown",
            OutputPredicate: static stdout => stdout.Contains("wasm32-unknown-unknown", StringComparison.Ordinal),
            // cargo can exist without rustup (distro packages); the target may still be
            // installed by other means, so an unprobeable check must not block startup.
            WarnWhenProbeUnavailable: true),
        // The server's tls-rustls feature pulls in aws-lc-sys, whose build script
        // compiles C. A C compiler lives outside rustup, so a fresh machine can
        // pass every Rust check above and still fail mid-build with a raw cc
        // error. (cmake is deliberately not checked: aws-lc-sys prefers it but
        // falls back to a bundled cc-only builder on mainstream targets - this
        // workspace builds without cmake installed.)
        new("cc (C compiler; aws-lc-sys compiles C for rustls TLS)",
            "cc", ["--version"],
            Remedy: "install a C toolchain (Debian/Ubuntu: apt install build-essential)"),
    ];

    /// <summary>
    /// Runs every check and throws a single exception listing all failures with their
    /// remedies, so a fresh machine is fixed in one pass instead of one failure per run.
    /// </summary>
    private static void RunToolchainPreflight(ToolchainCheck[] checks)
    {
        var failures = new List<string>();

        foreach (var check in checks)
        {
            var startInfo = new ProcessStartInfo(check.FileName)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            };
            foreach (var arg in check.Args)
            {
                startInfo.ArgumentList.Add(arg);
            }

            string stdout;
            string stderr;
            try
            {
                using var process = Process.Start(startInfo)
                    ?? throw new InvalidOperationException($"probe '{check.FileName}' did not start");
                stdout = process.StandardOutput.ReadToEnd();
                stderr = process.StandardError.ReadToEnd();
                if (!process.WaitForExit(15_000))
                {
                    process.Kill(entireProcessTree: true);
                    failures.Add($"{check.Description}: probe timed out. Fix: {check.Remedy}");
                    continue;
                }
                if (process.ExitCode != 0)
                {
                    var detail = stderr.Split('\n', StringSplitOptions.RemoveEmptyEntries) is [var first, ..]
                        ? $" ({first.Trim()})"
                        : "";
                    failures.Add($"{check.Description}: probe failed{detail}. Fix: {check.Remedy}");
                    continue;
                }
            }
            catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException)
            {
                if (check.WarnWhenProbeUnavailable)
                {
                    Console.WriteLine(
                        $"AddLeptosServerApp: skipped preflight check '{check.Description}': '{check.FileName}' is not available to probe with.");
                    continue;
                }
                failures.Add($"{check.Description}: '{check.FileName}' could not be run. Fix: {check.Remedy}");
                continue;
            }

            if (check.OutputPredicate is not null && !check.OutputPredicate(stdout))
            {
                failures.Add($"{check.Description}: not installed. Fix: {check.Remedy}");
            }
        }

        if (failures.Count > 0)
        {
            throw new InvalidOperationException(
                "AddLeptosServerApp: the dev environment is missing prerequisites:\n  - "
                + string.Join("\n  - ", failures));
        }
    }
}
