using System;
using System.IO;
using System.Text.Json;
using FluentResults;

namespace Hoi4ModdingSupporter.Models {
    public class SettingsRepository {
        private static readonly Lazy<SettingsRepository> _instance = new Lazy<SettingsRepository>(() => new SettingsRepository());
        public static SettingsRepository Instance => _instance.Value;

        public SettingsRecord CurrentSettings { get; set; } = new SettingsRecord(
            Version: 1,
            AppTheme: 2,
            RecentProjects: []
        );

        private readonly string settingsFilePath = Path.Combine(Environment.CurrentDirectory, "settings.json");
        private readonly int currentVersion = 1;

        private SettingsRepository() {}

        public Result ReadSettingsFileAsync() {
            if (!File.Exists(settingsFilePath)) {
                SaveSettingsFileAsync(CurrentSettings);

                return Result.Ok();
            }

            return Result.Try(() => {
                var json = File.ReadAllText(settingsFilePath);

                CurrentSettings = JsonSerializer.Deserialize(json, SettingsSourceContext.Default.SettingsRecord) ?? CurrentSettings;
                return Result.Ok();
            });
        }

        public Result SaveSettingsFileAsync(SettingsRecord settings) {
            return Result.Try(() => {
                var json = JsonSerializer.Serialize(settings, SettingsSourceContext.Default.SettingsRecord);
                File.WriteAllText(settingsFilePath, json);

                CurrentSettings = settings;

                return Result.Ok();
            });
        }
    }
}
