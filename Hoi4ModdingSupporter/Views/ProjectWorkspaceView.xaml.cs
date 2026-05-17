using System;
using System.Linq;
using System.Threading.Tasks;
using Hoi4ModdingSupporter.Models;
using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class ProjectWorkspaceView : Page {
        public ProjectWorkspaceViewModel ViewModel { get; private set; } = new(
            new RecentProjectRecord(
                FolderPath: string.Empty,
                DisplayName: "Project Workspace",
                LastAccessed: DateTime.Now,
                ImagePath: string.Empty
            )
        );

        public ProjectWorkspaceView() {
            InitializeComponent();
        }

        protected override void OnNavigatedTo(NavigationEventArgs e) {
            ViewModel = e.Parameter switch {
                WorkspaceNavigationParameter parameter => new ProjectWorkspaceViewModel(parameter.Project) {
                    CurrentSection = parameter.Section
                },
                ModDescriptor descriptor => new ProjectWorkspaceViewModel(ToRecentProject(descriptor)),
                RecentProjectRecord recentProject => new ProjectWorkspaceViewModel(recentProject),
                _ => ViewModel
            };
            Bindings.Update();
        }

        private static RecentProjectRecord ToRecentProject(ModDescriptor descriptor) {
            return new RecentProjectRecord(
                FolderPath: descriptor.ProjectFolderPath,
                DisplayName: descriptor.DisplayName,
                LastAccessed: DateTime.Now,
                ImagePath: descriptor.ImagePath
            );
        }

        public async Task<bool> TrySelectSectionAsync(WorkspaceSection section) {
            if (ViewModel.CurrentSection == section) {
                return true;
            }

            if (!await ConfirmNavigationAwayAsync()) {
                return false;
            }

            ViewModel.CurrentSection = section;
            Bindings.Update();
            return true;
        }

        public async Task<bool> ConfirmNavigationAwayAsync() {
            if (!ViewModel.HasUnsavedChanges) {
                return true;
            }

            var dialog = new ContentDialog {
                Title = "Unsaved Changes",
                Content = "Save changes before leaving this workspace?",
                PrimaryButtonText = "Save",
                SecondaryButtonText = "Discard",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = XamlRoot
            };

            var result = await dialog.ShowAsync();
            if (result == ContentDialogResult.Secondary) {
                ViewModel.DiscardSelectedFileChanges();
                return true;
            }

            if (result != ContentDialogResult.Primary) {
                return false;
            }

            ViewModel.SaveSelectedFileCommand.Execute(null);
            return !ViewModel.HasUnsavedChanges;
        }

        private void OnFileSelectionChanged(object sender, SelectionChangedEventArgs args) {
            if (args.AddedItems.FirstOrDefault() is not ProjectWorkspaceFile file) {
                return;
            }

            ViewModel.SelectFile(file);
        }
    }
}
