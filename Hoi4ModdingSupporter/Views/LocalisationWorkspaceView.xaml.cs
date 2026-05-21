using System;
using System.Threading.Tasks;
using Hoi4ModdingSupporter.Models;
using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class LocalisationWorkspaceView : Page {
        public LocalisationWorkspaceViewModel ViewModel { get; private set; } = new(
            new RecentProjectRecord(
                FolderPath: string.Empty,
                DisplayName: "Localisation",
                LastAccessed: DateTime.Now,
                ImagePath: string.Empty
            )
        );

        public LocalisationWorkspaceView() {
            InitializeComponent();
        }

        protected override void OnNavigatedTo(NavigationEventArgs e) {
            ViewModel = e.Parameter switch {
                ModDescriptor descriptor => new LocalisationWorkspaceViewModel(ToRecentProject(descriptor)),
                RecentProjectRecord recentProject => new LocalisationWorkspaceViewModel(recentProject),
                _ => ViewModel
            };
            Bindings.Update();
        }

        public async Task<bool> ConfirmNavigationAwayAsync() {
            if (!ViewModel.HasUnsavedChanges) {
                return true;
            }

            var dialog = new ContentDialog {
                Title = "Unsaved Changes",
                Content = "Save localisation changes before leaving this workspace?",
                PrimaryButtonText = "Save",
                SecondaryButtonText = "Discard",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = XamlRoot
            };

            var result = await dialog.ShowAsync();
            if (result == ContentDialogResult.Secondary) {
                ViewModel.DiscardPendingChanges();
                return true;
            }

            if (result != ContentDialogResult.Primary) {
                return false;
            }

            ViewModel.SavePendingChanges();
            return !ViewModel.HasUnsavedChanges;
        }

        private static RecentProjectRecord ToRecentProject(ModDescriptor descriptor) {
            return new RecentProjectRecord(
                FolderPath: descriptor.ProjectFolderPath,
                DisplayName: descriptor.DisplayName,
                LastAccessed: DateTime.Now,
                ImagePath: descriptor.ImagePath
            );
        }
    }
}
