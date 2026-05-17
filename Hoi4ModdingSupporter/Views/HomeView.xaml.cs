using System;
using System.Linq;
using Hoi4ModdingSupporter.Models;
using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class HomeView : Page {
        public HomeViewModel ViewModel { get; } = new();

        public HomeView() {
            InitializeComponent();
        }

        private async void OnOpenProjectClicked(object sender, RoutedEventArgs args) {
            var picker = new FileOpenPicker();
            picker.FileTypeFilter.Add(".mod");

            var window = App.MainWindow;
            if (window is null) {
                return;
            }

            InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(window));

            var file = await picker.PickSingleFileAsync();
            if (file is null) {
                return;
            }

            var descriptorResult = ModDescriptorReader.Read(file.Path);
            if (descriptorResult.IsSuccess) {
                ViewModel.AddRecentProject(descriptorResult.Value);
                return;
            }

            await ShowErrorDialogAsync(descriptorResult.Errors.FirstOrDefault()?.Message ?? "Failed to open the selected mod file.");
        }

        private async System.Threading.Tasks.Task ShowErrorDialogAsync(string message) {
            var dialog = new ContentDialog {
                Title = "Open Project Failed",
                Content = message,
                CloseButtonText = "OK",
                XamlRoot = XamlRoot
            };

            await dialog.ShowAsync();
        }
    }
}
