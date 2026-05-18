using System;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Linq;
using System.Threading.Tasks;
using Hoi4ModdingSupporter.Models;
using Hoi4ModdingSupporter.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Hoi4ModdingSupporter.Views {
    public sealed partial class ProjectWorkspaceView : Page {
        private ProjectWorkspaceViewModel? canvasViewModel;

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
            HookNationalFocusCanvas(ViewModel);
            RenderNationalFocusCanvas();
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
            RenderNationalFocusCanvas();
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
                ViewModel.DiscardPendingChanges();
                return true;
            }

            if (result != ContentDialogResult.Primary) {
                return false;
            }

            ViewModel.SavePendingChanges();
            return !ViewModel.HasUnsavedChanges;
        }

        private void OnFileSelectionChanged(object sender, SelectionChangedEventArgs args) {
            if (args.AddedItems.FirstOrDefault() is not ProjectWorkspaceFile file) {
                return;
            }

            ViewModel.SelectFile(file);
        }

        private void OnNationalFocusFileSelectionChanged(object sender, SelectionChangedEventArgs args) {
            Bindings.Update();
            RenderNationalFocusCanvas();
        }

        private void OnNationalFocusSelectionChanged(object sender, SelectionChangedEventArgs args) {
            RenderNationalFocusCanvas();
        }

        private void HookNationalFocusCanvas(ProjectWorkspaceViewModel viewModel) {
            if (canvasViewModel is not null) {
                canvasViewModel.VisibleNationalFocuses.CollectionChanged -= OnVisibleNationalFocusesChanged;
                canvasViewModel.PropertyChanged -= OnViewModelPropertyChanged;
            }

            canvasViewModel = viewModel;
            canvasViewModel.VisibleNationalFocuses.CollectionChanged += OnVisibleNationalFocusesChanged;
            canvasViewModel.PropertyChanged += OnViewModelPropertyChanged;
        }

        private void OnVisibleNationalFocusesChanged(object? sender, NotifyCollectionChangedEventArgs args) {
            RenderNationalFocusCanvas();
        }

        private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs args) {
            if (args.PropertyName is nameof(ProjectWorkspaceViewModel.SelectedNationalFocus)) {
                RenderNationalFocusCanvas();
            }
        }

        private void RenderNationalFocusCanvas() {
            nationalFocusCanvas.Children.Clear();

            foreach (var focus in ViewModel.VisibleNationalFocuses) {
                var button = CreateFocusButton(focus);
                Canvas.SetLeft(button, GetFocusCanvasCoordinate(focus.X, ViewModel.VisibleNationalFocuses.IndexOf(focus), 170));
                Canvas.SetTop(button, GetFocusCanvasCoordinate(focus.Y, ViewModel.VisibleNationalFocuses.IndexOf(focus), 120));
                nationalFocusCanvas.Children.Add(button);
            }
        }

        private Button CreateFocusButton(NationalFocusEntry focus) {
            var isSelected = Equals(focus, ViewModel.SelectedNationalFocus);
            var border = new Border {
                Width = 112,
                Height = 72,
                CornerRadius = new CornerRadius(6),
                BorderThickness = new Thickness(isSelected ? 2 : 1),
                BorderBrush = new SolidColorBrush(isSelected ? Colors.DodgerBlue : Colors.Gray),
                Background = new SolidColorBrush(Colors.Transparent),
                Child = new StackPanel {
                    Padding = new Thickness(6),
                    Spacing = 4,
                    Children = {
                        new TextBlock {
                            Text = string.IsNullOrWhiteSpace(focus.Icon) ? "No icon" : focus.Icon,
                            TextTrimming = Microsoft.UI.Xaml.TextTrimming.CharacterEllipsis,
                            FontSize = 11
                        },
                        new TextBlock {
                            Text = focus.DisplayName,
                            TextTrimming = Microsoft.UI.Xaml.TextTrimming.CharacterEllipsis,
                            FontSize = 12
                        }
                    }
                }
            };

            var button = new Button {
                Padding = new Thickness(0),
                Content = border
            };
            ToolTipService.SetToolTip(button, focus.DisplayName);
            button.Click += (_, _) => {
                if (!ViewModel.CanChangeWorkspaceSelection) {
                    return;
                }

                ViewModel.SelectedNationalFocus = focus;
                Bindings.Update();
                RenderNationalFocusCanvas();
            };

            return button;
        }

        private static double GetFocusCanvasCoordinate(string value, int index, double size) {
            if (int.TryParse(value, out var coordinate)) {
                return Math.Max(24, coordinate * size + 24);
            }

            return index * size + 24;
        }
    }
}
