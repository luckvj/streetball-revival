//! What every machine in a match reads from its own disc for the
//! simulation, and the digest that shows two machines read the same.
//!
//! An online match never sends disc data, only digests and pads. The
//! referee (a game hosting, or `street-server --vol2`) and every client
//! build the tables the simulation reads from their own disc, as the game
//! builds them for a match played alone ([`Disc::build`], [`Disc::read`]),
//! and install them under each match's own values ([`Disc::install`],
//! `street_sim::disc`):
//!
//! - the pose bank (`street_sim::pose::Poses`): the skeleton and every
//!   animation record's clip, for the rules that read bones (the swipe's
//!   hand, the limbs a ball meets, the blocks' hands, the tricks' roots);
//! - the gait records with the clips' hand flags ([`gait`]);
//! - the staged Level 2 gamebreakers' plans and hand tracks ([`staged`]);
//! - the player and team tables, for the roster anti-cheat
//!   ([`crate::roster`]).
//!
//! [`Disc::digest`] is SHA-256 over all four as the simulation holds them.
//! A joiner sends its digest with the join ([`crate::Join::disc`]), and the
//! server refuses one that differs from its own
//! ([`crate::Refusal::Disc`]): another region's disc, or a modded disc the
//! server doesn't share. A roster mod is not on the disc: it travels as the
//! match's roster and the anti-cheat checks it as before.

pub mod gait;
pub mod staged;

use std::io::{Read, Seek};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use street_assets::clip::Clips;
use street_assets::db::{self, FieldKind, Table};
use street_assets::eagl::Skeleton;
use street_assets::gait::GaitData;
use street_assets::iso::Iso;
use street_assets::scripts::MoveScripts;
use street_assets::timeline::ScriptBank;
use street_sim::World;
use street_sim::disc::Data;
use street_sim::pose::{ClipData, Poses, SkeletonBone};

use crate::roster::DiscTables;

/// What the simulation reads from a disc.
#[derive(Clone)]
enum Sim {
    Data(Arc<Data>),
    /// The rules' stand-ins, for tests without a disc (test builds only).
    #[cfg(any(test, feature = "stand-ins"))]
    StandIns,
}

/// One machine's disc, as the simulation and the anti-cheat read it.
#[derive(Clone)]
pub struct Disc {
    sim: Sim,
    tables: DiscTables,
    digest: [u8; 32],
}

impl std::fmt::Debug for Disc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Disc({})",
            street_assets::roster_share::hex(&self.digest)
        )
    }
}

/// What [`Disc::build`] builds from, as the game has already decoded it.
pub struct Parts<'a> {
    pub poses: Arc<Poses>,
    /// The player skeleton (`street_assets::player::skeleton`), which the
    /// staged hand tracks are sampled on.
    pub skeleton: &'a Skeleton,
    /// Every clip on the disc.
    pub clips: &'a Clips,
    pub timeline: &'a ScriptBank,
    pub scripts: &'a MoveScripts,
    pub gait: &'a GaitData,
    /// The disc's own player and team tables (not a roster mod's).
    pub players: &'a Table,
    pub teams: &'a Table,
}

impl Disc {
    /// Read and build everything from the disc image (`street-server
    /// --vol2`, tests).
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self, String> {
        let text = |e: street_assets::Error| e.to_string();
        let poses = Arc::new(poses(iso)?);
        let skeleton = street_assets::player::skeleton(iso).map_err(text)?;
        let clips = Clips::load_all(iso).map_err(|e| e.to_string())?;
        let timeline = ScriptBank::read(iso).map_err(text)?;
        let scripts = MoveScripts::read(iso).map_err(text)?;
        let gait = GaitData::read(iso).map_err(text)?;
        let players = Table::read(iso, db::PLAYERS).map_err(text)?;
        let teams = Table::read(iso, db::TEAMS).map_err(text)?;
        Self::build(Parts {
            poses,
            skeleton: &skeleton,
            clips: &clips,
            timeline: &timeline,
            scripts: &scripts,
            gait: &gait,
            players: &players,
            teams: &teams,
        })
    }

    /// Build the simulation's tables and their digest from what the game
    /// decoded at start.
    pub fn build(p: Parts<'_>) -> Result<Self, String> {
        let gait = gait::tables(p.clips, p.timeline, p.scripts, p.gait);
        let mut staged = staged::config(p.timeline)?;
        staged::attach_hands(&mut staged, p.timeline, p.clips, p.skeleton)
            .map_err(|e| format!("staged gamebreakers: {e}"))?;
        let text = |e: street_assets::Error| e.to_string();
        let tables = DiscTables {
            players: db::parse_players(p.players).map_err(text)?,
            teams: db::parse_teams(p.teams).map_err(text)?,
        };
        let mut hash = Sha256::new();
        hash.update(b"Streetball Revival disc data 1\0");
        let mut part = |tag: &[u8], bytes: &[u8]| {
            hash.update(tag);
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        };
        let mut bank = Vec::new();
        p.poses.canonical(&mut |b| bank.extend_from_slice(b));
        part(b"poses", &bank);
        part(b"gait", &gait.canonical());
        part(b"staged", &staged.canonical());
        part(b"players", &table_bytes(p.players).map_err(text)?);
        part(b"teams", &table_bytes(p.teams).map_err(text)?);
        Ok(Self {
            sim: Sim::Data(Arc::new(Data {
                poses: p.poses,
                gait,
                staged,
            })),
            tables,
            digest: hash.finalize().into(),
        })
    }

    /// A machine with no disc that plays the rules' stand-ins, for tests
    /// (test builds only: `cfg(test)` or street-net's `stand-ins`
    /// feature). Its digest is all zeros.
    #[cfg(any(test, feature = "stand-ins"))]
    pub fn stand_ins() -> Self {
        Self {
            sim: Sim::StandIns,
            tables: DiscTables::default(),
            digest: [0; 32],
        }
    }

    /// SHA-256 over the simulation's tables and the roster tables.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// The disc's own player and team tables.
    pub fn tables(&self) -> &DiscTables {
        &self.tables
    }

    /// Whether this is a real disc (not the tests' stand-ins).
    pub fn is_disc(&self) -> bool {
        matches!(self.sim, Sim::Data(_))
    }

    /// The simulation's tables.
    pub fn data(&self) -> Option<&Arc<Data>> {
        match &self.sim {
            Sim::Data(data) => Some(data),
            #[cfg(any(test, feature = "stand-ins"))]
            Sim::StandIns => None,
        }
    }

    /// Install the disc's tables under `world`'s own values
    /// (`street_sim::World::install`); with the tests' stand-ins, nothing.
    pub fn install(&self, world: &mut World) -> Result<(), String> {
        match &self.sim {
            Sim::Data(data) => world.install(data),
            #[cfg(any(test, feature = "stand-ins"))]
            Sim::StandIns => Ok(()),
        }
    }
}

/// The skeleton and every animation record's clip, for the simulation's
/// bone rules (`street_sim::pose`).
pub fn poses<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Poses, String> {
    let data = street_assets::pose::PoseData::read(iso)
        .map_err(|e| format!("the disc's animations: {e}"))?;
    let skeleton: Vec<SkeletonBone> = data
        .skeleton
        .bones
        .iter()
        .map(|b| SkeletonBone {
            parent: b.parent.map(|p| p as u8),
            translation: b.translation,
            rotation: b.rotation,
        })
        .collect();
    let clips = data
        .clips
        .into_iter()
        .map(|c| ClipData {
            raw_pose: c.raw_pose,
            bones: c.bones,
            rotations: c.rotations,
            root: c.root,
            states: c.states,
        })
        .collect();
    Poses::new(&skeleton, clips, data.records)
        .ok_or_else(|| "the disc's player skeleton lacks its 22 bones".into())
}

/// A database table's records as bytes in a fixed order: its shape, then
/// each field (strings by length and bytes).
fn table_bytes(t: &Table) -> street_assets::Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend((t.record_count() as u32).to_le_bytes());
    out.extend((t.field_count() as u32).to_le_bytes());
    for record in 0..t.record_count() {
        for field in 0..t.field_count() {
            if t.kind(field) == Some(FieldKind::String) {
                let s = t.string(record, field)?.unwrap_or_default();
                out.extend((s.len() as u32).to_le_bytes());
                out.extend(s.as_bytes());
            } else {
                out.extend(t.raw(record, field)?.to_le_bytes());
            }
        }
    }
    Ok(out)
}
