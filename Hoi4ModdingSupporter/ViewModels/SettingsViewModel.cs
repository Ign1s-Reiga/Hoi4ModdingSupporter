using System.ComponentModel.DataAnnotations;
using CommunityToolkit.Mvvm.ComponentModel;
using Hoi4ModdingSupporter.Models;

namespace Hoi4ModdingSupporter.ViewModels {
    public partial class SettingsViewModel : ObservableValidator {
        [ObservableProperty]
        [Range(0, 2)]
        private int appTheme = 2;

        public SettingsViewModel() {
            var current = SettingsRepository.Instance.CurrentSettings;

            AppTheme = (current.AppTheme >= 0 && current.AppTheme < 3)
                        ? current.AppTheme
                        : 2;

            ValidateAllProperties();
        }
    }
}
