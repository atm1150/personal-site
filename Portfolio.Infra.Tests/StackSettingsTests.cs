using Portfolio.Infra;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace Portfolio.Infra.Tests;

public class StackSettingsTests
{
    private static Dictionary<string, string> AllValidValues() => new()
    {
        [StackSettings.KeyRegion] = "sfo3",
        [StackSettings.KeyDropletSize] = "s-1vcpu-1gb",
        [StackSettings.KeyDropletImage] = "debian-13-x64",
        [StackSettings.KeyDropletName] = "portfolio-site",
        [StackSettings.KeyDropletTag] = "portfolio",
    };

    private static StackSettings LoadFrom(Dictionary<string, string> values) =>
        StackSettings.Load(key => values[key]);

    // --- happy path ---

    [Fact]
    public void Load_AllKeysValid_PopulatesRecord()
    {
        var settings = LoadFrom(AllValidValues());

        Assert.Equal("sfo3", settings.Region);
        Assert.Equal("s-1vcpu-1gb", settings.DropletSize);
        Assert.Equal("debian-13-x64", settings.DropletImage);
        Assert.Equal("portfolio-site", settings.DropletName);
        Assert.Equal("portfolio", settings.DropletTag);
    }

    // --- missing key ---

    [Theory]
    [InlineData(StackSettings.KeyRegion)]
    [InlineData(StackSettings.KeyDropletSize)]
    [InlineData(StackSettings.KeyDropletImage)]
    [InlineData(StackSettings.KeyDropletName)]
    [InlineData(StackSettings.KeyDropletTag)]
    public void Load_MissingKey_ThrowsNamingKey(string missingKey)
    {
        var values = AllValidValues();
        values.Remove(missingKey);

        var ex = Assert.ThrowsAny<Exception>(() => StackSettings.Load(key => values[key]));

        // Either a KeyNotFoundException from the dict or an InvalidOperationException from Load.
        Assert.True(
            ex is KeyNotFoundException || (ex is InvalidOperationException && ex.Message.Contains(missingKey)),
            $"Expected exception naming '{missingKey}', got: {ex}");
    }

    // --- empty value ---

    [Theory]
    [InlineData(StackSettings.KeyRegion)]
    [InlineData(StackSettings.KeyDropletSize)]
    [InlineData(StackSettings.KeyDropletImage)]
    [InlineData(StackSettings.KeyDropletName)]
    [InlineData(StackSettings.KeyDropletTag)]
    public void Load_EmptyValue_ThrowsNamingKey(string emptyKey)
    {
        var values = AllValidValues();
        values[emptyKey] = "";

        var ex = Assert.Throws<InvalidOperationException>(() => StackSettings.Load(key => values[key]));

        Assert.Contains(emptyKey, ex.Message);
    }

    // --- size slug validation ---

    [Theory]
    [InlineData("notaslug")]
    [InlineData("s-only")]
    [InlineData("noletterprefix")]
    [InlineData("1-bad-start")]
    public void Load_MalformedSizeSlug_ThrowsNamingKey(string badSlug)
    {
        var values = AllValidValues();
        values[StackSettings.KeyDropletSize] = badSlug;

        var ex = Assert.Throws<InvalidOperationException>(() => StackSettings.Load(key => values[key]));

        Assert.Contains(StackSettings.KeyDropletSize, ex.Message);
    }

    // --- yaml contract ---

    private static string LocateProdYaml()
    {
        // Test assembly: Portfolio.Infra.Tests/bin/<config>/net10.0/
        // Walk up 4 levels to reach the solution root, then into Portfolio.Infra/.
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        for (var i = 0; i < 4; i++)
            dir = dir.Parent ?? throw new InvalidOperationException(
                $"Ran out of parent directories walking up from {AppContext.BaseDirectory}");

        var path = Path.Combine(dir.FullName, "Portfolio.Infra", "Pulumi.prod.yaml");
        if (!File.Exists(path))
            throw new FileNotFoundException(
                $"Pulumi.prod.yaml not found at expected path: {path}");
        return path;
    }

    private static Dictionary<string, string> LoadYamlConfigKeys()
    {
        var yaml = File.ReadAllText(LocateProdYaml());
        var deserializer = new DeserializerBuilder()
            .WithNamingConvention(NullNamingConvention.Instance)
            .Build();

        // Pulumi owns top-level scalar entries (e.g. encryptionsalt); only the config mapping is under contract.
        var root = deserializer.Deserialize<Dictionary<string, object>>(yaml);
        if (!root.TryGetValue("config", out var section) || section is not IDictionary<object, object> config)
            return new Dictionary<string, string>();

        return config.ToDictionary(kv => (string)kv.Key, kv => kv.Value?.ToString() ?? string.Empty);
    }

    [Fact]
    public void ProdYaml_ContainsAllRequiredKeys()
    {
        var yamlKeys = LoadYamlConfigKeys();
        var ns = StackSettings.ConfigNamespace + ":";

        var requiredKeys = new[]
        {
            StackSettings.KeyRegion,
            StackSettings.KeyDropletSize,
            StackSettings.KeyDropletImage,
            StackSettings.KeyDropletName,
            StackSettings.KeyDropletTag,
        };

        foreach (var key in requiredKeys)
        {
            var fullKey = ns + key;
            Assert.True(yamlKeys.ContainsKey(fullKey),
                $"Pulumi.prod.yaml is missing required key '{fullKey}'.");
        }
    }

    [Fact]
    public void ProdYaml_ContainsNoOrphanKeys()
    {
        var yamlKeys = LoadYamlConfigKeys();
        var ns = StackSettings.ConfigNamespace + ":";

        var knownKeys = new HashSet<string>
        {
            ns + StackSettings.KeyRegion,
            ns + StackSettings.KeyDropletSize,
            ns + StackSettings.KeyDropletImage,
            ns + StackSettings.KeyDropletName,
            ns + StackSettings.KeyDropletTag,
        };
        knownKeys.UnionWith(StackSettings.AllowedForeignKeys);

        var orphans = yamlKeys.Keys
            .Where(k => !knownKeys.Contains(k))
            .ToList();

        Assert.Empty(orphans);
    }
}
