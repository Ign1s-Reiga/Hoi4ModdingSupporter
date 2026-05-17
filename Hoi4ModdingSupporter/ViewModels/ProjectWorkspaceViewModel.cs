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

        private ProjectWorkspaceFile? selectedFile;
        private ModAssetGroup? selectedAssetGroup;
        private string selectedFileContent = string.Empty;
        private string statusMessage = string.Empty;
        private bool hasUnsavedChanges;

        public ProjectWorkspaceViewModel(RecentProjectRecord project) {
            Project = project;

            RefreshFilesCommand = new RelayCommand(() => {
                RefreshFiles();
            });
            LoadSelectedFileCommand = new RelayCommand(
                () => StoreResult(LoadSelectedTextFile()),
                CanLoadOrSaveSelectedFile
            );
            SaveSelectedFileCommand = new RelayCommand(
                () => StoreResult(SaveSelectedTextContent()),
                CanLoadOrSaveSelectedFile
            );

            RefreshFiles();
        }

        public RecentProjectRecord Project { get; }

        public ObservableCollection<ProjectWorkspaceFile> Files { get; } = [];

        public ObservableCollection<ModAssetGroup> AssetGroups { get; } = [];

        public ModAssetGroup? SelectedAssetGroup {
            get => selectedAssetGroup;
            set {
                if (SetProperty(ref selectedAssetGroup, value)) {
                    OnPropertyChanged(nameof(SelectedAssetEntries));
                }
            }
        }

        public ObservableCollection<ProjectWorkspaceFile> SelectedAssetEntries => SelectedAssetGroup?.Entries ?? [];

        public ProjectWorkspaceFile? SelectedFile {
            get => selectedFile;
            set {
                if (SetProperty(ref selectedFile, value)) {
                    LoadSelectedFileCommand.NotifyCanExecuteChanged();
                    SaveSelectedFileCommand.NotifyCanExecuteChanged();

                    if (value?.IsTextFile == false) {
                        UnloadEditor();
                    }
                }
            }
        }

        public string SelectedFileContent {
            get => selectedFileContent;
            set {
                if (SetProperty(ref selectedFileContent, value)) {
                    HasUnsavedChanges = SelectedFile is not null;
                }
            }
        }

        public string StatusMessage {
            get => statusMessage;
            private set => SetProperty(ref statusMessage, value);
        }

        public bool HasUnsavedChanges {
            get => hasUnsavedChanges;
            private set => SetProperty(ref hasUnsavedChanges, value);
        }

        public IRelayCommand RefreshFilesCommand { get; }

        public IRelayCommand LoadSelectedFileCommand { get; }

        public IRelayCommand SaveSelectedFileCommand { get; }

        public Result RefreshFiles() {
            Files.Clear();
            AssetGroups.Clear();
            SelectedAssetGroup = null;
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
                }

                RefreshAssetGroups();
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

                var fileInfo = new FileInfo(SelectedFile.FullPath);
                SelectedFile = ProjectWorkspaceFile.FromFileInfo(Project.FolderPath, fileInfo, true);
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

        private void RefreshAssetGroups() {
            foreach (var assetArea in AssetAreas) {
                var entries = new ObservableCollection<ProjectWorkspaceFile>();
                foreach (var file in Files.Where(file => IsAssetAreaFile(file, assetArea.DirectoryName))) {
                    entries.Add(file);
                }

                AssetGroups.Add(new ModAssetGroup(assetArea.DirectoryName, assetArea.DisplayName, entries));
            }

            SelectedAssetGroup = AssetGroups.FirstOrDefault(group => group.Entries.Count > 0)
                ?? AssetGroups.FirstOrDefault();
        }

        private static bool IsAssetAreaFile(ProjectWorkspaceFile file, string areaName) {
            var firstSegment = file.RelativePath
                .Split([Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar], StringSplitOptions.RemoveEmptyEntries)
                .FirstOrDefault();

            return string.Equals(firstSegment, areaName, StringComparison.OrdinalIgnoreCase);
        }

        private void SetEditorContent(string content, bool hasUnsavedChanges) {
            SetProperty(ref selectedFileContent, content, nameof(SelectedFileContent));
            HasUnsavedChanges = hasUnsavedChanges;
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
