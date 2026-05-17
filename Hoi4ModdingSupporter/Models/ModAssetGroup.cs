using System.Collections.ObjectModel;

namespace Hoi4ModdingSupporter.Models {
    public record ModAssetGroup(
        string AreaName,
        string DisplayName,
        ObservableCollection<ProjectWorkspaceFile> Entries
    );
}
