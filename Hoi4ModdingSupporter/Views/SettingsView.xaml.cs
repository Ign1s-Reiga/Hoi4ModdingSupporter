using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class SettingsView : Page {
        public SettingsViewModel ViewModel { get; } = new();
        public SettingsView() {
            InitializeComponent();
        }
    }
}
