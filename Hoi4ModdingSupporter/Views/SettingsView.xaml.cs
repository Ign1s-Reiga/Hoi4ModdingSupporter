using System;
using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

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

            if (args.PropertyName == nameof(ViewModel.GameRootPathDisplay)) {
                Bindings.Update();
            }
        }

        private async void OnBrowseGameRootClicked(object sender, RoutedEventArgs args) {
            var window = App.MainWindow;
            if (window is null) {
                return;
            }

            var picker = new FolderPicker();
            picker.FileTypeFilter.Add("*");
            InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(window));

            var folder = await picker.PickSingleFolderAsync();
            if (folder is not null) {
                ViewModel.GameRootPath = folder.Path;
            }
        }
    }
}
