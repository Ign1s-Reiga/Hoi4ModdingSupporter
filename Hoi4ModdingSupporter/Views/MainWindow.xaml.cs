using System;
using System.Linq;
using System.Threading.Tasks;
using Hoi4ModdingSupporter.Models;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class MainWindow : Window {
        private RecentProjectRecord? currentWorkspace;
        private object? selectedNavigationItem;
        private bool suppressSelectionNavigation;

        public MainWindow() {
            InitializeComponent();

            this.ExtendsContentIntoTitleBar = true;
            this.SetTitleBar(titleBar);

            this.AppWindow.SetIcon("Assets/WindowIcon.ico");
            this.AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
            ApplyAppTheme(SettingsRepository.Instance.CurrentSettings.AppTheme);

            navView.SelectedItem = navView.MenuItems.OfType<NavigationViewItem>().First();
            selectedNavigationItem = navView.SelectedItem;
        }

        public void ApplyAppTheme(int appTheme) {
            rootGrid.RequestedTheme = appTheme switch {
                0 => ElementTheme.Light,
                1 => ElementTheme.Dark,
                _ => ElementTheme.Default
            };

            AppWindow.TitleBar.PreferredTheme = appTheme switch {
                0 => TitleBarTheme.Light,
                1 => TitleBarTheme.Dark,
                _ => TitleBarTheme.UseDefaultAppMode
            };
        }

        private void OnPaneToggleRequested(TitleBar sender, object args) {
            navView.IsPaneOpen = !navView.IsPaneOpen;
        }

        private async void OnBackRequested(TitleBar sender, object args) {
            if (navFrame.CanGoBack && await ConfirmCurrentPageCanNavigateAsync()) {
                navFrame.GoBack();
            }
        }

        private async void OnSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args) {
            if (suppressSelectionNavigation) {
                return;
            }

            var selectedItem = args.SelectedItemContainer;
            if (!args.IsSettingsSelected && TryGetWorkspaceSection(selectedItem, out var section)) {
                if (await SelectWorkspaceSectionAsync(section)) {
                    selectedNavigationItem = selectedItem;
                }
                else {
                    RestoreSelectedNavigationItem();
                }

                return;
            }

            var pageType = args.IsSettingsSelected
                ? typeof(SettingsView)
                : selectedItem?.Tag switch {
                    "HomeView" => typeof(HomeView),
                    _ => null
                };

            if (pageType is not null && navFrame.CurrentSourcePageType != pageType) {
                if (!await ConfirmCurrentPageCanNavigateAsync()) {
                    RestoreSelectedNavigationItem();
                    return;
                }

                navFrame.Navigate(pageType, pageType == typeof(ProjectWorkspaceView) ? currentWorkspace : null);
                selectedNavigationItem = args.IsSettingsSelected
                    ? navView.SettingsItem
                    : selectedItem;
            }
        }

        public async Task OpenWorkspaceAsync(ModDescriptor descriptor) {
            await OpenWorkspaceAsync(new RecentProjectRecord(
                FolderPath: descriptor.ProjectFolderPath,
                DisplayName: descriptor.DisplayName,
                LastAccessed: DateTime.Now,
                ImagePath: descriptor.ImagePath
            ));
        }

        public async Task OpenWorkspaceAsync(RecentProjectRecord project) {
            if (!await ConfirmCurrentPageCanNavigateAsync()) {
                return;
            }

            currentWorkspace = project;
            workspaceNavigationItem.Content = project.DisplayName;
            workspaceNavigationItem.Visibility = Visibility.Visible;
            var workspaceEditorItem = workspaceNavigationItem.MenuItems.OfType<NavigationViewItem>().FirstOrDefault()
                ?? workspaceNavigationItem;
            suppressSelectionNavigation = true;
            try {
                navView.SelectedItem = workspaceEditorItem;
            }
            finally {
                suppressSelectionNavigation = false;
            }

            selectedNavigationItem = workspaceEditorItem;
            navFrame.Navigate(
                typeof(ProjectWorkspaceView),
                new WorkspaceNavigationParameter(project, WorkspaceSection.VisualEditor)
            );
        }

        private async Task<bool> ConfirmCurrentPageCanNavigateAsync() {
            return navFrame.Content is not ProjectWorkspaceView workspace
                || await workspace.ConfirmNavigationAwayAsync();
        }

        private async Task<bool> SelectWorkspaceSectionAsync(WorkspaceSection section) {
            if (currentWorkspace is null) {
                RestoreSelectedNavigationItem();
                return false;
            }

            if (navFrame.Content is ProjectWorkspaceView workspace) {
                return await workspace.TrySelectSectionAsync(section);
            }

            if (!await ConfirmCurrentPageCanNavigateAsync()) {
                return false;
            }

            navFrame.Navigate(
                typeof(ProjectWorkspaceView),
                new WorkspaceNavigationParameter(currentWorkspace, section)
            );
            return true;
        }

        private static bool TryGetWorkspaceSection(NavigationViewItemBase? item, out WorkspaceSection section) {
            section = WorkspaceSection.VisualEditor;
            return item is NavigationViewItem navigationItem && navigationItem.Tag switch {
                "Workspace" or "WorkspaceEditor" => true,
                "WorkspaceModAssets" => SetSection(WorkspaceSection.ModAssets, out section),
                "WorkspaceGameAssets" => SetSection(WorkspaceSection.GameAssets, out section),
                _ => false
            };
        }

        private static bool SetSection(WorkspaceSection value, out WorkspaceSection section) {
            section = value;
            return true;
        }

        private void RestoreSelectedNavigationItem() {
            suppressSelectionNavigation = true;
            try {
                navView.SelectedItem = selectedNavigationItem;
            }
            finally {
                suppressSelectionNavigation = false;
            }
        }
    }
}
