# AGENTS.md

## Project Description

This project is a Support tool for Hearts of Iron IV Modding.
It provides a user-friendly interface to manage and edit project files, making it easier for modders to create and customize their mods.

## Want to do

- Load a mod project by `*.mod` file.
- User-friendly interface to edit project files.
- Easily to access game assets and mod assets.
- National Focus Editor
- Event, History, Ideology Manager
- Original Flag & NF Icon Importer
- Visual Mod Editor
- Use Original Game Asset smoothly, no need to extract them.
- Build as Standalone application, no need to install any dependencies.

## Code Style

- Follow editorconfig settings.

## Git Strategy

- Commit message format is must follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
- Push to `dev` branch until "Want to do" is completed, then merge to `main` branch.
- Commit each feature implementation and each fix.

## Architecture

- WinUI 3
- CommunityToolkit.Mvvm
- CommunityToolkit.WinUI.Controls.SettingsControls
- Fluent Results

## Implementation Guidelines

- Use MVVM pattern for separation of concerns.
- Implement it across multiple sub-agents to improve efficiency.
- Once the subagent reports that the implementation and modifications are complete, review the code.
  - If there is any redundancy or cause of bug in the code output by the subagent, send instructions to the subagent to correct it, including examples of how to fix it.
  - Once the corrections are complete, have the main agent review them and repeat the process until all problems are resolved.
