using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class SettingsView : Page {
        public SettingsViewModel ViewModel { get; } = new();
        public SettingsView() {
            InitializeComponent();
            ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        }

        private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs args) {
            if (args.PropertyName == nameof(ViewModel.AppTheme) && App.MainWindow is MainWindow mainWindow) {
                mainWindow.ApplyAppTheme(ViewModel.AppTheme);
            }
        }
    }
}
