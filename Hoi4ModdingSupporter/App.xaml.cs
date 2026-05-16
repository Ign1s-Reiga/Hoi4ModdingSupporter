using Hoi4ModdingSupporter.Models;
using Hoi4ModdingSupporter.Views;
using Microsoft.UI.Xaml;

namespace Hoi4ModdingSupporter {
    public partial class App : Application {
        private Window? window;

        public App() {
            InitializeComponent();

            // Retry reading settings file up to 5 times
            int retry = 5;
            while (retry > 0) {
                var result = SettingsRepository.Instance.ReadSettingsFileAsync();
                if (result.IsSuccess) {
                    break;
                }
                retry--;
            }
        }

        protected override void OnLaunched(LaunchActivatedEventArgs args) {
            window = new MainWindow();
            window.Activate();
        }
    }
}
