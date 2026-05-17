using System;
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

        public string SelectedFileTitle => ViewModel.SelectedFile?.RelativePath ?? "Select a text file";

        public bool IsEditorReadOnly => ViewModel.SelectedFile?.IsTextFile != true;

        public ProjectWorkspaceView() {
            InitializeComponent();
            ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        }

        protected override void OnNavigatedTo(NavigationEventArgs e) {
            ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
            ViewModel = e.Parameter switch {
                ModDescriptor descriptor => new ProjectWorkspaceViewModel(ToRecentProject(descriptor)),
                RecentProjectRecord recentProject => new ProjectWorkspaceViewModel(recentProject),
                _ => ViewModel
            };
            ViewModel.PropertyChanged += OnViewModelPropertyChanged;
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

        private void OnFileSelectionChanged(object sender, SelectionChangedEventArgs args) {
            if (ViewModel.SelectedFile?.IsTextFile == true) {
                ViewModel.LoadSelectedFileCommand.Execute(null);
            }
            else {
                ViewModel.UnloadEditor();
            }

            Bindings.Update();
        }

        private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs args) {
            if (args.PropertyName is nameof(ViewModel.SelectedFile) or nameof(ViewModel.StatusMessage)) {
                Bindings.Update();
            }
        }
    }
}
