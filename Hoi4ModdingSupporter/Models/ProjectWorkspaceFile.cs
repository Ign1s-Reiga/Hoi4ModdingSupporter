using System;
using System.IO;

namespace Hoi4ModdingSupporter.Models {
    public record ProjectWorkspaceFile(
        string FullPath,
        string RelativePath,
        string Name,
        string Extension,
        long SizeBytes,
        DateTime LastModified,
        bool IsTextFile
    ) {
        public static ProjectWorkspaceFile FromFileInfo(string projectFolderPath, FileInfo fileInfo, bool isTextFile) {
            return new ProjectWorkspaceFile(
                FullPath: fileInfo.FullName,
                RelativePath: Path.GetRelativePath(projectFolderPath, fileInfo.FullName),
                Name: fileInfo.Name,
                Extension: fileInfo.Extension,
                SizeBytes: fileInfo.Length,
                LastModified: fileInfo.LastWriteTime,
                IsTextFile: isTextFile
            );
        }
    }
}
