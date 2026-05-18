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
using Microsoft.UI.Xaml;

namespace Hoi4ModdingSupporter.ViewModels {
    public class ProjectWorkspaceViewModel : ObservableObject {
        private const int MaxProjectFileCount = 5000;
        private const long MaxTextFileSizeBytes = 1024 * 1024;
        private const string NationalFocusPathPrefix = "common\\national_focus";

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
        private ProjectWorkspaceFile? selectedNationalFocusFile;
        private NationalFocusEntry? selectedNationalFocus;
        private string fileSearchText = string.Empty;
        private string nationalFocusId = string.Empty;
        private string nationalFocusIcon = string.Empty;
        private string nationalFocusX = string.Empty;
        private string nationalFocusY = string.Empty;
        private string nationalFocusCost = string.Empty;
        private string nationalFocusPrerequisite = string.Empty;
        private string nationalFocusMutuallyExclusive = string.Empty;
        private string nationalFocusRelativePositionId = string.Empty;
        private string nationalFocusEffects = string.Empty;
        private string selectedFileContent = string.Empty;
        private string gameAssetStatusText = string.Empty;
        private string statusMessage = string.Empty;
        private bool hasUnsavedChanges;
        private bool hasUnsavedNationalFocusChanges;
        private bool isSettingEditorContent;
        private bool isSettingNationalFocusEditorContent;
        private WorkspaceSection currentSection = WorkspaceSection.VisualEditor;

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
            SaveNationalFocusCommand = new RelayCommand(
                () => StoreResult(SaveSelectedNationalFocus()),
                CanSaveSelectedNationalFocus
            );

            foreach (var scriptGroup in ScriptGroups) {
                FileGroups.Add(scriptGroup);
            }

            selectedFileGroup = FileGroups.FirstOrDefault();

            RefreshFiles();
        }

        public RecentProjectRecord Project { get; }

        public WorkspaceSection CurrentSection {
            get => currentSection;
            set {
                if (SetProperty(ref currentSection, value)) {
                    OnPropertyChanged(nameof(VisualEditorVisibility));
                    OnPropertyChanged(nameof(NationalFocusVisibility));
                    OnPropertyChanged(nameof(ModAssetsVisibility));
                    OnPropertyChanged(nameof(GameAssetsVisibility));
                    OnPropertyChanged(nameof(NationalFocusEditorVisibility));
                    OnPropertyChanged(nameof(WorkspaceSectionTitle));
                }
            }
        }

        public Visibility VisualEditorVisibility => CurrentSection == WorkspaceSection.VisualEditor
            ? Visibility.Visible
            : Visibility.Collapsed;

        public Visibility NationalFocusVisibility => CurrentSection == WorkspaceSection.NationalFocus
            ? Visibility.Visible
            : Visibility.Collapsed;

        public Visibility ModAssetsVisibility => CurrentSection == WorkspaceSection.ModAssets
            ? Visibility.Visible
            : Visibility.Collapsed;

        public Visibility GameAssetsVisibility => CurrentSection == WorkspaceSection.GameAssets
            ? Visibility.Visible
            : Visibility.Collapsed;

        public string WorkspaceSectionTitle => CurrentSection switch {
            WorkspaceSection.NationalFocus => "National Focus",
            WorkspaceSection.ModAssets => "Mod Assets",
            WorkspaceSection.GameAssets => "Game Assets",
            _ => "Visual Mod Editor"
        };

        public ObservableCollection<ProjectWorkspaceFile> Files { get; } = [];

        public ObservableCollection<ProjectWorkspaceFile> NationalFocusFiles { get; } = [];

        public ObservableCollection<NationalFocusEntry> NationalFocuses { get; } = [];

        public ObservableCollection<NationalFocusEntry> VisibleNationalFocuses { get; } = [];

        public ObservableCollection<string> NationalFocusIdOptions { get; } = [];

        public ProjectWorkspaceFile? SelectedNationalFocusFile {
            get => selectedNationalFocusFile;
            set {
                if (SetProperty(ref selectedNationalFocusFile, value)) {
                    OnPropertyChanged(nameof(SelectedNationalFocusFileTitle));
                    RefreshVisibleNationalFocuses();
                }
            }
        }

        public string SelectedNationalFocusFileTitle => SelectedNationalFocusFile?.RelativePath ?? "Select a focus file";

        public NationalFocusEntry? SelectedNationalFocus {
            get => selectedNationalFocus;
            set {
                if (SetProperty(ref selectedNationalFocus, value)) {
                    LoadNationalFocusEditor(value);
                    SaveNationalFocusCommand.NotifyCanExecuteChanged();
                }
            }
        }

        public string NationalFocusId {
            get => nationalFocusId;
            set {
                if (SetProperty(ref nationalFocusId, value)) {
                    MarkNationalFocusEditorChanged();
                    SaveNationalFocusCommand.NotifyCanExecuteChanged();
                }
            }
        }

        public string NationalFocusIcon {
            get => nationalFocusIcon;
            set {
                if (SetProperty(ref nationalFocusIcon, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusX {
            get => nationalFocusX;
            set {
                if (SetProperty(ref nationalFocusX, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusY {
            get => nationalFocusY;
            set {
                if (SetProperty(ref nationalFocusY, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusCost {
            get => nationalFocusCost;
            set {
                if (SetProperty(ref nationalFocusCost, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusPrerequisite {
            get => nationalFocusPrerequisite;
            set {
                if (SetProperty(ref nationalFocusPrerequisite, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusMutuallyExclusive {
            get => nationalFocusMutuallyExclusive;
            set {
                if (SetProperty(ref nationalFocusMutuallyExclusive, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusRelativePositionId {
            get => nationalFocusRelativePositionId;
            set {
                if (SetProperty(ref nationalFocusRelativePositionId, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public string NationalFocusEffects {
            get => nationalFocusEffects;
            set {
                if (SetProperty(ref nationalFocusEffects, value)) {
                    MarkNationalFocusEditorChanged();
                }
            }
        }

        public Visibility NationalFocusEditorVisibility => CurrentSection == WorkspaceSection.NationalFocus
            ? Visibility.Visible
            : Visibility.Collapsed;

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
            get => hasUnsavedChanges || hasUnsavedNationalFocusChanges;
            private set {
                SetRawTextUnsavedChanges(value);
            }
        }

        public bool HasUnsavedNationalFocusChanges => hasUnsavedNationalFocusChanges;

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

        public IRelayCommand SaveNationalFocusCommand { get; }

        public Result RefreshFiles() {
            Files.Clear();
            NationalFocusFiles.Clear();
            NationalFocuses.Clear();
            VisibleNationalFocuses.Clear();
            NationalFocusIdOptions.Clear();
            EditableFiles.Clear();
            FilteredEditableFiles.Clear();
            AssetGroups.Clear();
            GameAssetGroups.Clear();
            SelectedNationalFocusFile = null;
            SelectedNationalFocus = null;
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
                RefreshNationalFocuses();
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

        public Result SelectFile(ProjectWorkspaceFile file) {
            if (file.IsTextFile) {
                return LoadTextFile(file);
            }

            SelectedFile = file;
            UnloadEditor();

            return Result.Ok();
        }

        public void DiscardSelectedFileChanges() {
            if (SelectedFile?.IsTextFile == true) {
                var result = LoadTextFile(SelectedFile);
                if (result.IsSuccess) {
                    return;
                }
            }

            UnloadEditor();
        }

        public void DiscardPendingChanges() {
            DiscardSelectedFileChanges();
            SetNationalFocusUnsavedChanges(false);
            LoadNationalFocusEditor(SelectedNationalFocus);
        }

        public Result SavePendingChanges() {
            if (hasUnsavedChanges) {
                var result = SaveSelectedTextContent();
                if (result.IsFailed) {
                    return result;
                }
            }

            return hasUnsavedNationalFocusChanges
                ? SaveSelectedNationalFocus()
                : Result.Ok();
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

        public Result SaveSelectedNationalFocus() {
            if (SelectedNationalFocus is null) {
                return Result.Fail("No national focus is selected.");
            }

            if (string.IsNullOrWhiteSpace(NationalFocusId)) {
                return Result.Fail("National focus id is required.");
            }

            if (hasUnsavedChanges
                && SelectedFile is not null
                && string.Equals(SelectedFile.FullPath, SelectedNationalFocus.SourceFile.FullPath, StringComparison.OrdinalIgnoreCase)) {
                return Result.Fail("Save or discard raw text changes before saving the no-code focus editor.");
            }

            return Result.Try(() => {
                var sourceFile = SelectedNationalFocus.SourceFile;
                var lines = File.ReadAllLines(sourceFile.FullPath, Encoding.UTF8).ToList();
                var blockEndLine = Math.Min(SelectedNationalFocus.BlockEndLine, lines.Count - 1);

                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "id", NationalFocusId, quoteValue: true);
                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "icon", NationalFocusIcon, quoteValue: true);
                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "x", NationalFocusX, quoteValue: false);
                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "y", NationalFocusY, quoteValue: false);
                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "cost", NationalFocusCost, quoteValue: false);
                blockEndLine = SetFocusReferenceBlock(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "prerequisite", NationalFocusPrerequisite);
                blockEndLine = SetFocusReferenceBlock(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "mutually_exclusive", NationalFocusMutuallyExclusive);
                blockEndLine = SetFocusAssignment(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "relative_position_id", NationalFocusRelativePositionId, quoteValue: false);
                SetFocusBlock(lines, SelectedNationalFocus.BlockStartLine, blockEndLine, "completion_reward", NationalFocusEffects);

                File.WriteAllLines(sourceFile.FullPath, lines, Encoding.UTF8);
                SetNationalFocusUnsavedChanges(false);
                var updatedSourceFile = ProjectWorkspaceFile.FromFileInfo(Project.FolderPath, new FileInfo(sourceFile.FullPath), true);
                ReplaceFileEntry(Files, sourceFile, updatedSourceFile);
                ReplaceFileEntry(EditableFiles, sourceFile, updatedSourceFile);
                ReplaceFileEntry(FilteredEditableFiles, sourceFile, updatedSourceFile);

                var selectedId = NationalFocusId;
                RefreshNationalFocuses();
                SelectedNationalFocusFile = NationalFocusFiles.FirstOrDefault(file =>
                    string.Equals(file.FullPath, sourceFile.FullPath, StringComparison.OrdinalIgnoreCase)
                );
                SelectedNationalFocus = NationalFocuses.FirstOrDefault(focus =>
                    string.Equals(focus.SourceFile.FullPath, sourceFile.FullPath, StringComparison.OrdinalIgnoreCase)
                    && string.Equals(focus.Id, selectedId, StringComparison.OrdinalIgnoreCase)
                );

                if (SelectedFile is not null
                    && string.Equals(SelectedFile.FullPath, sourceFile.FullPath, StringComparison.OrdinalIgnoreCase)) {
                    LoadTextFile(SelectedFile);
                }

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

        private bool CanSaveSelectedNationalFocus() {
            return SelectedNationalFocus is not null && !string.IsNullOrWhiteSpace(NationalFocusId);
        }

        private void RefreshNationalFocuses() {
            NationalFocuses.Clear();
            NationalFocusFiles.Clear();
            VisibleNationalFocuses.Clear();
            NationalFocusIdOptions.Clear();

            foreach (var file in EditableFiles.Where(IsNationalFocusFile)) {
                var focuses = ReadNationalFocuses(file).ToArray();
                if (focuses.Length == 0) {
                    continue;
                }

                NationalFocusFiles.Add(file);
                foreach (var focus in focuses) {
                    NationalFocuses.Add(focus);
                }
            }

            NationalFocusIdOptions.Add(string.Empty);
            foreach (var focusId in NationalFocuses
                .Select(focus => focus.Id)
                .Where(id => !string.IsNullOrWhiteSpace(id))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(id => id, StringComparer.OrdinalIgnoreCase)) {
                NationalFocusIdOptions.Add(focusId);
            }

            SelectedNationalFocusFile = NationalFocusFiles.FirstOrDefault();
        }

        private void RefreshVisibleNationalFocuses() {
            VisibleNationalFocuses.Clear();

            var focuses = NationalFocuses.AsEnumerable();
            if (SelectedNationalFocusFile is not null) {
                focuses = focuses.Where(focus =>
                    string.Equals(
                        focus.SourceFile.FullPath,
                        SelectedNationalFocusFile.FullPath,
                        StringComparison.OrdinalIgnoreCase
                    )
                );
            }

            foreach (var focus in focuses) {
                VisibleNationalFocuses.Add(focus);
            }

            SelectedNationalFocus = VisibleNationalFocuses.FirstOrDefault();
        }

        private static bool IsNationalFocusFile(ProjectWorkspaceFile file) {
            return file.RelativePath.StartsWith(NationalFocusPathPrefix, StringComparison.OrdinalIgnoreCase)
                && string.Equals(file.Extension, ".txt", StringComparison.OrdinalIgnoreCase);
        }

        private static IEnumerable<NationalFocusEntry> ReadNationalFocuses(ProjectWorkspaceFile file) {
            string[] lines;
            try {
                lines = File.ReadAllLines(file.FullPath, Encoding.UTF8);
            }
            catch (IOException) {
                yield break;
            }
            catch (UnauthorizedAccessException) {
                yield break;
            }

            for (var lineIndex = 0; lineIndex < lines.Length; lineIndex++) {
                if (!IsFocusStartLine(lines[lineIndex])) {
                    continue;
                }

                var endLine = FindBlockEndLine(lines, lineIndex);
                if (endLine <= lineIndex) {
                    continue;
                }

                yield return new NationalFocusEntry(
                    file,
                    lineIndex,
                    endLine,
                    ReadAssignment(lines, lineIndex, endLine, "id"),
                    ReadAssignment(lines, lineIndex, endLine, "icon"),
                    ReadAssignment(lines, lineIndex, endLine, "x"),
                    ReadAssignment(lines, lineIndex, endLine, "y"),
                    ReadAssignment(lines, lineIndex, endLine, "cost"),
                    ReadFocusReferenceBlock(lines, lineIndex, endLine, "prerequisite"),
                    ReadFocusReferenceBlock(lines, lineIndex, endLine, "mutually_exclusive"),
                    ReadAssignment(lines, lineIndex, endLine, "relative_position_id"),
                    ReadBlockContent(lines, lineIndex, endLine, "completion_reward")
                );

                lineIndex = endLine;
            }
        }

        private static bool IsFocusStartLine(string line) {
            var uncommented = RemoveLineComment(line).Trim();
            var compact = string.Concat(uncommented.Where(character => !char.IsWhiteSpace(character)));
            return compact.Equals("focus={", StringComparison.OrdinalIgnoreCase);
        }

        private static int FindBlockEndLine(string[] lines, int startLine) {
            var depth = 0;
            for (var lineIndex = startLine; lineIndex < lines.Length; lineIndex++) {
                var line = RemoveLineComment(lines[lineIndex]);
                depth += line.Count(character => character == '{');
                depth -= line.Count(character => character == '}');

                if (depth <= 0 && (lineIndex > startLine || line.Contains('{', StringComparison.Ordinal))) {
                    return lineIndex;
                }
            }

            return -1;
        }

        private static string ReadAssignment(string[] lines, int startLine, int endLine, string key) {
            var prefix = key + "=";
            var spacedPrefix = key + " =";
            var depth = 1;

            for (var lineIndex = startLine + 1; lineIndex < endLine; lineIndex++) {
                var uncommented = RemoveLineComment(lines[lineIndex]);
                var line = uncommented.Trim();
                if (depth == 1
                    && (line.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)
                        || line.StartsWith(spacedPrefix, StringComparison.OrdinalIgnoreCase))) {
                    var separatorIndex = line.IndexOf('=');
                    if (separatorIndex < 0) {
                        continue;
                    }

                    return line[(separatorIndex + 1)..].Trim().Trim('"');
                }

                depth += uncommented.Count(character => character == '{');
                depth -= uncommented.Count(character => character == '}');
            }

            return string.Empty;
        }

        private static string ReadBlockContent(string[] lines, int startLine, int endLine, string key) {
            var blockStartLine = FindTopLevelBlockStartLine(lines.ToArray(), startLine, endLine, key);
            if (blockStartLine < 0) {
                return string.Empty;
            }

            var blockEndLine = FindBlockEndLine(lines, blockStartLine);
            if (blockEndLine < blockStartLine) {
                return string.Empty;
            }

            if (blockEndLine == blockStartLine) {
                return ReadInlineBlockContent(lines[blockStartLine]);
            }

            var blockLines = lines
                .Skip(blockStartLine + 1)
                .Take(blockEndLine - blockStartLine - 1)
                .ToArray();

            return string.Join(Environment.NewLine, TrimSharedIndentation(blockLines)).Trim();
        }

        private static string ReadInlineBlockContent(string line) {
            var uncommented = RemoveLineComment(line);
            var openBraceIndex = uncommented.IndexOf('{');
            var closeBraceIndex = uncommented.LastIndexOf('}');

            return openBraceIndex >= 0 && closeBraceIndex > openBraceIndex
                ? uncommented[(openBraceIndex + 1)..closeBraceIndex].Trim()
                : string.Empty;
        }

        private static string ReadFocusReferenceBlock(string[] lines, int startLine, int endLine, string key) {
            var blockStartLine = FindTopLevelBlockStartLine(lines, startLine, endLine, key);
            if (blockStartLine < 0) {
                return string.Empty;
            }

            var blockEndLine = FindBlockEndLine(lines, blockStartLine);
            if (blockEndLine < blockStartLine) {
                return string.Empty;
            }

            if (blockEndLine == blockStartLine) {
                return ReadInlineAssignment(ReadInlineBlockContent(lines[blockStartLine]), "focus");
            }

            return ReadAssignment(lines, blockStartLine, blockEndLine, "focus");
        }

        private static string ReadInlineAssignment(string content, string key) {
            var separatorIndex = content.IndexOf('=');
            if (separatorIndex < 0) {
                return string.Empty;
            }

            var propertyName = content[..separatorIndex].Trim();
            if (!propertyName.Equals(key, StringComparison.OrdinalIgnoreCase)) {
                return string.Empty;
            }

            return content[(separatorIndex + 1)..].Trim().Trim('"');
        }

        private static int SetFocusAssignment(
            List<string> lines,
            int startLine,
            int endLine,
            string key,
            string value,
            bool quoteValue
        ) {
            var replacement = string.IsNullOrWhiteSpace(value)
                ? string.Empty
                : $"{key} = {(quoteValue ? $"\"{value.Trim().Trim('"')}\"" : value.Trim())}";
            var depth = 1;

            for (var lineIndex = startLine + 1; lineIndex < endLine; lineIndex++) {
                var uncommented = RemoveLineComment(lines[lineIndex]);
                var trimmed = uncommented.TrimStart();
                if (depth == 1
                    && (trimmed.StartsWith(key + "=", StringComparison.OrdinalIgnoreCase)
                        || trimmed.StartsWith(key + " =", StringComparison.OrdinalIgnoreCase))) {
                    if (string.IsNullOrEmpty(replacement)) {
                        lines.RemoveAt(lineIndex);
                        return endLine - 1;
                    }
                    else {
                        var indentation = lines[lineIndex][..(lines[lineIndex].Length - lines[lineIndex].TrimStart().Length)];
                        lines[lineIndex] = indentation + replacement;
                        return endLine;
                    }
                }

                depth += uncommented.Count(character => character == '{');
                depth -= uncommented.Count(character => character == '}');
            }

            if (!string.IsNullOrEmpty(replacement)) {
                lines.Insert(endLine, "\t\t" + replacement);
                return endLine + 1;
            }

            return endLine;
        }

        private static int SetFocusBlock(
            List<string> lines,
            int startLine,
            int endLine,
            string key,
            string value
        ) {
            var blockStartLine = FindTopLevelBlockStartLine(lines.ToArray(), startLine, endLine, key);
            var normalizedLines = value
                .Replace("\r\n", "\n", StringComparison.Ordinal)
                .Replace('\r', '\n')
                .Split('\n')
                .Select(line => line.TrimEnd())
                .Where(line => !string.IsNullOrWhiteSpace(line))
                .ToArray();

            if (blockStartLine >= 0) {
                var blockEndLine = FindBlockEndLine(lines.ToArray(), blockStartLine);
                if (blockEndLine < blockStartLine || blockEndLine > endLine) {
                    return endLine;
                }

                if (normalizedLines.Length == 0) {
                    lines.RemoveRange(blockStartLine, blockEndLine - blockStartLine + 1);
                    return endLine - (blockEndLine - blockStartLine + 1);
                }

                var replacement = BuildFocusBlockLines(lines[blockStartLine], key, normalizedLines);
                lines.RemoveRange(blockStartLine, blockEndLine - blockStartLine + 1);
                lines.InsertRange(blockStartLine, replacement);
                return endLine + replacement.Length - (blockEndLine - blockStartLine + 1);
            }

            if (normalizedLines.Length == 0) {
                return endLine;
            }

            lines.InsertRange(endLine, BuildFocusBlockLines("\t\t" + key + " = {", key, normalizedLines));
            return endLine + normalizedLines.Length + 2;
        }

        private static int SetFocusReferenceBlock(
            List<string> lines,
            int startLine,
            int endLine,
            string key,
            string focusId
        ) {
            var blockStartLine = FindTopLevelBlockStartLine(lines.ToArray(), startLine, endLine, key);
            var normalizedFocusId = focusId.Trim().Trim('"');

            if (blockStartLine >= 0) {
                var blockEndLine = FindBlockEndLine(lines.ToArray(), blockStartLine);
                if (blockEndLine < blockStartLine || blockEndLine > endLine) {
                    return endLine;
                }

                if (string.IsNullOrWhiteSpace(normalizedFocusId)) {
                    lines.RemoveRange(blockStartLine, blockEndLine - blockStartLine + 1);
                    return endLine - (blockEndLine - blockStartLine + 1);
                }

                var replacement = BuildFocusReferenceBlockLines(lines[blockStartLine], key, normalizedFocusId);
                lines.RemoveRange(blockStartLine, blockEndLine - blockStartLine + 1);
                lines.InsertRange(blockStartLine, replacement);
                return endLine + replacement.Length - (blockEndLine - blockStartLine + 1);
            }

            if (string.IsNullOrWhiteSpace(normalizedFocusId)) {
                return endLine;
            }

            lines.InsertRange(endLine, BuildFocusReferenceBlockLines("\t\t" + key + " = {", key, normalizedFocusId));
            return endLine + 3;
        }

        private static int FindTopLevelBlockStartLine(string[] lines, int startLine, int endLine, string key) {
            var depth = 1;

            for (var lineIndex = startLine + 1; lineIndex < endLine; lineIndex++) {
                var uncommented = RemoveLineComment(lines[lineIndex]);
                var trimmed = uncommented.TrimStart();
                if (depth == 1
                    && (trimmed.StartsWith(key + "=", StringComparison.OrdinalIgnoreCase)
                        || trimmed.StartsWith(key + " =", StringComparison.OrdinalIgnoreCase))
                    && trimmed.Contains('{', StringComparison.Ordinal)) {
                    return lineIndex;
                }

                depth += uncommented.Count(character => character == '{');
                depth -= uncommented.Count(character => character == '}');
            }

            return -1;
        }

        private static string[] BuildFocusBlockLines(string existingStartLine, string key, string[] blockLines) {
            var indentation = existingStartLine[..(existingStartLine.Length - existingStartLine.TrimStart().Length)];
            var childIndentation = indentation + "\t";
            var output = new List<string> {
                indentation + key + " = {"
            };

            output.AddRange(blockLines.Select(line => childIndentation + line.TrimEnd()));
            output.Add(indentation + "}");

            return output.ToArray();
        }

        private static string[] BuildFocusReferenceBlockLines(string existingStartLine, string key, string focusId) {
            var indentation = existingStartLine[..(existingStartLine.Length - existingStartLine.TrimStart().Length)];
            var childIndentation = indentation + "\t";

            return [
                indentation + key + " = {",
                childIndentation + "focus = " + focusId,
                indentation + "}"
            ];
        }

        private static string[] TrimSharedIndentation(string[] lines) {
            var indentationLength = lines
                .Where(line => !string.IsNullOrWhiteSpace(line))
                .Select(line => line.Length - line.TrimStart().Length)
                .DefaultIfEmpty(0)
                .Min();

            return lines
                .Select(line => line.Length >= indentationLength ? line[indentationLength..] : line)
                .ToArray();
        }

        private static string RemoveLineComment(string line) {
            var commentIndex = line.IndexOf('#');
            return commentIndex >= 0 ? line[..commentIndex] : line;
        }

        private void LoadNationalFocusEditor(NationalFocusEntry? focus) {
            isSettingNationalFocusEditorContent = true;
            try {
                NationalFocusId = focus?.Id ?? string.Empty;
                NationalFocusIcon = focus?.Icon ?? string.Empty;
                NationalFocusX = focus?.X ?? string.Empty;
                NationalFocusY = focus?.Y ?? string.Empty;
                NationalFocusCost = focus?.Cost ?? string.Empty;
                NationalFocusPrerequisite = focus?.Prerequisite ?? string.Empty;
                NationalFocusMutuallyExclusive = focus?.MutuallyExclusive ?? string.Empty;
                NationalFocusRelativePositionId = focus?.RelativePositionId ?? string.Empty;
                NationalFocusEffects = focus?.CompletionReward ?? string.Empty;
                SetNationalFocusUnsavedChanges(false);
            }
            finally {
                isSettingNationalFocusEditorContent = false;
            }
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

        private void MarkNationalFocusEditorChanged() {
            if (!isSettingNationalFocusEditorContent) {
                SetNationalFocusUnsavedChanges(SelectedNationalFocus is not null);
            }
        }

        private void SetNationalFocusUnsavedChanges(bool value) {
            if (hasUnsavedNationalFocusChanges == value) {
                return;
            }

            hasUnsavedNationalFocusChanges = value;
            NotifyUnsavedStateChanged();
            OnPropertyChanged(nameof(HasUnsavedNationalFocusChanges));
        }

        private void SetRawTextUnsavedChanges(bool value) {
            if (hasUnsavedChanges == value) {
                return;
            }

            hasUnsavedChanges = value;
            NotifyUnsavedStateChanged();
        }

        private void NotifyUnsavedStateChanged() {
            OnPropertyChanged(nameof(HasUnsavedChanges));
            RefreshFilesCommand.NotifyCanExecuteChanged();
            OnPropertyChanged(nameof(CanChangeWorkspaceSelection));
            OnPropertyChanged(nameof(SelectedFileStatusText));
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
