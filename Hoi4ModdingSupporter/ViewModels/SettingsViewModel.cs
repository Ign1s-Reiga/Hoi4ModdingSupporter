using System.ComponentModel.DataAnnotations;
using CommunityToolkit.Mvvm.ComponentModel;
using Hoi4ModdingSupporter.Models;

namespace Hoi4ModdingSupporter.ViewModels {
    public partial class SettingsViewModel : ObservableValidator {
        private bool isInitializing = true;

        [ObservableProperty]
        [Range(0, 2)]
        private int appTheme = 2;

        private string gameRootPath = string.Empty;

        public SettingsViewModel() {
            var current = SettingsRepository.Instance.CurrentSettings;

            AppTheme = (current.AppTheme >= 0 && current.AppTheme < 3)
                        ? current.AppTheme
                        : 2;
            GameRootPath = current.GameRootPath ?? string.Empty;

            ValidateAllProperties();
            isInitializing = false;
        }

        public string GameRootPathDisplay => string.IsNullOrWhiteSpace(GameRootPath)
            ? "Not configured"
            : GameRootPath;

        public string GameRootPath {
            get => gameRootPath;
            set {
                if (SetProperty(ref gameRootPath, value ?? string.Empty)) {
                    OnGameRootPathChanged(gameRootPath);
                }
            }
        }

        partial void OnAppThemeChanged(int value) {
            if (isInitializing || value is < 0 or > 2) {
                return;
            }

            SettingsRepository.Instance.SaveSettingsFileAsync(
                SettingsRepository.Instance.CurrentSettings with {
                    AppTheme = value
                }
            );
        }

        private void OnGameRootPathChanged(string value) {
            OnPropertyChanged(nameof(GameRootPathDisplay));

            if (isInitializing) {
                return;
            }

            SettingsRepository.Instance.SaveSettingsFileAsync(
                SettingsRepository.Instance.CurrentSettings with {
                    GameRootPath = value ?? string.Empty
                }
            );
        }
    }
}
