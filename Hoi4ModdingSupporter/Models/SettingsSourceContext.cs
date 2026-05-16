using System;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hoi4ModdingSupporter.Models {
    [JsonSourceGenerationOptions(
        AllowTrailingCommas = true,
        WriteIndented = true,
        ReadCommentHandling = JsonCommentHandling.Skip,
        PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase
    )]
    [JsonSerializable(typeof(SettingsRecord))]
    internal partial class SettingsSourceContext : JsonSerializerContext { }

    public record SettingsRecord(
        int Version,
        int AppTheme,
        RecentProjectRecord[] RecentProjects
    );

    public record RecentProjectRecord(string FolderPath, string DisplayName, DateTime LastAccessed, string ImagePath);
}
