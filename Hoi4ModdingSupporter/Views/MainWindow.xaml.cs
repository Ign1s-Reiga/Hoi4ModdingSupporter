using System;
using System.Linq;
using System.Threading.Tasks;
using Hoi4ModdingSupporter.Models;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class MainWindow : Window {
        public MainWindow() {
            InitializeComponent();

            this.ExtendsContentIntoTitleBar = true;
            this.SetTitleBar(titleBar);

            this.AppWindow.SetIcon("Assets/WindowIcon.ico");
            this.AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
            ApplyAppTheme(SettingsRepository.Instance.CurrentSettings.AppTheme);

            navView.SelectedItem = navView.MenuItems.OfType<NavigationViewItem>().First();
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
            var pageType = args.IsSettingsSelected
                ? typeof(SettingsView)
                : args.SelectedItemContainer?.Tag switch {
                    "HomeView" => typeof(HomeView),
                    _ => null
                };

            if (pageType is not null && navFrame.CurrentSourcePageType != pageType) {
                if (!await ConfirmCurrentPageCanNavigateAsync()) {
                    return;
                }

                navFrame.Navigate(pageType);
            }
        }

        private async Task<bool> ConfirmCurrentPageCanNavigateAsync() {
            return navFrame.Content is not ProjectWorkspaceView workspace
                || await workspace.ConfirmNavigationAwayAsync();
        }
    }
}
