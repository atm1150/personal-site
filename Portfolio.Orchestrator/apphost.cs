var builder = DistributedApplication.CreateBuilder(args);

builder.AddLeptosServerApp("site", "../site");

builder.Build().Run();
