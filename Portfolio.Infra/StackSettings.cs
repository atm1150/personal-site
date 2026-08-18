using System.Text.RegularExpressions;

namespace Portfolio.Infra;

internal sealed record StackSettings(
    string Region,
    string DropletSize,
    string DropletImage,
    string DropletName,
    string DropletTag)
{
    internal const string ConfigNamespace = "portfolio-infra";

    internal const string KeyRegion = "region";
    internal const string KeyDropletSize = "dropletSize";
    internal const string KeyDropletImage = "dropletImage";
    internal const string KeyDropletName = "dropletName";
    internal const string KeyDropletTag = "dropletTag";

    // Keys other consumers read from the stack file (the DigitalOcean
    // provider, future tooling). Empty today; additions are deliberate.
    internal static readonly IReadOnlySet<string> AllowedForeignKeys =
        new HashSet<string>();

    // Matches DigitalOcean size slugs: letter-prefix, at least two dash-separated parts.
    // Examples: s-1vcpu-1gb, c-2, gd-8vcpu-32gb
    private static readonly Regex SizeSlugPattern = new(@"^[a-z]+-[a-z0-9].*-.*", RegexOptions.Compiled);

    internal static StackSettings Load(Func<string, string> require)
    {
        var region = Require(require, KeyRegion);
        var dropletSize = Require(require, KeyDropletSize);
        var dropletImage = Require(require, KeyDropletImage);
        var dropletName = Require(require, KeyDropletName);
        var dropletTag = Require(require, KeyDropletTag);

        if (!SizeSlugPattern.IsMatch(dropletSize))
            throw new InvalidOperationException(
                $"Stack config key '{KeyDropletSize}' has an invalid DigitalOcean size slug: '{dropletSize}'.");

        return new StackSettings(region, dropletSize, dropletImage, dropletName, dropletTag);
    }

    private static string Require(Func<string, string> require, string key)
    {
        var value = require(key);
        if (string.IsNullOrWhiteSpace(value))
            throw new InvalidOperationException(
                $"Stack config key '{key}' is required but is empty.");
        return value;
    }
}
