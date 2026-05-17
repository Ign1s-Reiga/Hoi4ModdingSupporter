using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class HomeView : Page {
        public HomeViewModel ViewModel { get; } = new();

        public HomeView() {
            InitializeComponent();
        }
    }
}
