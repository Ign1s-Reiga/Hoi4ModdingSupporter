using System;
using Hoi4ModdingSupporter.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Hoi4ModdingSupporter.Controls {
    public sealed partial class RecentProjectCard : UserControl {
        public static readonly DependencyProperty RecentProjectProperty = DependencyProperty.Register(
            name: nameof(RecentProject),
            propertyType: typeof(RecentProjectRecord),
            ownerType: typeof(RecentProjectCard),
            typeMetadata: new PropertyMetadata(null, OnRecentProjectChanged)
        );

        public RecentProjectRecord? RecentProject {
            get => (RecentProjectRecord?)GetValue(RecentProjectProperty);
            set => SetValue(RecentProjectProperty, value);
        }

        public string DisplayName => string.IsNullOrWhiteSpace(RecentProject?.DisplayName)
            ? "Unnamed Project"
            : RecentProject.DisplayName;

        public string FolderPath => RecentProject?.FolderPath ?? string.Empty;

        public string LastAccessedText => RecentProject is null
            ? string.Empty
            : $"Last opened {RecentProject.LastAccessed:g}";

        public RecentProjectCard() {
            InitializeComponent();
        }

        private static void OnRecentProjectChanged(DependencyObject dependencyObject, DependencyPropertyChangedEventArgs args) {
            if (dependencyObject is RecentProjectCard card) {
                card.Bindings.Update();
            }
        }
    }
}
