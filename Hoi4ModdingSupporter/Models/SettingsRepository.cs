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
            GameRootPath: string.Empty,
            RecentProjects: []
        );

        private readonly string settingsDirectoryPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Hoi4ModdingSupporter"
        );
        private readonly string settingsFilePath;

        private SettingsRepository() {
            settingsFilePath = Path.Combine(settingsDirectoryPath, "settings.json");
        }

        public Result ReadSettingsFileAsync() {
            if (!File.Exists(settingsFilePath)) {
                SaveSettingsFileAsync(CurrentSettings);

                return Result.Ok();
            }

            return Result.Try(() => {
                var json = File.ReadAllText(settingsFilePath);

                var settings = JsonSerializer.Deserialize(json, SettingsSourceContext.Default.SettingsRecord) ?? CurrentSettings;
                CurrentSettings = settings with {
                    GameRootPath = settings.GameRootPath ?? string.Empty,
                    RecentProjects = settings.RecentProjects ?? []
                };
                return Result.Ok();
            });
        }

        public Result SaveSettingsFileAsync(SettingsRecord settings) {
            return Result.Try(() => {
                Directory.CreateDirectory(settingsDirectoryPath);

                var json = JsonSerializer.Serialize(settings, SettingsSourceContext.Default.SettingsRecord);
                File.WriteAllText(settingsFilePath, json);

                CurrentSettings = settings;

                return Result.Ok();
            });
        }
    }
}
