//! The crowd: every seat of the court's plan with its body and head, in
//! their textures and shirt colour, animated the way the original animates
//! it. See `street_assets::crowd` for the data.
//!
//! Each (model, group) pair shares one mesh whose vertices are rewritten
//! when the group's frame changes; the seats are entities that place it.
//!
//! ## The original's crowd (traced in SLUS_206.51)
//!
//! - **Roles.** The loader `FUN_0030b9b8` gives each seat a role from its
//!   name (`FUN_0030b7e0`: `ST` 0, `SD` 1, `C` 2, `W` 4, anything else
//!   `DAT_00403030` = 1). A group (plan column 7) takes the role of its last
//!   seat (`FUN_0030ea48`) and starts on the first node of that role's
//!   graph.
//! - **Graphs** (`FUN_0030e3e0`, [`Graph::new`]): one per role present. A
//!   node is an animation, a category (2 the crowd's own idling, 0 a cheer,
//!   1 a gasp: named for the calls that ask for them) and the list its
//!   successor is chosen from. A role's
//!   animations follow the earlier roles' in the models: 10 for role 0, 8
//!   for 1, 7 for 2 and 1 for 4. Every court's models have exactly that
//!   many records. Role 4's one node walks: it moves its seats 0.6855
//!   model units along their own z every tick (`0x4da0b4`).
//! - **Playback** (class `0x308830`, vtable `0x4991d0`): each model plays
//!   one animation per group. `+0x5c` (`0x309eb8`) counts a timer down by
//!   the step; at zero it adds the period back and shows the next frame,
//!   or reports the end when none is left. `+0x4c` (`0x309fa8`) starts an
//!   animation with a period. The loader starts every group of every model
//!   on animation 0 at period 1, then runs each group's update
//!   `rand() % 100 + 100` times so the groups fall out of step.
//! - **Each tick** (`FUN_0030db18` from the crowd object's update
//!   `FUN_0030c9e0`, skipped while the pause menu is up,
//!   `DAT_00405388`): every group runs `FUN_0030dd50`. A walker steps (and
//!   after 1800 ticks steps back the whole way, `FUN_0030ebd8`). When the
//!   body's animation ends, the next node comes from the current node's
//!   list: a random one of those in the category the group wants, else the
//!   idling node that returns to the role's own list. It plays at period 4
//!   (one frame per four ticks) and the group wants idling again. Each head
//!   model follows on its own clock with the same animation index; heads
//!   and bodies have the same frame counts, so they stay in step.
//! - **Reactions** ([`Director::react`], `FUN_0030ca10`): the audio
//!   listener's crowd call `FUN_001a75a8` asks `FUN_0030d618` to make
//!   `trunc(groups * f)` distinct random groups want a cheer (0), or a
//!   gasp (1) for its event 0x29, where `f = lead / points to win * 0.5 +
//!   0.5`, at most 1 ([`fraction`]). A group takes it up at the end of its
//!   current animation.
//!
//! - **Colour** ([`Seat::vertex_colour`]): `FUN_0030e250` gives the head
//!   and the body the plan's colour word, halved with its middle bytes
//!   swapped (`0x30a450`); the VU programs store it as the strips' RGBAQ
//!   and the texture modulates. Read as RGBA at full scale on the body
//!   alone, the bright pinks saturated the shirts flat and the seated
//!   rows behind the baselines looked torn into shards, heads untouched.
//!
//! The draws come from the rewrite's own copy of the game's LCG
//! (`FUN_003b42a0`'s step). The original also draws the group picks and
//! heights from `FUN_003a16b8`, and shares both generators with the rest of
//! the game, so the sequence is not the original's.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology,
    prelude::*,
};
use street_assets::crowd::{Crowd, Model, Seat};

use crate::convert;

/// The game's spectator scale (0x30b9b8): x and z 0.3257; y the same times a
/// random height of 5.5..6.25 (heads 0, 1) or 5.25..5.92 (2, 3) over 6.5.
const SCALE: f32 = 0.325_732_9;

/// Ticks a frame shows once a group has made its first pick
/// (`FUN_0030dd50` starts every pick at period 4.0 and steps 1.0).
const PERIOD: f32 = 4.0;
/// The loader's period for the opening animation 0 (`0x30b9b8`).
const START_PERIOD: f32 = 1.0;
/// Ticks a walker walks before stepping back (`FUN_0030dc68`: 0x708).
const WALK_TICKS: u32 = 1800;
/// How far role 4's node walks a tick, along the seat's own z
/// (`0x4da0b4` = 0x3f2f7e3d, set by `FUN_0030e3e0`).
const WALK: f32 = f32::from_bits(0x3f2f_7e3d);
/// At most five walkers per group move (`FUN_0030ea48` allocates five
/// entries; `FUN_0030e2c0` overwrites the last when full).
const WALKERS: usize = 5;

/// The categories of a graph node.
const CHEER: u8 = 0;
const GASP: u8 = 1;
const IDLE: u8 = 2;

/// A crowd call: `FUN_0030ca10(f, crowd, cheer)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reaction {
    /// The share of the groups that react.
    pub fraction: f32,
    /// A cheer (every call but 0x29) or a gasp (0x29).
    pub cheer: bool,
}

/// Crowd calls for the next crowd tick ([`step`] takes them).
#[derive(Resource, Default)]
pub struct CrowdReactions(pub Vec<Reaction>);

/// `FUN_001a75a8`'s share of the groups: the leading score over the
/// points to win (21 when unset), halved, plus a half, at most 1.
pub fn fraction(lead: u32, to_win: u32) -> f32 {
    let to_win = if to_win > 0 { to_win } else { 21 };
    ((lead as f32 / to_win as f32) * 0.5 + 0.5).min(1.0)
}

#[derive(Clone, Copy, Debug)]
struct Node {
    /// The list the next node is chosen from.
    next: usize,
    category: u8,
    animation: usize,
    /// Model units a tick (role 4's walk).
    velocity: [f32; 3],
}

/// The crowd's animation graphs (`FUN_0030e3e0`).
pub struct Graph {
    nodes: Vec<Node>,
    lists: Vec<Vec<usize>>,
    /// Each role's own list (`PTR_DAT_00403058`).
    roots: [Option<usize>; 5],
    animations: usize,
}

/// One role's graph as `FUN_0030e3e0` writes it: its lists (of its own
/// nodes), its nodes as (list, category, animation), its animations.
type RoleGraph = (
    &'static [&'static [usize]],
    &'static [(usize, u8, usize)],
    usize,
);

/// Role 0 (`0x4d9c98`, nodes `0x4d9f08..0x4d9fe0`).
const ROLE_0: RoleGraph = (
    &[&[0, 1, 2, 3, 4, 5, 6, 7, 8], &[9]],
    &[
        (0, IDLE, 0),
        (0, CHEER, 1),
        (0, CHEER, 2),
        (0, CHEER, 3),
        (0, CHEER, 4),
        (0, GASP, 5),
        (0, GASP, 6),
        (0, GASP, 7),
        (1, IDLE, 8),
        (0, IDLE, 9),
    ],
    10,
);
/// Role 1 (`0x4d9cf8`, nodes `0x4d9e48..0x4d9ef0`).
const ROLE_1: RoleGraph = (
    &[&[0, 1, 5, 2, 3, 4], &[6], &[7]],
    &[
        (0, CHEER, 0),
        (0, CHEER, 1),
        (0, GASP, 2),
        (0, GASP, 3),
        (1, IDLE, 4),
        (2, IDLE, 5),
        (0, IDLE, 6),
        (0, IDLE, 7),
    ],
    8,
);
/// Role 2 (`0x4d9d88`, nodes `0x4d9ff8..0x4da088`).
const ROLE_2: RoleGraph = (
    &[&[0, 1, 2, 3, 4], &[5], &[6]],
    &[
        (1, IDLE, 2),
        (2, IDLE, 3),
        (0, IDLE, 4),
        (0, CHEER, 5),
        (0, GASP, 6),
        (0, IDLE, 0),
        (0, IDLE, 1),
    ],
    7,
);
/// Role 4, the walkers (`0x4d9e18`, node `0x4da0a0`).
const ROLE_4: RoleGraph = (&[&[0]], &[(0, IDLE, 0)], 1);

impl Graph {
    /// `FUN_0030e3e0` for the roles present (role 3 has no graph).
    pub fn new(present: [bool; 5]) -> Graph {
        let mut graph = Graph {
            nodes: Vec::new(),
            lists: Vec::new(),
            roots: [None; 5],
            animations: 0,
        };
        for (role, (lists, nodes, animations)) in
            [(0, ROLE_0), (1, ROLE_1), (2, ROLE_2), (4, ROLE_4)]
        {
            if !present[role] {
                continue;
            }
            let (first_node, first_list) = (graph.nodes.len(), graph.lists.len());
            let velocity = if role == 4 {
                [0.0, 0.0, WALK]
            } else {
                [0.0; 3]
            };
            graph
                .nodes
                .extend(nodes.iter().map(|&(next, category, animation)| Node {
                    next: first_list + next,
                    category,
                    animation: graph.animations + animation,
                    velocity,
                }));
            graph.lists.extend(
                lists
                    .iter()
                    .map(|l| l.iter().map(|n| first_node + n).collect()),
            );
            graph.roots[role] = Some(first_list);
            graph.animations += animations;
        }
        graph
    }

    /// The animation records the models must have.
    pub fn animations(&self) -> usize {
        self.animations
    }

    /// The node a group of `role` starts on (`FUN_0030ea48`).
    fn start(&self, role: usize) -> Option<usize> {
        let list = (*self.roots.get(role)?)?;
        self.lists[list].first().copied()
    }
}

/// One model's playback of one group's animation (class `0x308830`'s
/// per-group record, 0x30 bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Track {
    animation: usize,
    /// Within the animation.
    frame: usize,
    /// Frames still to show.
    remaining: i64,
    timer: f32,
    period: f32,
}

impl Track {
    /// The constructor's record (`0x308830`).
    fn new() -> Track {
        Track {
            animation: 0,
            frame: 0,
            remaining: 0,
            timer: 1.0,
            period: 1.0,
        }
    }

    /// `+0x5c` (`0x309eb8`): true when the animation has ended.
    fn advance(&mut self, step: f32) -> bool {
        self.timer -= step;
        if self.timer > 0.0 {
            return false;
        }
        self.timer += self.period;
        if self.remaining == 0 {
            return true;
        }
        self.remaining -= 1;
        self.frame += 1;
        false
    }

    /// `+0x4c` (`0x309fa8`): the timer carries on.
    fn start(&mut self, period: f32, animation: usize, frames: usize) {
        self.period = period;
        self.animation = animation;
        self.remaining = frames as i64 - 1;
        self.frame = 0;
    }
}

/// One group's state (`DAT_00403048`, 0x18 bytes a group).
#[derive(Clone, Debug, PartialEq)]
struct GroupState {
    role: usize,
    node: Option<usize>,
    /// The category its next pick should have.
    wants: u8,
    walk_ticks: u32,
    /// How far its walkers have walked, in model units.
    walk: [f32; 3],
}

/// The original's crowd logic, without Bevy: the graphs, each group's
/// state and each model's playback of each group.
pub struct Director {
    graph: Graph,
    groups: Vec<GroupState>,
    /// Per model (the body first), per group.
    tracks: Vec<Vec<Track>>,
    /// Per model, per animation record: its frames.
    frames: Vec<Vec<usize>>,
    random: u32,
}

impl Director {
    /// The loader's set-up (`0x30b9b8`): roles from the seats, every group
    /// of every model on animation 0 at period 1, then each group run
    /// `rand() % 100 + 100` times. `frames` is per model (body first), per
    /// animation record.
    pub fn new(seats: &[Seat], frames: Vec<Vec<usize>>, seed: u32) -> Director {
        let count = seats.iter().map(|s| s.group + 1).max().unwrap_or(0);
        let mut present = [false; 5];
        for s in seats {
            present[s.role.min(4)] = true;
        }
        let graph = Graph::new(present);
        let mut groups = vec![
            GroupState {
                // `FUN_0030dc68` starts every group on role 5 (no graph).
                role: 5,
                node: None,
                wants: IDLE,
                walk_ticks: 0,
                walk: [0.0; 3],
            };
            count
        ];
        for s in seats {
            groups[s.group].role = s.role;
            groups[s.group].node = graph.start(s.role);
        }
        let tracks = frames
            .iter()
            .map(|f| {
                (0..count)
                    .map(|_| {
                        let mut t = Track::new();
                        t.start(START_PERIOD, 0, f.first().copied().unwrap_or(0));
                        t
                    })
                    .collect()
            })
            .collect();
        let mut director = Director {
            graph,
            groups,
            tracks,
            frames,
            random: seed,
        };
        for g in 0..count {
            let runs = director.draw() % 100 + 100;
            for _ in 0..runs {
                director.tick_group(g);
            }
        }
        director
    }

    /// `FUN_003b42a0`'s step: the rewrite's own state.
    fn draw(&mut self) -> u32 {
        self.random = self.random.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.random & 0x7fff_ffff
    }

    /// Groups (the plan's highest group + 1, crowd `+0x40`).
    pub fn groups(&self) -> usize {
        self.groups.len()
    }

    /// The category a group's next pick should have: 0 cheer, 1 gasp, 2
    /// idling.
    pub fn wants(&self, group: usize) -> u8 {
        self.groups[group].wants
    }

    /// The animation and frame `model` shows for `group`.
    pub fn pose(&self, model: usize, group: usize) -> (usize, usize) {
        let t = &self.tracks[model][group];
        (t.animation, t.frame)
    }

    /// How far a group's walkers have walked (model units, seat space).
    pub fn walk(&self, group: usize) -> [f32; 3] {
        self.groups[group].walk
    }

    /// `FUN_0030ca10`: `trunc(groups * fraction)` distinct groups, at
    /// random, want a cheer or a gasp next.
    pub fn react(&mut self, reaction: Reaction) {
        let count = self.groups.len();
        // cvt.w.s truncates.
        let picks = (count as f32 * reaction.fraction) as i32;
        let mut taken = vec![false; count];
        let mut left = count as u32;
        for _ in 0..picks.max(0) {
            if left == 0 {
                break;
            }
            let k = self.draw().wrapping_add(left) % left;
            let Some(g) = (0..count).filter(|&g| !taken[g]).nth(k as usize) else {
                break;
            };
            taken[g] = true;
            self.groups[g].wants = if reaction.cheer { CHEER } else { GASP };
            left -= 1;
        }
    }

    /// `FUN_0030db18`: one tick of every group.
    pub fn tick(&mut self) {
        for g in 0..self.groups.len() {
            self.tick_group(g);
        }
    }

    /// `FUN_0030dd50(1.0, group)`.
    fn tick_group(&mut self, g: usize) {
        // A group without seats has no node (the original never meets one).
        let Some(node) = self.groups[g].node else {
            return;
        };
        // The walk (`FUN_0030ebd8`): a step, or after 1800 the way back.
        let state = &mut self.groups[g];
        state.walk_ticks += 1;
        let v = self.graph.nodes[node].velocity;
        let k = if state.walk_ticks < WALK_TICKS {
            1.0
        } else {
            let k = -(state.walk_ticks as f32);
            state.walk_ticks = 0;
            k
        };
        for (w, v) in state.walk.iter_mut().zip(v) {
            *w += v * k;
        }
        let mut pick = None;
        if self.tracks[0][g].advance(1.0) {
            let nodes = &self.graph.nodes;
            let list = &self.graph.lists[nodes[node].next];
            let wants = self.groups[g].wants;
            let matching: Vec<usize> = list
                .iter()
                .copied()
                .filter(|&n| nodes[n].category == wants)
                .collect();
            let chosen = if matching.is_empty() {
                // The idling node back to the group's own list
                // (`PTR_DAT_00403058[role]`): the last such in the list.
                let root = self.graph.roots.get(self.groups[g].role).copied().flatten();
                list.iter()
                    .copied()
                    .rfind(|&n| nodes[n].category == IDLE && Some(nodes[n].next) == root)
            } else {
                let k = self.draw() as usize % matching.len();
                Some(matching[k])
            };
            let chosen = chosen.unwrap_or(node);
            let animation = self.graph.nodes[chosen].animation;
            let frames = self.frames[0].get(animation).copied().unwrap_or(1);
            self.tracks[0][g].start(PERIOD, animation, frames);
            self.groups[g].node = Some(chosen);
            self.groups[g].wants = IDLE;
            pick = Some(animation);
        }
        let current = self.graph.nodes[self.groups[g].node.unwrap_or(node)].animation;
        for m in 1..self.tracks.len() {
            if self.tracks[m][g].advance(1.0) {
                let animation = pick.unwrap_or(current);
                let frames = self.frames[m].get(animation).copied().unwrap_or(1);
                self.tracks[m][g].start(PERIOD, animation, frames);
            }
        }
    }
}

/// A (model, group) mesh and the (animation, frame) it shows.
type GroupMesh = (Handle<Mesh>, Option<(usize, usize)>);

/// The animated crowd of the current court.
#[derive(Component)]
pub struct CrowdRig {
    models: Vec<Model>,
    /// Per model: its corners (vertex indices), for rebuilding positions.
    corners: Vec<Vec<u16>>,
    /// Per model, per group: the shared mesh and the pose it shows.
    meshes: Vec<Vec<Option<GroupMesh>>>,
    /// The walkers: their seat entity, group and transform at rest.
    walkers: Vec<(Entity, usize, Transform)>,
    director: Director,
}

impl CrowdRig {
    pub fn director(&self) -> &Director {
        &self.director
    }
}

/// A seat's vertex colour ([`Seat::vertex_colour`], 0x80 = 1.0) as the
/// material's base colour. The GS modulates the texture by it in its own
/// (display) values, `texel * colour >> 7`; Bevy multiplies in linear
/// light, so the factor goes in as sRGB to scale the displayed texel by
/// the same amount.
/// The seat's vertex colour as the GS modulates the texel with it
/// (`texel * colour >> 7`, red, green, blue and alpha alike): its alpha
/// byte is 0x7f for the plans' 0xff (`0x30a450` halves it), so a texel
/// keeps 127/128 of its alpha.
fn tint_colour(c: [u8; 4]) -> Color {
    let k = |v: u8| f32::from(v) / 128.0;
    Color::srgba(k(c[0]), k(c[1]), k(c[2]), k(c[3]).min(1.0))
}

/// A model's triangles as a mesh: one vertex per corner (texture coordinates
/// differ per corner), positions from the rest pose.
fn mesh(model: &Model, size: [f32; 2]) -> (Mesh, Vec<u16>) {
    let mut positions = Vec::with_capacity(model.triangles.len() * 3);
    let mut uvs = Vec::with_capacity(model.triangles.len() * 3);
    let mut corners = Vec::with_capacity(model.triangles.len() * 3);
    for tri in &model.triangles {
        for c in tri {
            positions.push(model.rest[c.vertex as usize]);
            uvs.push([c.texel[0] / size[0], c.texel[1] / size[1]]);
            corners.push(c.vertex);
        }
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    (mesh, corners)
}

/// Spawns the crowd; returns its root (despawn it to remove the crowd).
pub fn spawn(
    commands: &mut Commands,
    crowd: &Crowd,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Entity {
    // Models: the body, then the heads the seats use (`0x30b9b8` loads a
    // head kind only when a seat's plan column 8 names it).
    let mut models = vec![crowd.body.clone()];
    let mut head_model = [None; 4];
    for (k, h) in crowd.heads.iter().enumerate() {
        if let Some(h) = h
            && crowd.seats.iter().any(|s| s.head == k)
        {
            head_model[k] = Some(models.len());
            models.push(h.clone());
        }
    }
    let head_of = |seat: &Seat| head_model.get(seat.head).copied().flatten();
    let groups = crowd.seats.iter().map(|s| s.group + 1).max().unwrap_or(0);
    let mut corners = Vec::new();
    let mut model_meshes = Vec::new();
    for (m, model) in models.iter().enumerate() {
        let (_, c) = mesh(model, [64.0, 64.0]);
        corners.push(c);
        model_meshes.push(
            (0..groups)
                .map(|g| {
                    let used = crowd
                        .seats
                        .iter()
                        .any(|s| s.group == g && (m == 0 || head_of(s) == Some(m)));
                    used.then(|| (meshes.add(mesh(model, [64.0, 64.0]).0), None))
                })
                .collect::<Vec<_>>(),
        );
    }

    let mut material_for: HashMap<(usize, usize, [u8; 4]), Handle<StandardMaterial>> =
        HashMap::new();
    let mut material = |sheet: usize, image: usize, tint: [u8; 4]| -> Handle<StandardMaterial> {
        material_for
            .entry((sheet, image, tint))
            .or_insert_with(|| {
                let list = if sheet == 0 {
                    &crowd.body_sheet
                } else {
                    &crowd.head_sheets[sheet - 1]
                };
                let texture = (!list.is_empty()).then(|| &list[image % list.len()].1);
                materials.add(StandardMaterial {
                    base_color: tint_colour(tint),
                    base_color_texture: texture.map(|t| images.add(convert::image(t))),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            })
            .clone()
    };

    let root = commands
        .spawn((
            Name::new("crowd"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    // The heights' draws (the original's come from `FUN_003a16b8`).
    let mut random = 0x2545_f491u32;
    let mut walkers: Vec<(Entity, usize, Transform)> = Vec::new();
    let mut walking = vec![false; groups];
    for seat in &crowd.seats {
        random = random.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let (lo, hi) = if matches!(seat.head, 2 | 3) {
            (5.25, 5.916_666_5)
        } else {
            (5.5, 6.25)
        };
        let height = lo + (hi - lo) * (random & 0x7fff_ffff) as f32 * 4.656_613e-10;
        let [rx, ry, rz] = seat.rotation_degrees.map(f32::to_radians);
        let transform = Transform {
            translation: Vec3::from_array(seat.position),
            rotation: Quat::from_euler(EulerRot::YXZ, ry, rx, rz),
            scale: Vec3::new(SCALE, height / 6.5 * SCALE, SCALE),
        };
        let spectator = commands
            .spawn((
                Name::new(seat.name.clone()),
                transform,
                Visibility::default(),
            ))
            .id();
        commands.entity(root).add_child(spectator);
        // A role 4 seat gives its group a list of five walkers
        // (`FUN_0030ea48`); each seat of the group from then on joins it,
        // the last entry taking any beyond five (`FUN_0030e2c0`).
        walking[seat.group] |= seat.role == 4;
        if walking[seat.group] {
            let mine: Vec<usize> = (0..walkers.len())
                .filter(|&i| walkers[i].1 == seat.group)
                .collect();
            if mine.len() == WALKERS {
                walkers[mine[WALKERS - 1]] = (spectator, seat.group, transform);
            } else {
                walkers.push((spectator, seat.group, transform));
            }
        }
        // Body and head take the same colour (`FUN_0030e250`).
        let tint = seat.vertex_colour();
        if let Some(Some((body, _))) = model_meshes[0].get(seat.group) {
            let part = commands
                .spawn((
                    Mesh3d(body.clone()),
                    MeshMaterial3d(material(0, seat.textures[0], tint)),
                    NoFrustumCulling,
                ))
                .id();
            commands.entity(spectator).add_child(part);
        }
        if let Some(m) = head_of(seat)
            && let Some(Some((head, _))) = model_meshes[m].get(seat.group)
        {
            let part = commands
                .spawn((
                    Mesh3d(head.clone()),
                    MeshMaterial3d(material(seat.head + 1, seat.textures[1], tint)),
                    NoFrustumCulling,
                ))
                .id();
            commands.entity(spectator).add_child(part);
        }
    }
    let frames = models
        .iter()
        .map(|m| m.animations.iter().map(|a| a.frames).collect())
        .collect();
    let director = Director::new(&crowd.seats, frames, 0x9e37_79b9);
    commands.entity(root).insert(CrowdRig {
        models,
        corners,
        meshes: model_meshes,
        walkers,
        director,
    });
    root
}

pub fn plugin(app: &mut App) {
    app.init_resource::<CrowdReactions>()
        .add_systems(Update, show);
}

/// One crowd tick (`FUN_0030c9e0` → `FUN_0030db18`): this tick's calls,
/// then every group. The game runs it at 60 Hz while the pause menu is
/// closed.
pub fn step(mut reactions: ResMut<CrowdReactions>, mut rigs: Query<&mut CrowdRig>) {
    let calls = std::mem::take(&mut reactions.0);
    for mut rig in &mut rigs {
        for &call in &calls {
            rig.director.react(call);
        }
        rig.director.tick();
    }
}

/// Writes the frames the groups moved on to into their meshes, and places
/// the walkers.
fn show(
    mut rigs: Query<&mut CrowdRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut transforms: Query<&mut Transform>,
) {
    let mut pose = Vec::new();
    for mut rig in &mut rigs {
        let rig = &mut *rig;
        for (m, groups) in rig.meshes.iter_mut().enumerate() {
            for (g, slot) in groups.iter_mut().enumerate() {
                let Some((handle, shown)) = slot else {
                    continue;
                };
                let now = rig.director.pose(m, g);
                if *shown == Some(now) {
                    continue;
                }
                let Some(anim) = rig.models[m].animations.get(now.0) else {
                    continue;
                };
                anim.pose(now.1, &mut pose);
                let positions: Vec<[f32; 3]> =
                    rig.corners[m].iter().map(|&v| pose[v as usize]).collect();
                if let Some(mut mesh) = meshes.get_mut(&*handle) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                    *shown = Some(now);
                }
            }
        }
        for &(entity, group, rest) in &rig.walkers {
            let walk = Vec3::from_array(rig.director.walk(group));
            if let Ok(mut t) = transforms.get_mut(entity) {
                // The step is in the seat's own space (`FUN_0030eb20`
                // multiplies a translation in ahead of the seat's matrix).
                t.translation = rest.translation + rest.rotation * (rest.scale * walk);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(name: &str, group: usize) -> Seat {
        Seat {
            name: name.into(),
            head: 0,
            role: street_assets::crowd::role(name),
            rotation_degrees: [0.0; 3],
            position: [0.0; 3],
            group,
            colour: [0x80; 4],
            textures: [0, 0],
        }
    }

    /// A one-triangle model with one still animation.
    fn little_model(name: &str) -> Model {
        let rest = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let corner = |vertex| street_assets::crowd::Corner {
            vertex,
            texel: [0.0, 0.0],
        };
        Model {
            name: name.into(),
            vertices: 3,
            rest: rest.clone(),
            triangles: vec![[corner(0), corner(1), corner(2)]],
            animations: vec![street_assets::crowd::Animation {
                rate: 60,
                key: rest,
                deltas: Vec::new(),
                scale: [0.0; 3],
                frames: 0,
            }],
        }
    }

    /// `FUN_0030e250` colours the head and the body alike with the plan's
    /// word through `0x30a450`: `0xb65a9bff` draws both at (0x5b, 0x4d,
    /// 0x2d) / 0x80. The rewrite had drawn the body at (0xb6, 0x5a, 0x9b)
    /// / 0x80 in linear light, a saturated pink that flattened the shirts'
    /// shading into shards, and the head untinted.
    #[test]
    fn head_and_body_share_the_seats_colour() {
        let mut s = seat("B_ST258", 0);
        s.colour = [0xb6, 0x5a, 0x9b, 0xff];
        let crowd = Crowd {
            seats: vec![s],
            body: little_model("spec"),
            heads: [Some(little_model("head")), None, None, None],
            body_sheet: Vec::new(),
            head_sheets: Default::default(),
        };
        let mut world = World::new();
        let mut materials = Assets::<StandardMaterial>::default();
        let root = spawn(
            &mut world.commands(),
            &crowd,
            &mut Assets::<Mesh>::default(),
            &mut materials,
            &mut Assets::<Image>::default(),
        );
        world.flush();
        let spectator = world.get::<Children>(root).unwrap()[0];
        let parts: Vec<Color> = world
            .get::<Children>(spectator)
            .unwrap()
            .iter()
            .map(|part| {
                let handle = &world
                    .get::<MeshMaterial3d<StandardMaterial>>(part)
                    .unwrap()
                    .0;
                materials.get(handle).unwrap().base_color
            })
            .collect();
        // The alpha byte 0x7f (0xff halved) as well: 127/128.
        let want = Color::srgba(91.0 / 128.0, 77.0 / 128.0, 45.0 / 128.0, 127.0 / 128.0);
        assert_eq!(parts, [want, want], "body then head");
    }

    /// The category of the node a group is on.
    fn category(d: &Director, g: usize) -> u8 {
        d.graph.nodes[d.groups[g].node.unwrap()].category
    }

    #[test]
    fn the_graphs_index_the_roles_animations_in_order() {
        let all = Graph::new([true, true, true, false, true]);
        assert_eq!(all.animations(), 26);
        // Each role's first node and its own animations.
        let start = |g: &Graph, role| g.nodes[g.start(role).unwrap()].animation;
        assert_eq!([0, 1, 2, 4].map(|r| start(&all, r)), [0, 10, 18 + 2, 25]);
        // Without role 0, role 1 starts at animation 0 (Sol's crowd: 15).
        let sol = Graph::new([false, true, true, false, false]);
        assert_eq!(
            (sol.animations(), start(&sol, 1), start(&sol, 2)),
            (15, 0, 10)
        );
        assert_eq!(
            Graph::new([true, true, false, false, false]).animations(),
            18
        );
        // Whatever a group wants, every list of its role has a next node: one
        // of that category, or idling back to the role's own list.
        for (role, root) in all.roots.iter().enumerate() {
            let Some(root) = *root else { continue };
            let mut lists = vec![root];
            let mut i = 0;
            while i < lists.len() {
                for &n in &all.lists[lists[i]] {
                    if !lists.contains(&all.nodes[n].next) {
                        lists.push(all.nodes[n].next);
                    }
                }
                i += 1;
            }
            for &l in &lists {
                for wants in [CHEER, GASP, IDLE] {
                    let list = &all.lists[l];
                    assert!(
                        list.iter().any(|&n| all.nodes[n].category == wants)
                            || list.iter().any(|&n| {
                                all.nodes[n].category == IDLE && all.nodes[n].next == root
                            }),
                        "role {role} list {l} wanting {wants}"
                    );
                }
            }
        }
    }

    /// `0x309fa8`/`0x309eb8`: at period 4 a three-frame animation started on
    /// the tick the last one ended shows each frame four ticks (counting
    /// that tick) and reports its end on the twelfth.
    #[test]
    fn a_frame_shows_four_ticks() {
        let mut t = Track::new();
        t.start(PERIOD, 0, 1);
        assert!(t.advance(1.0));
        t.start(PERIOD, 7, 3);
        let mut seen = Vec::new();
        let mut ended = None;
        for tick in 1..=12 {
            if t.advance(1.0) {
                ended = Some(tick);
                break;
            }
            seen.push(t.frame);
        }
        assert_eq!(seen, [0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(ended, Some(12));
    }

    #[test]
    fn the_share_of_groups_follows_the_lead() {
        assert_eq!(fraction(0, 21), 0.5);
        assert_eq!(fraction(21, 21), 1.0);
        assert_eq!(fraction(30, 21), 1.0);
        assert_eq!(fraction(7, 0), 7.0 / 21.0 * 0.5 + 0.5);
    }

    /// `FUN_0030ca10`: trunc(groups × share) distinct groups take the call.
    #[test]
    fn a_reaction_reaches_its_share_of_groups() {
        let seats: Vec<Seat> = (0..8).map(|g| seat("A_ST1", g)).collect();
        let mut d = Director::new(&seats, vec![vec![10; 10]], 7);
        assert!((0..8).all(|g| d.wants(g) == IDLE));
        d.react(Reaction {
            fraction: 0.66,
            cheer: false,
        });
        let gasping = (0..8).filter(|&g| d.wants(g) == GASP).count();
        assert_eq!(gasping, 5); // trunc(8 × 0.66)
        d.react(Reaction {
            fraction: 1.0,
            cheer: true,
        });
        assert!((0..8).all(|g| d.wants(g) == CHEER));
    }

    /// Calm groups only idle; a cheer is taken up when the current
    /// animation ends, then the group idles again. Heads keep the body's
    /// animation and frame.
    #[test]
    fn groups_cheer_at_the_end_of_their_animation() {
        let seats: Vec<Seat> = (0..4).map(|g| seat("A_ST1", g)).collect();
        let frames = vec![vec![5; 10], vec![5; 10]];
        let mut d = Director::new(&seats, frames, 3);
        for _ in 0..2000 {
            d.tick();
            for g in 0..4 {
                assert_eq!(category(&d, g), IDLE);
                assert_eq!(d.pose(0, g), d.pose(1, g));
            }
        }
        d.react(Reaction {
            fraction: 1.0,
            cheer: true,
        });
        // A group whose next list has no cheer (node 8's) drops the call.
        let can: Vec<bool> = (0..4)
            .map(|g| {
                let node = d.groups[g].node.unwrap();
                d.graph.lists[d.graph.nodes[node].next]
                    .iter()
                    .any(|&n| d.graph.nodes[n].category == CHEER)
            })
            .collect();
        let mut cheered = [None; 4];
        for tick in 0..400 {
            d.tick();
            for (g, at) in cheered.iter_mut().enumerate() {
                if category(&d, g) == CHEER && at.is_none() {
                    *at = Some(tick);
                    assert_eq!(d.wants(g), IDLE);
                }
                assert_eq!(d.pose(0, g), d.pose(1, g));
            }
        }
        // Five frames of four ticks: the longest wait is one animation.
        for (g, at) in cheered.iter().enumerate() {
            if can[g] {
                assert!(at.is_some_and(|t| t < 20), "group {g}: {at:?}");
            } else {
                assert_eq!(*at, None, "group {g}");
            }
        }
        for _ in 0..40 {
            d.tick();
        }
        assert!((0..4).all(|g| category(&d, g) == IDLE));
    }

    /// Role 1 offers no cheer from its interlude lists: a call that lands
    /// there is dropped, as `FUN_0030dd50` drops it.
    #[test]
    fn a_call_on_an_interlude_is_dropped() {
        let seats = [seat("A_SD1", 0)];
        let mut d = Director::new(&seats, vec![vec![3; 8]], 11);
        // Run until the group is on node ea8 (animation 4, list D).
        let mut guard = 0;
        while d.graph.nodes[d.groups[0].node.unwrap()].animation != 4 {
            d.tick();
            guard += 1;
            assert!(guard < 100_000);
        }
        d.react(Reaction {
            fraction: 1.0,
            cheer: true,
        });
        while d.graph.nodes[d.groups[0].node.unwrap()].animation == 4 {
            d.tick();
        }
        assert_eq!(d.graph.nodes[d.groups[0].node.unwrap()].animation, 6);
        assert_eq!(d.wants(0), IDLE);
    }

    /// Role 4 walks 0.6855 model units a tick along z and every 1800 ticks
    /// steps back the whole way (`FUN_0030ebd8`).
    #[test]
    fn walkers_walk_and_step_back() {
        let seats = [seat("G_WK1", 0)];
        let mut d = Director::new(&seats, vec![vec![4; 1]], 5);
        let start = d.walk(0)[2];
        let ticks = d.groups[0].walk_ticks;
        d.tick();
        assert_eq!(d.walk(0)[2], start + WALK);
        for _ in 0..(WALK_TICKS - ticks - 2) {
            d.tick();
        }
        assert_eq!(d.groups[0].walk_ticks, WALK_TICKS - 1);
        let before = d.walk(0)[2];
        d.tick();
        assert_eq!(d.groups[0].walk_ticks, 0);
        assert!((d.walk(0)[2] - (before - WALK * WALK_TICKS as f32)).abs() < 1e-2);
        assert_eq!(d.walk(0)[0], 0.0);
    }

    /// Every court's models have exactly the records its roles' graphs
    /// index (`FUN_0030e3e0`), and its crowd runs.
    #[test]
    fn every_courts_graph_fits_its_models() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            eprintln!("STREET_ISO not set; skipping");
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).expect("STREET_ISO");
        for court in [
            "CENTER", "FOREST", "FOSTER", "INDOOR", "MOSSWD", "NYCAGE", "RUCKER", "RUCKRC",
            "SEATTL", "SOLHOL",
        ] {
            let c = street_assets::crowd::load(&mut iso, court).unwrap();
            let mut present = [false; 5];
            for s in &c.seats {
                present[s.role] = true;
            }
            let graph = Graph::new(present);
            assert_eq!(graph.animations(), c.body.animations.len(), "{court}");
            // Each group is one role (the last seat sets it).
            for g in 0..=c.seats.iter().map(|s| s.group).max().unwrap() {
                let roles: std::collections::BTreeSet<usize> = c
                    .seats
                    .iter()
                    .filter(|s| s.group == g)
                    .map(|s| s.role)
                    .collect();
                assert!(roles.len() <= 1, "{court} group {g}: {roles:?}");
            }
            let frames = std::iter::once(&c.body)
                .chain(c.heads.iter().flatten())
                .map(|m| m.animations.iter().map(|a| a.frames).collect())
                .collect();
            let mut d = Director::new(&c.seats, frames, 1);
            d.react(Reaction {
                fraction: 1.0,
                cheer: true,
            });
            for _ in 0..600 {
                d.tick();
                for g in 0..d.groups() {
                    let (a, f) = d.pose(0, g);
                    assert!(f < c.body.animations[a].frames, "{court} group {g}");
                }
            }
        }
    }
}
