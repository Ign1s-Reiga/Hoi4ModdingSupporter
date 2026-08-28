//! The province map.
//!
//! `map/provinces.bmp` paints every province in a unique colour and
//! `map/definition.csv` says which colour is which province. Joining those to
//! the state files gives a map that can be coloured by state or by owner, and
//! a click anywhere on it resolved back to a province and the state holding it.
//!
//! Decoding is the expensive part — thirteen million pixels — so a loaded map
//! is cached and only rebuilt when the mod folder changes.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::ImageFormat;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::{self, Item, Items, Value};
use crate::text_file;

/// Nothing on the map is a province: the black borders in `provinces.bmp` and
/// any colour the definitions do not mention.
const NO_PROVINCE: u16 = u16::MAX;

/// How much of a pixel's colour survives, as a percentage, inside a province
/// and at each kind of edge.
const INSIDE: u16 = 100;
const PROVINCE_EDGE: u16 = 78;
const STATE_EDGE: u16 = 45;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapSummary {
    pub width: u32,
    pub height: u32,
    pub province_count: usize,
    pub state_count: usize,
    /// Provinces no state claims, which usually means sea or an oversight.
    pub unassigned_land: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvincePick {
    pub province_id: u32,
    pub kind: String,
    pub terrain: String,
    /// Empty when no state lists this province.
    pub state_id: String,
    pub state_name: String,
    pub owner: String,
    /// File the state is defined in, so the editor can open it.
    pub state_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MapMode {
    /// A distinct colour per state.
    States,
    /// The colour each country uses in game.
    Owners,
}

struct ProvinceInfo {
    id: u32,
    kind: String,
    terrain: String,
}

struct StateInfo {
    id: String,
    name: String,
    owner: String,
    path: String,
}

pub struct MapData {
    folder: String,
    width: u32,
    height: u32,
    /// Province index per pixel, row major from the top row.
    province_at: Vec<u16>,
    provinces: Vec<ProvinceInfo>,
    /// Province id to the state that lists it.
    state_of_province: HashMap<u32, usize>,
    states: Vec<StateInfo>,
    /// State index per province index, so the render loop never hashes.
    /// `usize::MAX` means no state claims it.
    state_of_index: Vec<usize>,
    country_colors: HashMap<String, [u8; 3]>,
}

impl MapData {
    /// Mod folder this map was built from, so a cache can tell it is stale.
    pub fn folder(&self) -> &str {
        &self.folder
    }

    pub fn summary(&self) -> MapSummary {
        let unassigned_land = self
            .provinces
            .iter()
            .filter(|province| {
                province.kind == "land" && !self.state_of_province.contains_key(&province.id)
            })
            .count();

        MapSummary {
            width: self.width,
            height: self.height,
            province_count: self.provinces.len(),
            state_count: self.states.len(),
            unassigned_land,
        }
    }

    /// The province and state under a pixel of the full sized map.
    pub fn pick(&self, x: u32, y: u32) -> Option<ProvincePick> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = self.province_at[(y as usize) * (self.width as usize) + x as usize];
        if index == NO_PROVINCE {
            return None;
        }

        let province = self.provinces.get(index as usize)?;
        let state = self
            .state_of_province
            .get(&province.id)
            .and_then(|at| self.states.get(*at));

        Some(ProvincePick {
            province_id: province.id,
            kind: province.kind.clone(),
            terrain: province.terrain.clone(),
            state_id: state.map(|state| state.id.clone()).unwrap_or_default(),
            state_name: state.map(|state| state.name.clone()).unwrap_or_default(),
            owner: state.map(|state| state.owner.clone()).unwrap_or_default(),
            state_path: state.map(|state| state.path.clone()).unwrap_or_default(),
        })
    }

    /// Paints the map and encodes it as a PNG data URL.
    pub fn render(&self, mode: MapMode) -> AppResult<String> {
        let buffer = image::RgbImage::from_raw(self.width, self.height, self.paint(mode))
            .ok_or_else(|| AppError::message("the rendered map did not fit its buffer"))?;

        let mut encoded = Vec::new();
        buffer
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .map_err(|error| AppError::Image {
                path: format!("{}/map/provinces.bmp", self.folder),
                message: error.to_string(),
            })?;

        Ok(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(&encoded)
        ))
    }

    /// Paints every pixel of the map as raw RGB.
    ///
    /// Both outlines are drawn in the one pass: an edge between two provinces
    /// is darkened a little, an edge between two states a lot. Without the
    /// province lines a state is a flat wash of colour with no way to see the
    /// provinces it is made of; without the state lines being stronger, the
    /// two are indistinguishable.
    fn paint(&self, mode: MapMode) -> Vec<u8> {
        let palette = self.palette(mode);
        let width = self.width as usize;
        let height = self.height as usize;

        let mut pixels = vec![0u8; width * height * 3];

        for y in 0..height {
            for x in 0..width {
                let at = y * width + x;
                let mine = self.province_at[at];
                let colour = palette[self.palette_index(mine)];

                // Only the right and lower neighbours are compared: the pixel
                // on the other side of an edge darkens itself when its turn
                // comes, so every border still ends up one pixel wide.
                let right = (x + 1 < width).then(|| self.province_at[at + 1]);
                let below = (y + 1 < height).then(|| self.province_at[at + width]);

                let mut shade = INSIDE;
                for other in [right, below].into_iter().flatten() {
                    if other == mine {
                        continue;
                    }
                    shade = shade.min(if self.state_of(other) == self.state_of(mine) {
                        PROVINCE_EDGE
                    } else {
                        STATE_EDGE
                    });
                }

                let out = at * 3;
                for channel in 0..3 {
                    pixels[out + channel] = (colour[channel] as u16 * shade / 100) as u8;
                }
            }
        }

        pixels
    }

    /// State a province belongs to, or `usize::MAX` for none.
    fn state_of(&self, province: u16) -> usize {
        self.state_of_index
            .get(province as usize)
            .copied()
            .unwrap_or(usize::MAX)
    }

    /// Palette slot for a province index. Everything the definitions do not
    /// name falls to the last entry rather than borrowing another province's
    /// colour, which is what plain wrapping would do.
    fn palette_index(&self, province: u16) -> usize {
        (province as usize).min(self.provinces.len())
    }

    /// One colour per province index, plus a trailing entry for no province.
    fn palette(&self, mode: MapMode) -> Vec<[u8; 3]> {
        let water = [38, 54, 78];
        let unclaimed = [58, 58, 62];
        let mut palette = Vec::with_capacity(self.provinces.len() + 1);

        for (index, province) in self.provinces.iter().enumerate() {
            if province.kind != "land" {
                palette.push(water);
                continue;
            }

            let state = self
                .state_of_index
                .get(index)
                .filter(|at| **at != usize::MAX)
                .and_then(|at| self.states.get(*at));

            let colour = match (state, mode) {
                (None, _) => unclaimed,
                (Some(state), MapMode::States) => distinct_colour(&state.id),
                (Some(state), MapMode::Owners) => {
                    if state.owner.is_empty() {
                        unclaimed
                    } else {
                        self.country_colors
                            .get(&state.owner)
                            .copied()
                            .unwrap_or_else(|| distinct_colour(&state.owner))
                    }
                }
            };

            palette.push(colour);
        }

        // Index NO_PROVINCE wraps to this entry: the black province borders.
        palette.push([20, 20, 22]);
        palette
    }
}

/// A stable colour for a key, spread around the hue circle so neighbouring
/// states rarely share a shade.
fn distinct_colour(key: &str) -> [u8; 3] {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }

    let hue = (hash % 360) as f32;
    let saturation = 0.45 + ((hash >> 9) % 25) as f32 / 100.0;
    let value = 0.55 + ((hash >> 17) % 30) as f32 / 100.0;

    hsv_to_rgb(hue, saturation, value)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [u8; 3] {
    let chroma = value * saturation;
    let secondary = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let offset = value - chroma;

    let (red, green, blue) = match hue as u32 / 60 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };

    [
        ((red + offset) * 255.0) as u8,
        ((green + offset) * 255.0) as u8,
        ((blue + offset) * 255.0) as u8,
    ]
}

/// Reads the map of a mod, falling back to the game folder for anything the
/// mod does not override.
pub fn load(folder: &str, game_root: &str) -> AppResult<MapData> {
    let definitions_path = resolve(folder, game_root, "map/definition.csv")?;
    let provinces_path = resolve(folder, game_root, "map/provinces.bmp")?;

    let (provinces, by_colour) = read_definitions(&definitions_path)?;

    let image = image::open(&provinces_path)
        .map_err(|error| AppError::Image {
            path: provinces_path.display().to_string(),
            message: error.to_string(),
        })?
        .to_rgb8();

    let (width, height) = image.dimensions();
    let mut province_at = vec![NO_PROVINCE; (width as usize) * (height as usize)];

    for (at, pixel) in image.pixels().enumerate() {
        let key = colour_key(pixel[0], pixel[1], pixel[2]);
        province_at[at] = by_colour[key];
    }

    let (states, state_of_province) = read_states(folder);
    let country_colors = read_country_colors(folder, game_root);

    // Flatten province -> state once so rendering is pure array indexing. The
    // extra slot is for NO_PROVINCE, which wraps to the end.
    let mut state_of_index = vec![usize::MAX; provinces.len() + 1];
    for (index, province) in provinces.iter().enumerate() {
        if let Some(at) = state_of_province.get(&province.id) {
            state_of_index[index] = *at;
        }
    }

    Ok(MapData {
        folder: folder.to_string(),
        width,
        height,
        province_at,
        provinces,
        state_of_province,
        states,
        state_of_index,
        country_colors,
    })
}

fn resolve(folder: &str, game_root: &str, relative: &str) -> AppResult<PathBuf> {
    let in_mod = Path::new(folder).join(relative);
    if in_mod.is_file() {
        return Ok(in_mod);
    }

    let in_game = Path::new(game_root).join(relative);
    if in_game.is_file() {
        return Ok(in_game);
    }

    Err(AppError::message(format!(
        "{relative} is in neither the mod nor the game folder. Set the game folder in Settings if the mod does not ship its own map."
    )))
}

fn colour_key(red: u8, green: u8, blue: u8) -> usize {
    ((red as usize) << 16) | ((green as usize) << 8) | blue as usize
}

/// `definition.csv` is `id;red;green;blue;type;coastal;terrain;continent`.
fn read_definitions(path: &Path) -> AppResult<(Vec<ProvinceInfo>, Vec<u16>)> {
    let file = text_file::read(path)?;

    let mut provinces = Vec::new();
    // A direct lookup beats hashing thirteen million times.
    let mut by_colour = vec![NO_PROVINCE; 1 << 24];

    for line in file.content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 7 {
            continue;
        }

        let (Ok(id), Ok(red), Ok(green), Ok(blue)) = (
            fields[0].trim().parse::<u32>(),
            fields[1].trim().parse::<u8>(),
            fields[2].trim().parse::<u8>(),
            fields[3].trim().parse::<u8>(),
        ) else {
            continue;
        };

        // Province zero is the placeholder every map carries.
        if id == 0 {
            continue;
        }

        if provinces.len() >= NO_PROVINCE as usize {
            break;
        }

        by_colour[colour_key(red, green, blue)] = provinces.len() as u16;
        provinces.push(ProvinceInfo {
            id,
            kind: fields[4].trim().to_string(),
            terrain: fields[6].trim().to_string(),
        });
    }

    Ok((provinces, by_colour))
}

/// Every state in the mod, and which state each province belongs to.
fn read_states(folder: &str) -> (Vec<StateInfo>, HashMap<u32, usize>) {
    let root = Path::new(folder).join("history").join("states");
    let mut states = Vec::new();
    let mut state_of_province = HashMap::new();

    let Ok(entries) = std::fs::read_dir(&root) else {
        return (states, state_of_province);
    };

    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("txt"))
        .collect();
    paths.sort();

    for path in paths {
        let Ok(file) = crate::state::read(&path.display().to_string()) else {
            continue;
        };

        for state in file.states {
            let at = states.len();
            for province in &state.provinces {
                if let Ok(id) = province.parse::<u32>() {
                    state_of_province.insert(id, at);
                }
            }

            states.push(StateInfo {
                id: state.id,
                name: state.name,
                owner: state.owner,
                path: file.path.clone(),
            });
        }
    }

    (states, state_of_province)
}

/// `common/countries/colors.txt` holds `TAG = { color = rgb { r g b } }`.
fn read_country_colors(folder: &str, game_root: &str) -> HashMap<String, [u8; 3]> {
    let mut colours = HashMap::new();

    // The mod is read last so its overrides win.
    for root in [game_root, folder] {
        let path = Path::new(root)
            .join("common")
            .join("countries")
            .join("colors.txt");
        let Ok(file) = text_file::read(&path) else {
            continue;
        };
        let Ok(document) = paradox::parse(&file.content) else {
            continue;
        };

        for pair in document.pairs() {
            let Some(block) = pair.value.as_block() else {
                continue;
            };
            if let Some(colour) = read_colour(block) {
                colours.insert(pair.key.to_uppercase(), colour);
            }
        }
    }

    colours
}

/// `color = rgb { 210 134 134 }` parses as the scalar `rgb` followed by a
/// separate block, so the numbers are taken from whichever comes first.
fn read_colour(block: &paradox::Block) -> Option<[u8; 3]> {
    let mut take_next_block = false;

    for item in &block.items {
        match item {
            Item::Pair(pair) if pair.key.eq_ignore_ascii_case("color") => match &pair.value {
                Value::Block(inner) => return channels(inner),
                Value::Scalar(_) => take_next_block = true,
            },
            Item::Value(Value::Block(inner)) if take_next_block => return channels(inner),
            _ => {}
        }
    }

    None
}

fn channels(block: &paradox::Block) -> Option<[u8; 3]> {
    let numbers: Vec<f32> = block
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Value(Value::Scalar(scalar)) => scalar.value().parse::<f32>().ok(),
            _ => None,
        })
        .collect();

    if numbers.len() < 3 {
        return None;
    }

    // Some files use 0 to 1 rather than 0 to 255.
    let scale = if numbers.iter().all(|value| *value <= 1.0) {
        255.0
    } else {
        1.0
    };

    Some([
        (numbers[0] * scale).clamp(0.0, 255.0) as u8,
        (numbers[1] * scale).clamp(0.0, 255.0) as u8,
        (numbers[2] * scale).clamp(0.0, 255.0) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one row map of four pixels: two of province 1, then province 2, then
    /// province 3. Provinces 1 and 2 share a state, province 3 has its own.
    fn strip() -> MapData {
        let provinces = (1..=3)
            .map(|id| ProvinceInfo {
                id,
                kind: "land".to_string(),
                terrain: "plains".to_string(),
            })
            .collect();

        let states = ["10", "20"]
            .iter()
            .map(|id| StateInfo {
                id: id.to_string(),
                name: format!("STATE_{id}"),
                owner: "ENG".to_string(),
                path: String::new(),
            })
            .collect();

        MapData {
            folder: String::new(),
            width: 4,
            height: 1,
            province_at: vec![0, 0, 1, 2],
            provinces,
            state_of_province: HashMap::from([(1, 0), (2, 0), (3, 1)]),
            states,
            state_of_index: vec![0, 0, 1, usize::MAX],
            country_colors: HashMap::new(),
        }
    }

    fn pixel(pixels: &[u8], at: usize) -> [u8; 3] {
        [pixels[at * 3], pixels[at * 3 + 1], pixels[at * 3 + 2]]
    }

    fn shaded(colour: [u8; 3], shade: u16) -> [u8; 3] {
        colour.map(|channel| (channel as u16 * shade / 100) as u8)
    }

    #[test]
    fn draws_province_edges_under_state_edges() {
        let pixels = strip().paint(MapMode::States);

        let first = distinct_colour("10");
        let second = distinct_colour("20");

        // Inside province 1, with province 1 to the right.
        assert_eq!(pixel(&pixels, 0), shaded(first, INSIDE));
        // Province 1 meeting province 2, both in state 10.
        assert_eq!(pixel(&pixels, 1), shaded(first, PROVINCE_EDGE));
        // Province 2 meeting province 3, which is a different state.
        assert_eq!(pixel(&pixels, 2), shaded(first, STATE_EDGE));
        // The last pixel has no neighbour to compare against.
        assert_eq!(pixel(&pixels, 3), shaded(second, INSIDE));
    }

    #[test]
    fn an_undefined_colour_does_not_borrow_a_province_colour() {
        let mut map = strip();
        map.province_at = vec![NO_PROVINCE; 4];

        let pixels = map.paint(MapMode::States);

        // The palette's last entry, not whatever province the index wraps to.
        assert_eq!(pixel(&pixels, 0), [20, 20, 22]);
    }

    #[test]
    fn a_colour_is_stable_for_a_key() {
        assert_eq!(distinct_colour("42"), distinct_colour("42"));
        assert_ne!(distinct_colour("42"), distinct_colour("43"));
    }

    #[test]
    fn reads_the_rgb_form_the_game_uses() {
        let document = paradox::parse("ENG = {\n\tcolor = rgb { 201 56 93 }\n}\n").expect("parses");
        let block = document.block("ENG").expect("country block");

        assert_eq!(read_colour(block), Some([201, 56, 93]));
    }

    #[test]
    fn reads_a_plain_colour_block() {
        let document = paradox::parse("GER = {\n\tcolor = { 210 134 134 }\n}\n").expect("parses");
        let block = document.block("GER").expect("country block");

        assert_eq!(read_colour(block), Some([210, 134, 134]));
    }

    #[test]
    fn scales_a_colour_written_from_zero_to_one() {
        let document = paradox::parse("FRA = {\n\tcolor = { 1.0 0.0 0.5 }\n}\n").expect("parses");
        let block = document.block("FRA").expect("country block");

        assert_eq!(read_colour(block), Some([255, 0, 127]));
    }

    #[test]
    fn parses_the_definition_format() {
        let directory = std::env::temp_dir().join("hoi4ms-map-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("definition.csv");
        std::fs::write(
            &path,
            "0;0;0;0;land;false;unknown;0\n1;230;81;119;lake;false;lakes;7\n2;0;0;55;land;false;forest;1\n",
        )
        .expect("write");

        let (provinces, by_colour) = read_definitions(&path).expect("reads");

        // Province zero is skipped.
        assert_eq!(provinces.len(), 2);
        assert_eq!(provinces[0].id, 1);
        assert_eq!(provinces[0].kind, "lake");
        assert_eq!(provinces[1].terrain, "forest");
        assert_eq!(by_colour[colour_key(230, 81, 119)], 0);
        assert_eq!(by_colour[colour_key(0, 0, 55)], 1);
    }
}
