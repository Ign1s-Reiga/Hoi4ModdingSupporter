using System;
using System.IO;
using Hoi4ModdingSupporter.Models;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Hoi4ModdingSupporter.Controls {
    public sealed partial class RecentProjectCard : UserControl {
        private ImageSource? projectImageSource;

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

        public ImageSource? ProjectImageSource => projectImageSource;

        public Visibility ProjectImageVisibility => projectImageSource is null
            ? Visibility.Collapsed
            : Visibility.Visible;

        public Visibility FallbackIconVisibility => ProjectImageVisibility == Visibility.Visible
            ? Visibility.Collapsed
            : Visibility.Visible;

        public RecentProjectCard() {
            InitializeComponent();
        }

        private static void OnRecentProjectChanged(DependencyObject dependencyObject, DependencyPropertyChangedEventArgs args) {
            if (dependencyObject is RecentProjectCard card) {
                card.projectImageSource = CreateProjectImageSource(card.RecentProject?.ImagePath);
                card.Bindings.Update();
            }
        }

        private static ImageSource? CreateProjectImageSource(string? imagePath) {
            if (string.IsNullOrWhiteSpace(imagePath) || !File.Exists(imagePath)) {
                return null;
            }

            try {
                var fullPath = Path.GetFullPath(imagePath);
                if (!Uri.TryCreate(fullPath, UriKind.Absolute, out var imageUri)) {
                    return null;
                }

                return new BitmapImage(imageUri);
            }
            catch (ArgumentException) {
                return null;
            }
            catch (IOException) {
                return null;
            }
            catch (UnauthorizedAccessException) {
                return null;
            }
        }

        private void OnProjectImageFailed(object sender, ExceptionRoutedEventArgs args) {
            projectImageSource = null;
            Bindings.Update();
        }
    }
}
