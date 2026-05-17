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
    }
}
