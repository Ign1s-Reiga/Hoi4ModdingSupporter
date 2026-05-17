using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Text;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using FluentResults;
using Hoi4ModdingSupporter.Models;

namespace Hoi4ModdingSupporter.ViewModels {
    public class ProjectWorkspaceViewModel : ObservableObject {
        private const int MaxProjectFileCount = 5000;
        private const long MaxTextFileSizeBytes = 1024 * 1024;

        private static readonly ModAssetArea[] AssetAreas = [
            new("common", "Common"),
            new("events", "Events"),
            new("gfx", "GFX"),
            new("history", "History"),
            new("interface", "Interface"),
            new("map", "Map"),
            new("music", "Music"),
            new("localisation", "Localisation")
        ];

        private static readonly HashSet<string> ExcludedDirectoryNames = new(StringComparer.OrdinalIgnoreCase) {
            ".git",
            ".vs",
            "bin",
            "obj"
        };

        private static readonly HashSet<string> TextFileExtensions = new(StringComparer.OrdinalIgnoreCase) {
            ".asset",
            ".csv",
            ".gui",
            ".gfx",
            ".json",
            ".lua",
            ".mod",
            ".txt",
            ".yml",
            ".yaml"
        };

        private static readonly ProjectFileGroup[] ScriptGroups = [
            new("All Editable Text", []),
            new("National Focuses", [NormalizeRelativePath("common/national_focus")]),
            new("Events", [NormalizeRelativePath("events")]),
            new("History", [NormalizeRelativePath("history")]),
            new("Ideologies", [NormalizeRelativePath("common/ideologies")]),
            new("Decisions", [NormalizeRelativePath("common/decisions")]),
            new("Scripted Effects", [NormalizeRelativePath("common/scripted_effects")]),
            new("Scripted Triggers", [NormalizeRelativePath("common/scripted_triggers")]),
            new("Localisation", [NormalizeRelativePath("localisation")]),
            new("Interface", [NormalizeRelativePath("interface")])
        ];

        private ProjectWorkspaceFile? selectedFile;
        private ProjectFileGroup? selectedFileGroup;
        private ModAssetGroup? selectedAssetGroup;
        private ModAssetGroup? selectedGameAssetGroup;
        private string fileSearchText = string.Empty;
        private string selectedFileContent = string.Empty;
        private string gameAssetStatusText = string.Empty;
        private string statusMessage = string.Empty;
        private bool hasUnsavedChanges;
        private bool isSettingEditorContent;

        public ProjectWorkspaceViewModel(RecentProjectRecord project) {
            Project = project;

            RefreshFilesCommand = new RelayCommand(() => {
                RefreshFiles();
            }, () => CanChangeWorkspaceSelection);
            LoadSelectedFileCommand = new RelayCommand(
                () => StoreResult(LoadSelectedTextFile()),
                CanLoadOrSaveSelectedFile
            );
            SaveSelectedFileCommand = new RelayCommand(
                () => StoreResult(SaveSelectedTextContent()),
                CanLoadOrSaveSelectedFile
            );

            foreach (var scriptGroup in ScriptGroups) {
                FileGroups.Add(scriptGroup);
            }

            selectedFileGroup = FileGroups.FirstOrDefault();

            RefreshFiles();
        }

        public RecentProjectRecord Project { get; }

        public ObservableCollection<ProjectWorkspaceFile> Files { get; } = [];

        public ObservableCollection<ProjectWorkspaceFile> EditableFiles { get; } = [];

        public ObservableCollection<ProjectWorkspaceFile> FilteredEditableFiles { get; } = [];

        public ObservableCollection<ProjectFileGroup> FileGroups { get; } = [];

        public ProjectFileGroup? SelectedFileGroup {
            get => selectedFileGroup;
            set {
                if (SetProperty(ref selectedFileGroup, value)) {
                    RefreshFilteredEditableFiles();
                }
            }
        }

        public string FileSearchText {
            get => fileSearchText;
            set {
                if (SetProperty(ref fileSearchText, value)) {
                    RefreshFilteredEditableFiles();
                }
            }
        }

        public ObservableCollection<ModAssetGroup> AssetGroups { get; } = [];

        public ObservableCollection<ModAssetGroup> GameAssetGroups { get; } = [];

        public ModAssetGroup? SelectedAssetGroup {
            get => selectedAssetGroup;
            set {
                if (SetProperty(ref selectedAssetGroup, value)) {
                    OnPropertyChanged(nameof(SelectedAssetEntries));
                }
            }
        }

        public ObservableCollection<ProjectWorkspaceFile> SelectedAssetEntries => SelectedAssetGroup?.Entries ?? [];

        public ModAssetGroup? SelectedGameAssetGroup {
            get => selectedGameAssetGroup;
            set {
                if (SetProperty(ref selectedGameAssetGroup, value)) {
                    OnPropertyChanged(nameof(SelectedGameAssetEntries));
                }
            }
        }

        public ObservableCollection<ProjectWorkspaceFile> SelectedGameAssetEntries => SelectedGameAssetGroup?.Entries ?? [];

        public ProjectWorkspaceFile? SelectedFile {
            get => selectedFile;
            set {
                if (SetProperty(ref selectedFile, value)) {
                    LoadSelectedFileCommand.NotifyCanExecuteChanged();
                    SaveSelectedFileCommand.NotifyCanExecuteChanged();
                    OnPropertyChanged(nameof(SelectedFileTitle));
                    OnPropertyChanged(nameof(IsEditorReadOnly));
                    OnPropertyChanged(nameof(SelectedFileStatusText));

                    if (value?.IsTextFile == false) {
                        UnloadEditor();
                    }
                }
            }
        }

        public string SelectedFileContent {
            get => selectedFileContent;
            set {
                if (SetProperty(ref selectedFileContent, value) && !isSettingEditorContent) {
                    HasUnsavedChanges = SelectedFile is not null;
                }
            }
        }

        public string StatusMessage {
            get => statusMessage;
            private set {
                if (SetProperty(ref statusMessage, value)) {
                    OnPropertyChanged(nameof(WorkspaceStatusText));
                }
            }
        }

        public string GameAssetStatusText {
            get => gameAssetStatusText;
            private set {
                if (SetProperty(ref gameAssetStatusText, value)) {
                    OnPropertyChanged(nameof(WorkspaceStatusText));
                }
            }
        }

        public string WorkspaceStatusText => string.IsNullOrWhiteSpace(StatusMessage)
            ? GameAssetStatusText
            : StatusMessage;

        public bool HasUnsavedChanges {
            get => hasUnsavedChanges;
            private set {
                if (SetProperty(ref hasUnsavedChanges, value)) {
                    RefreshFilesCommand.NotifyCanExecuteChanged();
                    OnPropertyChanged(nameof(CanChangeWorkspaceSelection));
                    OnPropertyChanged(nameof(SelectedFileStatusText));
                }
            }
        }

        public bool CanChangeWorkspaceSelection => !HasUnsavedChanges;

        public string SelectedFileTitle => SelectedFile?.RelativePath ?? "Select a text file";

        public bool IsEditorReadOnly => SelectedFile?.IsTextFile != true;

        public string SelectedFileStatusText {
            get {
                if (SelectedFile is null) {
                    return $"{FilteredEditableFiles.Count:N0} scripts";
                }

                if (HasUnsavedChanges) {
                    return "Unsaved changes";
                }

                return $"{SelectedFile.SizeBytes:N0} bytes";
            }
        }

        public IRelayCommand RefreshFilesCommand { get; }

        public IRelayCommand LoadSelectedFileCommand { get; }

        public IRelayCommand SaveSelectedFileCommand { get; }

        public Result RefreshFiles() {
            Files.Clear();
            EditableFiles.Clear();
            FilteredEditableFiles.Clear();
            AssetGroups.Clear();
            GameAssetGroups.Clear();
            SelectedAssetGroup = null;
            SelectedGameAssetGroup = null;
            SelectedFile = null;
            SelectedFileContent = string.Empty;
            HasUnsavedChanges = false;

            if (string.IsNullOrWhiteSpace(Project.FolderPath)) {
                return StoreResult(Result.Fail("Project folder path is empty."));
            }

            if (!Directory.Exists(Project.FolderPath)) {
                return StoreResult(Result.Fail($"Project folder does not exist: {Project.FolderPath}"));
            }

            return Result.Try(() => {
                var stoppedAtFileLimit = false;
                foreach (var file in EnumerateProjectFiles(Project.FolderPath)) {
                    if (Files.Count >= MaxProjectFileCount) {
                        stoppedAtFileLimit = true;
                        break;
                    }

                    Files.Add(file);
                    if (file.IsTextFile) {
                        EditableFiles.Add(file);
                    }
                }

                RefreshFilteredEditableFiles();
                RefreshAssetGroups();
                RefreshGameAssetGroups();
                StatusMessage = stoppedAtFileLimit
                    ? $"Stopped after loading {MaxProjectFileCount:N0} files."
                    : string.Empty;

                return Result.Ok();
            });
        }

        public Result LoadSelectedTextFile() {
            if (SelectedFile is null) {
                return Result.Fail("No file is selected.");
            }

            return LoadTextFile(SelectedFile);
        }

        public void UnloadEditor() {
            SetEditorContent(string.Empty, hasUnsavedChanges: false);
        }

        public Result LoadTextFile(ProjectWorkspaceFile file) {
            UnloadEditor();

            if (!file.IsTextFile) {
                return Result.Fail("Selected file is not a supported text file.");
            }

            if (file.SizeBytes > MaxTextFileSizeBytes) {
                return Result.Fail($"Selected file is larger than {MaxTextFileSizeBytes:N0} bytes.");
            }

            if (!File.Exists(file.FullPath)) {
                return Result.Fail($"Selected file does not exist: {file.FullPath}");
            }

            return Result.Try(() => {
                SelectedFile = file;
                SetEditorContent(File.ReadAllText(file.FullPath, Encoding.UTF8), hasUnsavedChanges: false);

                return Result.Ok();
            });
        }

        public Result SaveSelectedTextContent() {
            if (SelectedFile is null) {
                return Result.Fail("No file is selected.");
            }

            if (!SelectedFile.IsTextFile) {
                return Result.Fail("Selected file is not a supported text file.");
            }

            return Result.Try(() => {
                File.WriteAllText(SelectedFile.FullPath, SelectedFileContent, Encoding.UTF8);

                var previousFile = SelectedFile;
                var fileInfo = new FileInfo(SelectedFile.FullPath);
                var updatedFile = ProjectWorkspaceFile.FromFileInfo(Project.FolderPath, fileInfo, true);
                ReplaceFileEntry(Files, previousFile, updatedFile);
                ReplaceFileEntry(EditableFiles, previousFile, updatedFile);
                ReplaceFileEntry(FilteredEditableFiles, previousFile, updatedFile);
                SelectedFile = updatedFile;
                HasUnsavedChanges = false;

                return Result.Ok();
            });
        }

        private static IEnumerable<ProjectWorkspaceFile> EnumerateProjectFiles(string projectFolderPath) {
            var pendingDirectories = new Stack<string>();
            pendingDirectories.Push(projectFolderPath);

            while (pendingDirectories.Count > 0) {
                var directoryPath = pendingDirectories.Pop();

                foreach (var childDirectoryPath in EnumerateDirectories(directoryPath)) {
                    var directoryName = Path.GetFileName(childDirectoryPath);
                    if (!ExcludedDirectoryNames.Contains(directoryName)) {
                        pendingDirectories.Push(childDirectoryPath);
                    }
                }

                foreach (var filePath in EnumerateFiles(directoryPath)) {
                    var file = TryCreateWorkspaceFile(projectFolderPath, filePath);
                    if (file is not null) {
                        yield return file;
                    }
                }
            }
        }

        private static IEnumerable<string> EnumerateDirectories(string directoryPath) {
            try {
                return Directory
                    .EnumerateDirectories(directoryPath)
                    .Where(IsSafeDirectory)
                    .OrderBy(path => path)
                    .ToArray();
            }
            catch (IOException) {
                return [];
            }
            catch (UnauthorizedAccessException) {
                return [];
            }
        }

        private static IEnumerable<string> EnumerateFiles(string directoryPath) {
            try {
                return Directory.EnumerateFiles(directoryPath).OrderBy(path => path).ToArray();
            }
            catch (IOException) {
                return [];
            }
            catch (UnauthorizedAccessException) {
                return [];
            }
        }

        private static bool IsSafeDirectory(string directoryPath) {
            try {
                var attributes = File.GetAttributes(directoryPath);
                return !attributes.HasFlag(FileAttributes.ReparsePoint)
                    && !attributes.HasFlag(FileAttributes.System);
            }
            catch (IOException) {
                return false;
            }
            catch (UnauthorizedAccessException) {
                return false;
            }
        }

        private static ProjectWorkspaceFile? TryCreateWorkspaceFile(string projectFolderPath, string filePath) {
            try {
                var fileInfo = new FileInfo(filePath);
                var isTextFile = IsSupportedTextFile(fileInfo);

                return ProjectWorkspaceFile.FromFileInfo(projectFolderPath, fileInfo, isTextFile);
            }
            catch (IOException) {
                return null;
            }
            catch (UnauthorizedAccessException) {
                return null;
            }
        }

        private static bool IsSupportedTextFile(FileInfo fileInfo) {
            if (!TextFileExtensions.Contains(fileInfo.Extension)) {
                return false;
            }

            if (fileInfo.Length > MaxTextFileSizeBytes) {
                return false;
            }

            try {
                using var stream = File.OpenRead(fileInfo.FullName);
                Span<byte> buffer = stackalloc byte[512];
                var bytesRead = stream.Read(buffer);

                return !buffer[..bytesRead].Contains((byte)0);
            }
            catch (IOException) {
                return false;
            }
            catch (UnauthorizedAccessException) {
                return false;
            }
        }

        private bool CanLoadOrSaveSelectedFile() {
            return SelectedFile?.IsTextFile == true;
        }

        private void RefreshFilteredEditableFiles() {
            FilteredEditableFiles.Clear();

            var filteredFiles = EditableFiles.AsEnumerable();
            if (SelectedFileGroup is not null) {
                filteredFiles = filteredFiles.Where(SelectedFileGroup.Includes);
            }

            if (!string.IsNullOrWhiteSpace(FileSearchText)) {
                filteredFiles = filteredFiles.Where(file =>
                    file.RelativePath.Contains(FileSearchText, StringComparison.OrdinalIgnoreCase));
            }

            foreach (var file in filteredFiles) {
                FilteredEditableFiles.Add(file);
            }

            if (SelectedFile?.IsTextFile == true && !FilteredEditableFiles.Contains(SelectedFile)) {
                SelectedFile = null;
                UnloadEditor();
            }

            OnPropertyChanged(nameof(SelectedFileStatusText));
        }

        private static void ReplaceFileEntry(
            ObservableCollection<ProjectWorkspaceFile> files,
            ProjectWorkspaceFile previousFile,
            ProjectWorkspaceFile updatedFile
        ) {
            var index = files.IndexOf(previousFile);
            if (index >= 0) {
                files[index] = updatedFile;
            }
        }

        private void RefreshAssetGroups() {
            foreach (var assetArea in AssetAreas) {
                var entries = new ObservableCollection<ProjectWorkspaceFile>();
                foreach (var file in Files.Where(file =>
                    !file.IsTextFile && IsAssetAreaFile(file, assetArea.DirectoryName))) {
                    entries.Add(file);
                }

                AssetGroups.Add(new ModAssetGroup(assetArea.DirectoryName, assetArea.DisplayName, entries));
            }

            SelectedAssetGroup = AssetGroups.FirstOrDefault(group => group.Entries.Count > 0)
                ?? AssetGroups.FirstOrDefault();
        }

        private void RefreshGameAssetGroups() {
            GameAssetStatusText = string.Empty;

            var gameRootPath = SettingsRepository.Instance.CurrentSettings.GameRootPath;
            if (string.IsNullOrWhiteSpace(gameRootPath)) {
                AddEmptyGameAssetGroups();
                GameAssetStatusText = "Set the Hearts of Iron IV folder in Settings to browse original assets.";
                return;
            }

            if (!Directory.Exists(gameRootPath)) {
                AddEmptyGameAssetGroups();
                GameAssetStatusText = $"Game folder does not exist: {gameRootPath}";
                return;
            }

            var entriesByArea = AssetAreas.ToDictionary(
                assetArea => assetArea.DirectoryName,
                _ => new ObservableCollection<ProjectWorkspaceFile>(),
                StringComparer.OrdinalIgnoreCase
            );
            var loadedAssetCount = 0;

            foreach (var file in EnumerateProjectFiles(gameRootPath)) {
                if (loadedAssetCount >= MaxProjectFileCount) {
                    break;
                }

                if (file.IsTextFile || TryGetAssetAreaName(file) is not string areaName) {
                    continue;
                }

                if (entriesByArea.TryGetValue(areaName, out var entries)) {
                    entries.Add(file);
                    loadedAssetCount++;
                }
            }

            foreach (var assetArea in AssetAreas) {
                GameAssetGroups.Add(new ModAssetGroup(
                    assetArea.DirectoryName,
                    assetArea.DisplayName,
                    entriesByArea[assetArea.DirectoryName]
                ));
            }

            if (loadedAssetCount >= MaxProjectFileCount) {
                GameAssetStatusText = $"Stopped after loading {MaxProjectFileCount:N0} game assets.";
            }

            SelectedGameAssetGroup = GameAssetGroups.FirstOrDefault(group => group.Entries.Count > 0)
                ?? GameAssetGroups.FirstOrDefault();
        }

        private void AddEmptyGameAssetGroups() {
            foreach (var assetArea in AssetAreas) {
                GameAssetGroups.Add(new ModAssetGroup(
                    assetArea.DirectoryName,
                    assetArea.DisplayName,
                    []
                ));
            }

            SelectedGameAssetGroup = GameAssetGroups.FirstOrDefault();
        }

        private static bool IsAssetAreaFile(ProjectWorkspaceFile file, string areaName) {
            return string.Equals(TryGetAssetAreaName(file), areaName, StringComparison.OrdinalIgnoreCase);
        }

        private static string? TryGetAssetAreaName(ProjectWorkspaceFile file) {
            var firstSegment = file.RelativePath
                .Split([Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar], StringSplitOptions.RemoveEmptyEntries)
                .FirstOrDefault();

            return firstSegment;
        }

        private static string NormalizeRelativePath(string relativePath) {
            return relativePath.Replace(Path.AltDirectorySeparatorChar, Path.DirectorySeparatorChar);
        }

        private void SetEditorContent(string content, bool hasUnsavedChanges) {
            isSettingEditorContent = true;
            try {
                SetProperty(ref selectedFileContent, content, nameof(SelectedFileContent));
                HasUnsavedChanges = hasUnsavedChanges;
            }
            finally {
                isSettingEditorContent = false;
            }
        }

        private Result StoreResult(Result result) {
            StatusMessage = result.IsSuccess
                ? string.Empty
                : result.Errors.FirstOrDefault()?.Message ?? "Operation failed.";

            return result;
        }

        private record ModAssetArea(string DirectoryName, string DisplayName);
    }
}
