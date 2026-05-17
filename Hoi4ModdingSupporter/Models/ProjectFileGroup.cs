using System;

namespace Hoi4ModdingSupporter.Models {
    public record ProjectFileGroup(
        string DisplayName,
        string[] PathPrefixes
    ) {
        public bool Includes(ProjectWorkspaceFile file) {
            if (PathPrefixes.Length == 0) {
                return true;
            }

            foreach (var pathPrefix in PathPrefixes) {
                if (IsInPathPrefix(file.RelativePath, pathPrefix)) {
                    return true;
                }
            }

            return false;
        }

        private static bool IsInPathPrefix(string relativePath, string pathPrefix) {
            return string.Equals(relativePath, pathPrefix, StringComparison.OrdinalIgnoreCase)
                || relativePath.StartsWith(pathPrefix + "\\", StringComparison.OrdinalIgnoreCase)
                || relativePath.StartsWith(pathPrefix + "/", StringComparison.OrdinalIgnoreCase);
        }
    }
}
