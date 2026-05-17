using System.Collections.ObjectModel;
using System.Linq;
using Hoi4ModdingSupporter.Models;

namespace Hoi4ModdingSupporter.ViewModels {
    public class HomeViewModel {
        public ObservableCollection<RecentProjectRecord> RecentProjects { get; }

        public HomeViewModel() {
            RecentProjects = new ObservableCollection<RecentProjectRecord>(
                SettingsRepository.Instance.CurrentSettings.RecentProjects
                    .OrderByDescending(project => project.LastAccessed)
            );
        }

        public void AddRecentProject(ModDescriptor descriptor) {
            var recentProject = new RecentProjectRecord(
                FolderPath: descriptor.ProjectFolderPath,
                DisplayName: descriptor.DisplayName,
                LastAccessed: System.DateTime.Now,
                ImagePath: descriptor.ImagePath
            );

            var existingProject = RecentProjects.FirstOrDefault(project =>
                string.Equals(project.FolderPath, recentProject.FolderPath, System.StringComparison.OrdinalIgnoreCase)
            );
            if (existingProject is not null) {
                RecentProjects.Remove(existingProject);
            }

            RecentProjects.Insert(0, recentProject);

            SettingsRepository.Instance.SaveSettingsFileAsync(
                SettingsRepository.Instance.CurrentSettings with {
                    RecentProjects = RecentProjects.ToArray()
                }
            );
        }
    }
}
