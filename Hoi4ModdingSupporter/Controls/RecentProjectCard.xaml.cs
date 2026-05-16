using System.IO;
using Hoi4ModdingSupporter.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Controls {
    public sealed partial class RecentProjectCard : UserControl {
        public RecentProjectRecord RecentProject;
        public Visibility IsImageFileExists { get; set; } = Visibility.Collapsed;

        public RecentProjectCard(RecentProjectRecord recentProject) {
            InitializeComponent();

            this.RecentProject = recentProject;
            this.IsImageFileExists = !string.IsNullOrEmpty(recentProject.ImagePath) && File.Exists(recentProject.ImagePath)
                ? Visibility.Visible
                : Visibility.Collapsed;
        }
    }
}
