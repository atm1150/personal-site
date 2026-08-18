using Pulumi;
using Pulumi.DigitalOcean;
using Pulumi.DigitalOcean.Inputs;
using Portfolio.Infra;

return await Deployment.RunAsync(() =>
{
    var config = new Pulumi.Config(StackSettings.ConfigNamespace);
    var settings = StackSettings.Load(key => config.Require(key));

    var adminIp = InfraConfig.RequireAdminIp();
    var sshPublicKey = InfraConfig.RequireSshPublicKey();

    var sshKey = new SshKey("portfolio-ssh-key", new SshKeyArgs
    {
        Name = "portfolio-deploy-key",
        PublicKey = sshPublicKey,
    });

    // Slug verified against DigitalOcean's image list at first preview.
    var droplet = new Droplet("portfolio-droplet", new DropletArgs
    {
        Name = settings.DropletName,
        Size = settings.DropletSize,
        Image = settings.DropletImage,
        Region = settings.Region,
        SshKeys = new InputList<string> { sshKey.Id },
        Monitoring = true,
        Ipv6 = false,
        Tags = new InputList<string> { settings.DropletTag },
    });

    // Droplet.Id is a string in Pulumi; the DO API uses an integer droplet ID.
    var dropletId = droplet.Id.Apply(int.Parse);

    var reservedIp = new ReservedIp("portfolio-reserved-ip", new ReservedIpArgs
    {
        Region = settings.Region,
    });

    _ = new ReservedIpAssignment("portfolio-ip-assignment", new ReservedIpAssignmentArgs
    {
        IpAddress = reservedIp.IpAddress,
        DropletId = dropletId,
    });

    var adminCidr = adminIp + "/32";

    _ = new Firewall("portfolio-firewall", new FirewallArgs
    {
        Name = "portfolio-firewall",
        DropletIds = new InputList<int> { dropletId },
        InboundRules =
        {
            new FirewallInboundRuleArgs
            {
                Protocol = "tcp",
                PortRange = "80",
                SourceAddresses = new InputList<string> { "0.0.0.0/0", "::/0" },
            },
            new FirewallInboundRuleArgs
            {
                Protocol = "tcp",
                PortRange = "443",
                SourceAddresses = new InputList<string> { "0.0.0.0/0", "::/0" },
            },
            new FirewallInboundRuleArgs
            {
                Protocol = "tcp",
                PortRange = "22",
                SourceAddresses = new InputList<string> { adminCidr },
            },
        },
        OutboundRules =
        {
            new FirewallOutboundRuleArgs
            {
                Protocol = "tcp",
                PortRange = "all",
                DestinationAddresses = new InputList<string> { "0.0.0.0/0", "::/0" },
            },
            new FirewallOutboundRuleArgs
            {
                Protocol = "udp",
                PortRange = "all",
                DestinationAddresses = new InputList<string> { "0.0.0.0/0", "::/0" },
            },
            new FirewallOutboundRuleArgs
            {
                Protocol = "icmp",
                DestinationAddresses = new InputList<string> { "0.0.0.0/0", "::/0" },
            },
        },
    });

    return new Dictionary<string, object?>
    {
        ["reservedIpAddress"] = reservedIp.IpAddress,
    };
});
