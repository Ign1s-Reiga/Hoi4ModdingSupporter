namespace Hoi4ModdingSupporter.Models {
    public record NationalFocusEntry(
        ProjectWorkspaceFile SourceFile,
        int BlockStartLine,
        int BlockEndLine,
        string Id,
        string Icon,
        string X,
        string Y,
        string Cost,
        string CompletionReward
    ) {
        public string DisplayName => string.IsNullOrWhiteSpace(Id)
            ? $"{SourceFile.RelativePath} focus #{BlockStartLine + 1}"
            : Id;
    }
}
