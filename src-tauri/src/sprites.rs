//! Sprite lookup: the `GFX_...` name a script uses, resolved to a picture.
//!
//! A focus names its icon but never says where the art is. The sprite is
//! defined in `interface/*.gfx`, which points at a `.dds` or `.tga` somewhere
//! under `gfx/`. Two overrides are in play and they are not the same one:
//!
//! - A mod inherits every sprite the game defines and may redefine any of
//!   them, so the game's `.gfx` files are read first and the mod's land on top.
//! - The texture is then looked for in the mod before the game, whichever
//!   folder defined the sprite, because dropping a file at the same relative
//!   path is how art is usually replaced.
//!
//! Indexing means parsing a few hundred files, so callers are expected to hold
//! on to a `SpriteIndex` rather than build one per icon.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::assets;
use crate::paradox::{self, Block, Item, Items};
use crate::project::normalise_path;
use crate::text_file;

/// Icons are drawn small, so the art is downscaled before it crosses the
/// bridge. Anything wanting it at full size has the texture path.
const ICON_MAX_DIMENSION: u32 = 128;

/// How far into a `.gfx` file to look for definitions. They sit one level
/// down, inside `spriteTypes = { ... }`.
const MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteIcon {
    pub name: String,
    /// PNG data URL, or null when the sprite or its texture was not found.
    pub url: Option<String>,
    /// The texture behind it, empty when nothing resolved it.
    pub path: String,
}

struct Sprite {
    /// Texture path as written, relative to a mod or game folder.
    texture: String,
    /// `noOfFrames`: one strip holds the same icon in several states.
    frames: u32,
}

pub struct SpriteIndex {
    /// Keyed by the upper cased sprite name, which scripts spell freely.
    sprites: HashMap<String, Sprite>,
}

impl SpriteIndex {
    /// Decodes one sprite for display. A name nothing defines, a texture that
    /// is not on disk and a file the decoder rejects all come back the same
    /// way — an icon with no picture — because each leaves the caller drawing
    /// the same blank.
    pub fn icon(&self, name: &str, folder: &str, game_root: &str) -> SpriteIcon {
        let blank = SpriteIcon {
            name: name.to_string(),
            url: None,
            path: String::new(),
        };

        let Some(sprite) = self.sprites.get(&key(name)) else {
            return blank;
        };
        let Some(texture) = resolve_texture(&sprite.texture, folder, game_root) else {
            return blank;
        };

        SpriteIcon {
            name: name.to_string(),
            url: assets::sprite_data_url(&texture, Some(ICON_MAX_DIMENSION), sprite.frames).ok(),
            path: normalise_path(&texture),
        }
    }
}

/// Reads every sprite definition the mod plays with.
pub fn index(folder: &str, game_root: &str) -> SpriteIndex {
    let mut sprites = HashMap::new();

    // The game first, so a mod redefining one of its sprite names wins.
    for root in [game_root, folder] {
        if root.trim().is_empty() {
            continue;
        }

        for path in gfx_files(Path::new(root)) {
            read_definitions(&path, &mut sprites);
        }
    }

    SpriteIndex { sprites }
}

fn gfx_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root.join("interface"))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .map(|value| value.eq_ignore_ascii_case("gfx"))
                .unwrap_or(false)
        })
        .collect()
}

/// A `.gfx` file the tool cannot read or parse costs one icon, not the tree,
/// so it is skipped rather than reported.
fn read_definitions(path: &Path, into: &mut HashMap<String, Sprite>) {
    let Ok(file) = text_file::read(path) else {
        return;
    };
    let Ok(document) = paradox::parse(&file.content) else {
        return;
    };

    collect(&document.items, MAX_DEPTH, into);
}

/// Collects any block naming a texture, whatever wraps it. A `.gfx` file holds
/// `spriteType`, `frameAnimatedSpriteType` and a dozen more type names, all
/// carrying the same two keys, and a few skip the `spriteTypes` wrapper.
fn collect(items: &[Item], depth: usize, into: &mut HashMap<String, Sprite>) {
    for item in items {
        let Item::Pair(pair) = item else {
            continue;
        };
        let Some(block) = pair.value.as_block() else {
            continue;
        };

        if let Some((name, sprite)) = read_sprite(block) {
            into.insert(name, sprite);
            continue;
        }

        if depth > 0 {
            collect(&block.items, depth - 1, into);
        }
    }
}

fn read_sprite(block: &Block) -> Option<(String, Sprite)> {
    let name = block.scalar("name")?;
    let texture = block.scalar("texturefile")?;

    let frames = block
        .scalar("noOfFrames")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|frames| *frames > 1)
        .unwrap_or(1);

    Some((key(&name), Sprite { texture, frames }))
}

fn key(name: &str) -> String {
    name.trim().to_uppercase()
}

/// The texture as the game would find it: the mod's copy of the file first.
fn resolve_texture(texture: &str, folder: &str, game_root: &str) -> Option<PathBuf> {
    let relative = texture.replace('\\', "/");
    let relative = relative.trim().trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }

    for root in [folder, game_root] {
        if root.trim().is_empty() {
            continue;
        }

        let candidate = Path::new(root).join(relative);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME_SPRITES: &str = r#"
spriteTypes = {
	spriteType = {
		name = "GFX_focus_generic_industry"
		texturefile = "gfx/interface/goals/focus_generic_industry.png"
	}
	frameAnimatedSpriteType = {
		name = "GFX_focus_generic_army"
		texturefile = "gfx/interface/goals/focus_generic_army.png"
		noOfFrames = 2
	}
}
"#;

    /// A game folder and a mod folder side by side, as the app sees them.
    fn fixture(name: &str) -> (String, String) {
        let root = std::env::temp_dir().join("hoi4ms-sprite-tests").join(name);
        let _ = std::fs::remove_dir_all(&root);

        let game = root.join("game");
        let modded = root.join("mod");
        std::fs::create_dir_all(&modded).expect("mod dir");

        write(&game.join("interface/goals.gfx"), GAME_SPRITES);
        art(
            &game.join("gfx/interface/goals/focus_generic_industry.png"),
            8,
            8,
        );
        // Twice as wide as it draws: two frames side by side.
        art(
            &game.join("gfx/interface/goals/focus_generic_army.png"),
            16,
            8,
        );

        (normalise_path(&modded), normalise_path(&game))
    }

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, body).expect("write");
    }

    fn art(path: &Path, width: u32, height: u32) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        image::RgbaImage::from_pixel(width, height, image::Rgba([9, 9, 9, 255]))
            .save(path)
            .expect("save");
    }

    /// The data URL back as an image, to check what was actually encoded.
    fn decode(icon: &SpriteIcon) -> image::DynamicImage {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let url = icon.url.as_deref().expect("the icon has a picture");
        let bytes = STANDARD
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .expect("base64");
        image::load_from_memory(&bytes).expect("png")
    }

    #[test]
    fn a_mod_inherits_the_games_sprites() {
        let (folder, game) = fixture("inherit");

        let icon = index(&folder, &game).icon("GFX_focus_generic_industry", &folder, &game);
        let picture = decode(&icon);

        assert!(icon.path.starts_with(&game), "resolved to {}", icon.path);
        assert_eq!((picture.width(), picture.height()), (8, 8));
    }

    #[test]
    fn an_unknown_sprite_has_no_picture() {
        let (folder, game) = fixture("unknown");

        let icon = index(&folder, &game).icon("GFX_focus_that_does_not_exist", &folder, &game);

        assert!(icon.url.is_none());
        assert!(icon.path.is_empty());
    }

    #[test]
    fn the_mods_own_texture_replaces_the_games() {
        let (folder, game) = fixture("texture");
        // No definition of its own: the mod only drops a file where the
        // game's sprite already points.
        art(
            &Path::new(&folder).join("gfx/interface/goals/focus_generic_industry.png"),
            8,
            8,
        );

        let icon = index(&folder, &game).icon("GFX_focus_generic_industry", &folder, &game);

        assert!(icon.path.starts_with(&folder), "resolved to {}", icon.path);
    }

    #[test]
    fn the_mods_own_definition_replaces_the_games() {
        let (folder, game) = fixture("definition");
        write(
            &Path::new(&folder).join("interface/my_mod.gfx"),
            r#"
spriteTypes = {
	spriteType = {
		name = "GFX_focus_generic_industry"
		texturefile = "gfx/interface/goals/my_industry.png"
	}
}
"#,
        );
        art(
            &Path::new(&folder).join("gfx/interface/goals/my_industry.png"),
            4,
            4,
        );

        let icon = index(&folder, &game).icon("GFX_focus_generic_industry", &folder, &game);

        assert!(icon.path.ends_with("my_industry.png"), "{}", icon.path);
    }

    #[test]
    fn an_animated_sprite_is_shown_as_its_first_frame() {
        let (folder, game) = fixture("frames");

        let icon = index(&folder, &game).icon("GFX_focus_generic_army", &folder, &game);
        let picture = decode(&icon);

        assert_eq!((picture.width(), picture.height()), (8, 8));
    }

    #[test]
    fn a_missing_game_folder_leaves_the_mods_own_sprites() {
        let (folder, _) = fixture("nogame");
        write(
            &Path::new(&folder).join("interface/my_mod.gfx"),
            "spriteTypes = { spriteType = { name = \"GFX_focus_solo\" texturefile = \"gfx/solo.png\" } }",
        );
        art(&Path::new(&folder).join("gfx/solo.png"), 4, 4);

        let icon = index(&folder, "").icon("GFX_focus_solo", &folder, "");

        assert!(icon.url.is_some());
    }

    #[test]
    fn sprite_names_are_matched_however_they_are_spelled() {
        let (folder, game) = fixture("case");

        let index = index(&folder, &game);

        assert!(index
            .icon("gfx_focus_generic_industry", &folder, &game)
            .url
            .is_some());
        assert!(index
            .icon("  GFX_focus_generic_industry  ", &folder, &game)
            .url
            .is_some());
    }
}
