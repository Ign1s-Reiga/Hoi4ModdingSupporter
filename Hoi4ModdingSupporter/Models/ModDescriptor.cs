using System;
using System.Collections.Generic;
using System.IO;
using FluentResults;

namespace Hoi4ModdingSupporter.Models {
    public record ModDescriptor(
        string ModFilePath,
        string ProjectFolderPath,
        string DisplayName,
        string ImagePath
    );

    public static class ModDescriptorReader {
        public static Result<ModDescriptor> Read(string modFilePath) {
            if (string.IsNullOrWhiteSpace(modFilePath)) {
                return Result.Fail("Mod file path is empty.");
            }

            if (!File.Exists(modFilePath)) {
                return Result.Fail($"Mod file does not exist: {modFilePath}");
            }

            if (!string.Equals(Path.GetExtension(modFilePath), ".mod", StringComparison.OrdinalIgnoreCase)) {
                return Result.Fail("Selected file is not a .mod file.");
            }

            return Result.Try(() => {
                var values = ReadKeyValues(modFilePath);
                var descriptorDirectoryPath = Path.GetDirectoryName(modFilePath) ?? Environment.CurrentDirectory;

                values.TryGetValue("name", out var displayName);
                values.TryGetValue("path", out var projectPath);
                values.TryGetValue("picture", out var picturePath);

                var projectFolderPath = ResolveProjectFolderPath(descriptorDirectoryPath, projectPath);
                var imagePath = ResolveImagePath(projectFolderPath, descriptorDirectoryPath, picturePath);

                return new ModDescriptor(
                    ModFilePath: modFilePath,
                    ProjectFolderPath: projectFolderPath,
                    DisplayName: string.IsNullOrWhiteSpace(displayName)
                        ? Path.GetFileNameWithoutExtension(modFilePath)
                        : displayName,
                    ImagePath: imagePath
                );
            });
        }

        private static Dictionary<string, string> ReadKeyValues(string modFilePath) {
            var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

            foreach (var line in File.ReadLines(modFilePath)) {
                var trimmed = line.Trim();
                if (trimmed.Length == 0 || trimmed.StartsWith('#')) {
                    continue;
                }

                var separatorIndex = trimmed.IndexOf('=');
                if (separatorIndex <= 0) {
                    continue;
                }

                var key = trimmed[..separatorIndex].Trim();
                var value = trimmed[(separatorIndex + 1)..].Trim().Trim('"');

                if (key.Length > 0) {
                    values[key] = value;
                }
            }

            return values;
        }

        private static string ResolveProjectFolderPath(string descriptorDirectoryPath, string? projectPath) {
            if (string.IsNullOrWhiteSpace(projectPath)) {
                return descriptorDirectoryPath;
            }

            if (Path.IsPathRooted(projectPath)) {
                return Path.GetFullPath(projectPath);
            }

            var descriptorDirectoryName = Path.GetFileName(descriptorDirectoryPath.TrimEnd(Path.DirectorySeparatorChar));
            var normalizedProjectPath = projectPath.Replace(Path.AltDirectorySeparatorChar, Path.DirectorySeparatorChar);
            var firstSegment = normalizedProjectPath.Split(Path.DirectorySeparatorChar)[0];
            var basePath = string.Equals(firstSegment, descriptorDirectoryName, StringComparison.OrdinalIgnoreCase)
                ? Directory.GetParent(descriptorDirectoryPath)?.FullName ?? descriptorDirectoryPath
                : descriptorDirectoryPath;

            return Path.GetFullPath(normalizedProjectPath, basePath);
        }

        private static string ResolveImagePath(string projectFolderPath, string descriptorDirectoryPath, string? picturePath) {
            if (string.IsNullOrWhiteSpace(picturePath)) {
                return string.Empty;
            }

            if (Path.IsPathRooted(picturePath)) {
                return File.Exists(picturePath) ? Path.GetFullPath(picturePath) : string.Empty;
            }

            var projectImagePath = Path.GetFullPath(picturePath, projectFolderPath);
            if (File.Exists(projectImagePath)) {
                return projectImagePath;
            }

            var descriptorImagePath = Path.GetFullPath(picturePath, descriptorDirectoryPath);
            return File.Exists(descriptorImagePath) ? descriptorImagePath : string.Empty;
        }
    }
}
