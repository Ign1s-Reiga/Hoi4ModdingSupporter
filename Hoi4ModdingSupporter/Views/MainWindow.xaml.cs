using System;
using System.Linq;
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
            this.AppWindow.TitleBar.PreferredTheme = TitleBarTheme.UseDefaultAppMode;
            this.AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;

            navView.SelectedItem = navView.MenuItems.OfType<NavigationViewItem>().First();
        }

        private void OnPaneToggleRequested(TitleBar sender, object args) {
            navView.IsPaneOpen = !navView.IsPaneOpen;
        }

        private void OnBackRequested(TitleBar sender, object args) {
            navFrame.GoBack();
        }

        private void OnSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args) {
            var selectedItem = (NavigationViewItem)navView.SelectedItem;
            if (selectedItem is NavigationViewItem item) {
                var tag = args.IsSettingsSelected ? "SettingsView" : (string)item.Tag;

                Type? pageType = Type.GetType($"Hoi4ModdingSupporter.Views.{tag}");
                navFrame.Navigate(pageType);
            }
        }
    }
}
