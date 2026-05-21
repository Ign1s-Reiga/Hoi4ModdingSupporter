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
    public class LocalisationWorkspaceViewModel : ObservableObject {
        private const int MaxLocalisationFileCount = 3000;
        private const long MaxLocalisationFileSizeBytes = 2 * 1024 * 1024;

        private static readonly string[] LocalisationRootNames = [
            "localisation",
            "localization"
        ];

        private readonly List<LocalisationFileInfo> allLocalisationFiles = [];
        private ProjectWorkspaceFile? selectedFile;
        private string? selectedLanguage;
        private string selectedFileLanguage = string.Empty;
        private string statusMessage = string.Empty;
        private bool hasUnsavedChanges;
        private bool isLoadingEntries;

        public LocalisationWorkspaceViewModel(RecentProjectRecord project) {
            Project = project;

            RefreshFilesCommand = new RelayCommand(
                () => StoreResult(RefreshFiles()),
                () => CanChangeSelection
            );
            SaveSelectedFileCommand = new RelayCommand(
                () => StoreResult(SaveSelectedFile()),
                () => SelectedFile is not null && Entries.Count > 0
            );

            RefreshFiles();
        }

        public RecentProjectRecord Project { get; }

        public ObservableCollection<string> Languages { get; } = [];

        public ObservableCollection<ProjectWorkspaceFile> LocalisationFiles { get; } = [];

        public ObservableCollection<LocalisationEntry> Entries { get; } = [];

        public ProjectWorkspaceFile? SelectedFile {
            get => selectedFile;
            set {
                if (SetProperty(ref selectedFile, value)) {
                    LoadSelectedFile();
                    OnPropertyChanged(nameof(SelectedFileTitle));
                    SaveSelectedFileCommand.NotifyCanExecuteChanged();
                }
            }
        }

        public string? SelectedLanguage {
            get => selectedLanguage;
            set {
                if (SetProperty(ref selectedLanguage, value)) {
                    ApplyLanguageFilter();
                }
            }
        }

        public string StatusMessage {
            get => statusMessage;
            set => SetProperty(ref statusMessage, value);
        }

        public bool HasUnsavedChanges {
            get => hasUnsavedChanges;
            private set {
                if (SetProperty(ref hasUnsavedChanges, value)) {
                    OnPropertyChanged(nameof(CanChangeSelection));
                    RefreshFilesCommand.NotifyCanExecuteChanged();
                }
            }
        }

        public bool CanChangeSelection => !HasUnsavedChanges;

        public string SelectedFileTitle => SelectedFile?.RelativePath ?? "Select a localisation file";

        public IRelayCommand RefreshFilesCommand { get; }

        public IRelayCommand SaveSelectedFileCommand { get; }

        public Result RefreshFiles() {
            allLocalisationFiles.Clear();
            Languages.Clear();
            LocalisationFiles.Clear();
            Entries.Clear();
            selectedFile = null;
            OnPropertyChanged(nameof(SelectedFile));
            OnPropertyChanged(nameof(SelectedFileTitle));

            if (string.IsNullOrWhiteSpace(Project.FolderPath)) {
                return StoreResult(Result.Fail("Project folder path is empty."));
            }

            if (!Directory.Exists(Project.FolderPath)) {
                return StoreResult(Result.Fail($"Project folder does not exist: {Project.FolderPath}"));
            }

            return Result.Try(() => {
                var roots = GetLocalisationRoots(Project.FolderPath).ToList();
                if (roots.Count == 0) {
                    StatusMessage = "No localisation folder was found.";
                    return;
                }

                foreach (var root in roots) {
                    foreach (var fileInfo in EnumerateLocalisationFiles(root)) {
                        if (allLocalisationFiles.Count >= MaxLocalisationFileCount) {
                            break;
                        }

                        var file = ProjectWorkspaceFile.FromFileInfo(Project.FolderPath, fileInfo, true);
                        var language = ReadLanguageHeader(fileInfo.FullName);
                        allLocalisationFiles.Add(new LocalisationFileInfo(file, language));

                        if (!string.IsNullOrWhiteSpace(language) && !Languages.Contains(language)) {
                            Languages.Add(language);
                        }
                    }
                }

                SelectedLanguage = Languages.FirstOrDefault();
                ApplyLanguageFilter();
                StatusMessage = LocalisationFiles.Count == 0
                    ? "No .yml localisation files were found."
                    : string.Empty;
            });
        }

        public Result SaveSelectedFile() {
            if (SelectedFile is null) {
                return Result.Fail("No localisation file is selected.");
            }

            return Result.Try(() => {
                var lines = File.ReadAllLines(SelectedFile.FullPath, Encoding.UTF8);

                foreach (var entry in Entries.Where(entry => !string.IsNullOrWhiteSpace(entry.Key))) {
                    if (entry.LineIndex < 0 || entry.LineIndex >= lines.Length) {
                        continue;
                    }

                    lines[entry.LineIndex] = FormatEntryLine(entry);
                }

                File.WriteAllLines(SelectedFile.FullPath, lines, new UTF8Encoding(encoderShouldEmitUTF8Identifier: true));
                HasUnsavedChanges = false;
                StatusMessage = "Localisation file saved.";
            });
        }

        public Result SavePendingChanges() {
            return HasUnsavedChanges
                ? SaveSelectedFile()
                : Result.Ok();
        }

        public void DiscardPendingChanges() {
            LoadSelectedFile();
            HasUnsavedChanges = false;
        }

        private void LoadSelectedFile() {
            Entries.Clear();
            selectedFileLanguage = string.Empty;

            if (SelectedFile is null) {
                StatusMessage = "Select a localisation file.";
                return;
            }

            if (SelectedFile.SizeBytes > MaxLocalisationFileSizeBytes) {
                StatusMessage = "Selected localisation file is too large to edit in the table.";
                return;
            }

            StoreResult(Result.Try(() => {
                isLoadingEntries = true;
                try {
                    var lineIndex = 0;
                    foreach (var line in File.ReadLines(SelectedFile.FullPath, Encoding.UTF8)) {
                        var trimmed = line.Trim();
                        if (string.IsNullOrWhiteSpace(trimmed) || trimmed.StartsWith('#')) {
                            lineIndex++;
                            continue;
                        }

                        if (string.IsNullOrWhiteSpace(selectedFileLanguage) && IsLanguageHeader(trimmed)) {
                            selectedFileLanguage = trimmed.TrimEnd(':').Trim();
                            lineIndex++;
                            continue;
                        }

                        if (TryParseEntry(line, out var key, out var version, out var value)) {
                            Entries.Add(new LocalisationEntry(key, version, value, lineIndex, MarkEntryChanged));
                        }

                        lineIndex++;
                    }
                }
                finally {
                    isLoadingEntries = false;
                }

                HasUnsavedChanges = false;
                StatusMessage = Entries.Count == 0
                    ? "No localisation entries were found in the selected file."
                    : string.Empty;
            }));
        }

        private void MarkEntryChanged() {
            if (isLoadingEntries) {
                return;
            }

            HasUnsavedChanges = true;
            SaveSelectedFileCommand.NotifyCanExecuteChanged();
        }

        private void ApplyLanguageFilter() {
            LocalisationFiles.Clear();
            Entries.Clear();
            selectedFile = null;
            OnPropertyChanged(nameof(SelectedFile));
            OnPropertyChanged(nameof(SelectedFileTitle));

            var files = string.IsNullOrWhiteSpace(SelectedLanguage)
                ? allLocalisationFiles
                : allLocalisationFiles.Where(file => string.Equals(file.Language, SelectedLanguage, StringComparison.OrdinalIgnoreCase));

            foreach (var file in files.Select(file => file.File)) {
                LocalisationFiles.Add(file);
            }
        }

        private Result StoreResult(Result result) {
            if (result.IsFailed) {
                StatusMessage = result.Errors.FirstOrDefault()?.Message ?? "Operation failed.";
            }

            return result;
        }

        private static IEnumerable<string> GetLocalisationRoots(string projectFolderPath) {
            foreach (var directoryName in LocalisationRootNames) {
                var root = Path.Combine(projectFolderPath, directoryName);
                if (Directory.Exists(root)) {
                    yield return root;
                }
            }
        }

        private static IEnumerable<FileInfo> EnumerateLocalisationFiles(string rootPath) {
            var root = new DirectoryInfo(rootPath);
            return root.EnumerateFiles("*.yml", SearchOption.AllDirectories)
                .OrderBy(file => file.FullName, StringComparer.OrdinalIgnoreCase);
        }

        private static string ReadLanguageHeader(string filePath) {
            foreach (var line in File.ReadLines(filePath, Encoding.UTF8)) {
                var trimmed = line.Trim();
                if (string.IsNullOrWhiteSpace(trimmed) || trimmed.StartsWith('#')) {
                    continue;
                }

                return IsLanguageHeader(trimmed)
                    ? trimmed.TrimEnd(':').Trim()
                    : string.Empty;
            }

            return string.Empty;
        }

        private static bool IsLanguageHeader(string value) {
            return value.StartsWith("l_", StringComparison.OrdinalIgnoreCase)
                && value.EndsWith(':');
        }

        private static bool TryParseEntry(string line, out string key, out string version, out string value) {
            key = string.Empty;
            version = string.Empty;
            value = string.Empty;

            var commentFreeLine = RemoveComment(line);
            var colonIndex = commentFreeLine.IndexOf(':');
            if (colonIndex <= 0) {
                return false;
            }

            key = commentFreeLine[..colonIndex].Trim();
            if (string.IsNullOrWhiteSpace(key) || key.StartsWith("l_", StringComparison.OrdinalIgnoreCase)) {
                return false;
            }

            var remainder = commentFreeLine[(colonIndex + 1)..].Trim();
            var quoteIndex = remainder.IndexOf('"');
            if (quoteIndex < 0) {
                return false;
            }

            version = remainder[..quoteIndex].Trim();
            var quotedValue = remainder[quoteIndex..].Trim();
            value = UnescapeValue(TrimQuotes(quotedValue));
            return true;
        }

        private static string FormatEntryLine(LocalisationEntry entry) {
            var version = string.IsNullOrWhiteSpace(entry.Version)
                ? string.Empty
                : entry.Version.Trim();
            return $" {entry.Key.Trim()}:{version} \"{EscapeValue(entry.Value)}\"";
        }

        private static string RemoveComment(string line) {
            var inQuote = false;
            for (var i = 0; i < line.Length; i++) {
                if (line[i] == '"' && (i == 0 || line[i - 1] != '\\')) {
                    inQuote = !inQuote;
                }

                if (line[i] == '#' && !inQuote) {
                    return line[..i];
                }
            }

            return line;
        }

        private static string TrimQuotes(string value) {
            var start = value.IndexOf('"');
            var end = value.LastIndexOf('"');
            return start >= 0 && end > start
                ? value[(start + 1)..end]
                : value;
        }

        private static string EscapeValue(string value) {
            return value
                .Replace("\\", "\\\\", StringComparison.Ordinal)
                .Replace("\"", "\\\"", StringComparison.Ordinal);
        }

        private static string UnescapeValue(string value) {
            return value
                .Replace("\\\"", "\"", StringComparison.Ordinal)
                .Replace("\\\\", "\\", StringComparison.Ordinal);
        }

        private record LocalisationFileInfo(ProjectWorkspaceFile File, string Language);
    }
}
