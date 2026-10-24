//! The player's GameCube discs (Street V3, the original game) for the game: a
//! roster.json player whose model is `v3gc:<key>` or `v1gc:<key>`
//! ([`crate::roster_edits::gamecube_model`]) is drawn as that character,
//! put on Vol. 2's skeleton ([`crate::canonical`]) so Vol. 2's animations
//! play on it unchanged.
//!
//! `street-game --v3 <disc> --v1 <disc>` (or `STREET_V3_GC`, `STREET_V1_GC`)
//! calls [`add`]; the renderer calls [`load`]. Without the disc the player
//! keeps its Vol. 2 stand-in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use crate::{
    Error, Result,
    canonical::{self, Rebound},
    eagl::Skeleton,
    gc_character::{self, CharacterInfo, Game, Library, Look},
    gcdisc::GcDisc,
};

struct Disc {
    path: PathBuf,
    lib: Arc<Library>,
    /// A stock body's height in the game's units (0 until measured).
    stock_units: Mutex<Option<f32>>,
}

static DISCS: RwLock<Vec<Disc>> = RwLock::new(Vec::new());
type Key = (String, Look);
static LOADED: Mutex<Option<HashMap<Key, Arc<Loaded>>>> = Mutex::new(None);

/// A GameCube character ready to draw.
#[derive(Debug)]
pub struct Loaded {
    pub info: CharacterInfo,
    /// On Vol. 2's skeleton, in Vol. 2's units.
    pub rebound: Rebound,
    /// The character as the game draws it, soles to crown
    /// ([`gc_character::model_height_in`]).
    pub model_height_in: f32,
}

/// Use the disc at `path` (V3 or the original game, any image `nod` reads); a disc
/// of the same game given before is replaced. Returns the game and its
/// character count.
pub fn add(path: &Path) -> Result<(Game, usize)> {
    let mut disc = GcDisc::open(path)?;
    let lib = Library::open(&mut disc)?;
    let (game, count) = (lib.game, lib.characters.len());
    let mut discs = DISCS.write().unwrap_or_else(|e| e.into_inner());
    discs.retain(|d| d.lib.game != game);
    discs.push(Disc {
        path: path.to_path_buf(),
        lib: Arc::new(lib),
        stock_units: Mutex::new(None),
    });
    if let Ok(mut l) = LOADED.lock() {
        *l = None;
    }
    Ok((game, count))
}

/// Forget every disc (tests).
pub fn clear() {
    DISCS.write().unwrap_or_else(|e| e.into_inner()).clear();
    if let Ok(mut l) = LOADED.lock() {
        *l = None;
    }
}

fn game_of(id: &str) -> Option<Game> {
    match id.split_once(':')?.0.to_ascii_lowercase().as_str() {
        "v3gc" => Some(Game::V3),
        "v1gc" => Some(Game::V1),
        _ => None,
    }
}

/// Whether the disc for `id`'s game was given.
pub fn has(id: &str) -> bool {
    let Some(game) = game_of(id) else {
        return false;
    };
    DISCS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|d| d.lib.game == game)
}

/// The table facts of `id` (`v3gc:mario`), when its disc was given.
pub fn info(id: &str) -> Option<CharacterInfo> {
    let game = game_of(id)?;
    let discs = DISCS.read().unwrap_or_else(|e| e.into_inner());
    discs
        .iter()
        .find(|d| d.lib.game == game)?
        .lib
        .find(id)
        .cloned()
}

/// `id` assembled in `look` and put on `vol2` (Vol. 2's player skeleton).
/// Kept, so a character loads once per run.
pub fn load(id: &str, look: Look, vol2: &Skeleton) -> Result<Arc<Loaded>> {
    let key = (id.to_ascii_lowercase(), look);
    if let Some(l) = LOADED
        .lock()
        .ok()
        .and_then(|m| m.as_ref().and_then(|m| m.get(&key).cloned()))
    {
        return Ok(l);
    }
    let game = game_of(id).ok_or_else(|| Error::NotFound(format!("{id}: not a GameCube id")))?;
    let (path, lib) = {
        let discs = DISCS.read().unwrap_or_else(|e| e.into_inner());
        let d = discs
            .iter()
            .find(|d| d.lib.game == game)
            .ok_or_else(|| Error::NotFound(format!("{id}: no {} disc given", game.prefix())))?;
        (d.path.clone(), d.lib.clone())
    };
    let info = lib
        .find(id)
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("{id}: not on the disc")))?;
    let mut disc = GcDisc::open(&path)?;
    let stock_units = {
        let measured = DISCS
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|d| d.lib.game == game)
            .and_then(|d| *d.stock_units.lock().unwrap_or_else(|e| e.into_inner()));
        match measured {
            Some(u) => u,
            None => {
                let u = lib.stock_units(&mut disc);
                if let Some(d) = DISCS
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .find(|d| d.lib.game == game)
                {
                    *d.stock_units.lock().unwrap_or_else(|e| e.into_inner()) = Some(u);
                }
                u
            }
        }
    };
    let c = lib.assemble(&mut disc, &info.key, look)?;
    let model_height_in = gc_character::model_height_in(&info, &c, stock_units);
    let rebound = canonical::rebind(&c, vol2)?;
    let loaded = Arc::new(Loaded {
        info,
        rebound,
        model_height_in,
    });
    if let Ok(mut m) = LOADED.lock() {
        m.get_or_insert_with(HashMap::new)
            .insert(key, loaded.clone());
    }
    Ok(loaded)
}
