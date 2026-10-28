//! The arena behind the launcher: myballer.online's court (`arena.js`), drawn
//! with egui's painter. The same night court and stands, seen from the
//! site's first view (from (23, 18, 27) toward (0, 0, -1), a 43° lens, the
//! picture moved a fifth of the way right on a wide window), and moved by the
//! cursor as the site moves it: 0.6 across and 0.25 up for the pointer's way
//! from the middle of the window.
//!
//! egui has no depth buffer, so the floor (the apron, the boards and the mark
//! at center court) is drawn first and everything standing on it far to near;
//! the lights are the site's (an ambient fill, a warm key, a violet and a
//! lavender point light) worked out per vertex and tone-mapped, and its fog
//! darkens with distance.

use eframe::egui::{
    self, Color32, ColorImage, Mesh, Pos2, Rect, Shape, Stroke, TextureFilter, TextureHandle,
    TextureId, TextureOptions, Vec2, epaint::Vertex,
};

type V3 = [f32; 3];

const COURT_W: f32 = 15.0;
const COURT_L: f32 = 28.0;
/// `views[0]`: where the camera stands and what it looks at.
const EYE: V3 = [23.0, 18.0, 27.0];
const LOOK: V3 = [0.0, 0.0, -1.0];
const FOV_DEGREES: f32 = 43.0;
/// The pointer's reach (`targetPosition.x += pointerX*.6`, `.y += pointerY*.25`).
const REACH: Vec2 = Vec2::new(0.6, 0.25);
/// `FogExp2(0x07040d, .016)`.
const FOG_DENSITY: f32 = 0.016;

pub struct Arena {
    court: TextureHandle,
    mark: TextureHandle,
    banner: TextureHandle,
    crowd: Vec<(V3, Color32)>,
    /// The pointer's way from the middle of the window, eased.
    pointer: Vec2,
}

impl Arena {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut rng = Lcg(9);
        let court = ctx.load_texture("court", court_image(&mut rng), mipmapped());
        let crowd = crowd(&mut rng);
        let png = |bytes: &[u8]| {
            let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .expect("the launcher's pictures are PNGs")
                .into_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            ColorImage::from_rgba_unmultiplied(size, image.as_raw())
        };
        let mark = ctx.load_texture("mb-mark", png(MARK_PNG), mipmapped());
        let banner = ctx.load_texture(
            "banner",
            png(include_bytes!("../assets/myballer-banner.png")),
            mipmapped(),
        );
        Self {
            court,
            mark,
            banner,
            crowd,
            pointer: Vec2::ZERO,
        }
    }

    /// The MB mark, for the buttons.
    pub fn mark(&self) -> &TextureHandle {
        &self.mark
    }

    /// One frame of the arena over `screen`, the pointer at `pointer` (none
    /// off the window: it stays where it was) after `dt` seconds.
    pub fn draw(&mut self, painter: &egui::Painter, screen: Rect, pointer: Option<Pos2>, dt: f32) {
        if let Some(p) = pointer {
            let want = Vec2::new(
                (p.x - screen.left()) / screen.width() - 0.5,
                0.5 - (p.y - screen.top()) / screen.height(),
            );
            self.pointer += (want - self.pointer) * (dt * 6.0).min(1.0);
        }
        let eye = [
            EYE[0] + self.pointer.x * REACH.x,
            EYE[1] + self.pointer.y * REACH.y,
            EYE[2],
        ];
        let camera = Camera::new(eye, LOOK, screen);
        let mut scene = Scene {
            camera: &camera,
            items: Vec::new(),
        };

        // The floor, in order: the apron, the boards, the stripes, the mark.
        let apron = rgb(0x180d24);
        floor_quad(
            painter,
            &camera,
            [-9.5, -0.03, -16.0],
            [19.0, 32.0],
            lit_color(apron, [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]),
        );
        textured_floor(
            painter,
            &camera,
            self.court.id(),
            [-COURT_W / 2.0, 0.0, -COURT_L / 2.0],
            [COURT_W, COURT_L],
            [15, 28],
            Shade::Lit,
            1.0,
        );
        let stripe = alpha_of(rgb(0x8e36d9), 0.6);
        for x in [-9.0, 9.0] {
            scene.line([x, 0.02, -15.5], [x, 0.02, 15.5], 0.035, stripe);
        }
        let stripes = std::mem::take(&mut scene.items);
        scene.paint(painter, stripes);
        // The mark lies across the court's width, its top toward +z as the
        // plane's rotation lays it.
        textured_floor(
            painter,
            &camera,
            self.mark.id(),
            [-1.9, 0.025, -1.31],
            [3.8, 2.62],
            [4, 4],
            Shade::Basic,
            0.9,
        );

        // What stands on it.
        let steel = rgb(0x17121d);
        hoop(&mut scene, -12.7, false, steel);
        hoop(&mut scene, 12.7, true, steel);
        let stand = rgb(0x130b1d);
        let rail = alpha_of(rgb(0x6e248f), 0.45);
        for side in [-1.0_f32, 1.0] {
            for row in 0..7 {
                let x = side * (10.2 + row as f32 * 1.25);
                let y = 0.3 + row as f32 * 0.6;
                scene.cube([x, y, 0.0], [1.2, 0.45, 34.0], stand, true);
                scene.line([x, y + 0.26, -17.0], [x, y + 0.26, 17.0], 0.035, rail);
            }
        }
        scene.cube([0.0, 2.3, -18.0], [38.0, 5.0, 2.0], stand, true);
        // The billboard: a steel frame and the banner on it.
        scene.cube([0.0, 7.8, -17.62], [10.25, 5.88, 0.2], steel, true);
        scene.picture(
            self.banner.id(),
            [
                [-5.0, 10.6125, -17.5],
                [5.0, 10.6125, -17.5],
                [5.0, 4.9875, -17.5],
                [-5.0, 4.9875, -17.5],
            ],
        );
        // The rafters' light bars.
        for x in [-11.0, 11.0] {
            scene.line(
                [x - 1.5, 14.0, -9.0],
                [x + 1.5, 14.0, -9.0],
                0.07,
                rgb(0xffead5),
            );
        }
        for &(p, c) in &self.crowd {
            scene.dot(p, 0.07, alpha_of(c, 0.6));
        }
        let items = std::mem::take(&mut scene.items);
        scene.paint(painter, items);
    }
}

pub const MARK_PNG: &[u8] = include_bytes!("../assets/mb-mark.png");

fn mipmapped() -> TextureOptions {
    TextureOptions {
        mipmap_mode: Some(TextureFilter::Linear),
        ..TextureOptions::LINEAR
    }
}

/// The site's generator: `seed = seed*1664525 + 1013904223`, over 2³².
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (f64::from(self.0) / 4_294_967_296.0) as f32
    }
}

/// `courtTexture()`: 64 warm boards, 1,250 grain streaks, the lines in
/// warm white and the paint in the MyBaller purple, 1024 by 1912.
fn court_image(rng: &mut Lcg) -> ColorImage {
    let (w, h) = (1024usize, 1912usize);
    let mut px = vec![[0u8; 4]; w * h];
    let sx = w as f32 / COURT_W;
    let sy = h as f32 / COURT_L;
    for b in 0..64 {
        let shade = 150 + (b * 37) % 23 - 11;
        let c = [shade + 36, shade - 8, shade - 62].map(|v: i32| v.clamp(0, 255) as u8);
        let (x0, x1) = (b as usize * w / 64, ((b + 1) as usize * w / 64 + 1).min(w));
        for y in 0..h {
            for p in &mut px[y * w + x0..y * w + x1] {
                *p = [c[0], c[1], c[2], 255];
            }
        }
    }
    for _ in 0..1250 {
        let a = 0.04 + rng.next() * 0.05;
        let x = (rng.next() * w as f32) as usize;
        let y0 = (rng.next() * h as f32) as usize;
        let len = (25.0 + rng.next() * 110.0) as usize;
        for y in y0..(y0 + len).min(h) {
            blend(&mut px[y * w + x.min(w - 1)], [60, 30, 10], a);
        }
    }
    let mut canvas = Canvas {
        px: &mut px,
        w,
        h,
        width: 0.045 * sx,
    };
    let ink = [0xff, 0xf5, 0xe0];
    let cx = w as f32 / 2.0;
    let (wf, hf) = (w as f32, h as f32);
    canvas.rect_outline([0.12 * sx, 0.12 * sy], [14.76 * sx, 27.76 * sy], ink);
    canvas.path(&[[0.0, hf / 2.0], [wf, hf / 2.0]], ink);
    canvas.arc([cx, hf / 2.0], 1.8 * sx, 0.0, std::f32::consts::TAU, ink);
    for end in [false, true] {
        let y0 = if end { hf } else { 0.0 };
        let dir = if end { -1.0 } else { 1.0 };
        let top = if end { y0 - 5.8 * sy } else { y0 };
        canvas.fill(
            [cx - 2.45 * sx, top],
            [4.9 * sx, 5.8 * sy],
            [0x79, 0x2a, 0xc1],
        );
        canvas.rect_outline([cx - 2.45 * sx, top], [4.9 * sx, 5.8 * sy], ink);
        canvas.arc(
            [cx, y0 + dir * 5.8 * sy],
            1.8 * sx,
            0.0,
            std::f32::consts::TAU,
            ink,
        );
        let rim_y = y0 + dir * 1.575 * sy;
        let a = (6.6_f32 / 6.75).acos();
        let corner = rim_y + dir * 1.2 * sy;
        canvas.path(&[[cx - 6.6 * sx, y0], [cx - 6.6 * sx, corner]], ink);
        canvas.path(&[[cx + 6.6 * sx, y0], [cx + 6.6 * sx, corner]], ink);
        let (from, to) = if end {
            (std::f32::consts::PI + a, std::f32::consts::TAU - a)
        } else {
            (a, std::f32::consts::PI - a)
        };
        canvas.arc([cx, rim_y], 6.75 * sx, from, to, ink);
    }
    let rgba: Vec<u8> = px.into_iter().flatten().collect();
    ColorImage::from_rgba_unmultiplied([w, h], &rgba)
}

fn blend(p: &mut [u8; 4], c: [u8; 3], a: f32) {
    for k in 0..3 {
        p[k] = (f32::from(p[k]) * (1.0 - a) + f32::from(c[k]) * a).round() as u8;
    }
}

/// Lines on the court's picture: round dabs along the path.
struct Canvas<'a> {
    px: &'a mut [[u8; 4]],
    w: usize,
    h: usize,
    width: f32,
}

impl Canvas<'_> {
    fn dab(&mut self, at: [f32; 2], c: [u8; 3]) {
        let r = self.width / 2.0;
        let (x0, x1) = ((at[0] - r - 1.0).floor(), (at[0] + r + 1.0).ceil());
        let (y0, y1) = ((at[1] - r - 1.0).floor(), (at[1] + r + 1.0).ceil());
        for y in y0.max(0.0) as usize..(y1.max(0.0) as usize).min(self.h) {
            for x in x0.max(0.0) as usize..(x1.max(0.0) as usize).min(self.w) {
                let d =
                    ((x as f32 + 0.5 - at[0]).powi(2) + (y as f32 + 0.5 - at[1]).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    blend(&mut self.px[y * self.w + x], c, a);
                }
            }
        }
    }

    fn path(&mut self, points: &[[f32; 2]], c: [u8; 3]) {
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            let n = (len * 2.0).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                self.dab([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], c);
            }
        }
    }

    fn arc(&mut self, center: [f32; 2], r: f32, from: f32, to: f32, c: [u8; 3]) {
        let n = ((to - from).abs() * r).ceil().max(8.0) as usize;
        let points: Vec<[f32; 2]> = (0..=n)
            .map(|k| {
                let t = from + (to - from) * k as f32 / n as f32;
                [center[0] + r * t.cos(), center[1] + r * t.sin()]
            })
            .collect();
        self.path(&points, c);
    }

    fn rect_outline(&mut self, at: [f32; 2], size: [f32; 2], c: [u8; 3]) {
        let [x, y] = at;
        let [w, h] = size;
        self.path(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]], c);
    }

    fn fill(&mut self, at: [f32; 2], size: [f32; 2], c: [u8; 3]) {
        for y in at[1].max(0.0) as usize..((at[1] + size[1]) as usize).min(self.h) {
            for x in at[0].max(0.0) as usize..((at[0] + size[0]) as usize).min(self.w) {
                let p = &mut self.px[y * self.w + x];
                *p = [c[0], c[1], c[2], 255];
            }
        }
    }
}

/// The crowd's 650 lights, after the court's draws from the same generator:
/// a third behind the far end, a third along each side, row by row up the
/// stands, one in seven warm.
fn crowd(rng: &mut Lcg) -> Vec<(V3, Color32)> {
    (0..650)
        .map(|i| {
            let side = i % 3;
            let row = rng.next() * 7.0;
            let p = if side == 0 {
                [(rng.next() - 0.5) * 36.0, 1.0 + row * 0.8, -19.0 - row]
            } else {
                let s = if side == 1 { -1.0 } else { 1.0 };
                [
                    s * (10.0 + row * 1.25),
                    0.8 + row * 0.65,
                    (rng.next() - 0.5) * 34.0,
                ]
            };
            let c = if i % 7 == 0 {
                rgb(0xffe5bb)
            } else {
                rgb(0xb9a4d6)
            };
            (p, c)
        })
        .collect()
}

struct Camera {
    eye: V3,
    right: V3,
    up: V3,
    fwd: V3,
    /// Pixels per unit at depth 1.
    scale: f32,
    center: Pos2,
}

impl Camera {
    fn new(eye: V3, look: V3, screen: Rect) -> Self {
        let fwd = norm(sub(look, eye));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        let scale = screen.height() / 2.0 / (FOV_DEGREES.to_radians() / 2.0).tan();
        // `setViewOffset(w, h, -w*.2, 0, w, h)` on a window wider than 850.
        let shift = if screen.width() > 850.0 {
            screen.width() * 0.2
        } else {
            0.0
        };
        Self {
            eye,
            right,
            up,
            fwd,
            scale,
            center: screen.center() + Vec2::new(shift, 0.0),
        }
    }

    /// Where `p` falls on the screen and how far ahead it lies.
    fn project(&self, p: V3) -> Option<(Pos2, f32)> {
        let d = sub(p, self.eye);
        let z = dot(d, self.fwd);
        if z < 0.2 {
            return None;
        }
        let x = dot(d, self.right) / z * self.scale;
        let y = dot(d, self.up) / z * self.scale;
        Some((self.center + Vec2::new(x, -y), z))
    }

    /// `FogExp2`: how much of a colour survives at depth `z`.
    fn clear(z: f32) -> f32 {
        (-(FOG_DENSITY * z).powi(2)).exp()
    }
}

enum Item {
    Poly {
        points: Vec<Pos2>,
        color: Color32,
    },
    Picture {
        points: [Pos2; 4],
        texture: TextureId,
        tint: Color32,
    },
    Line {
        a: Pos2,
        b: Pos2,
        width: f32,
        color: Color32,
    },
    Dot {
        at: Pos2,
        radius: f32,
        color: Color32,
    },
}

/// What stands on the floor, collected with each item's depth and drawn far
/// to near.
struct Scene<'a> {
    camera: &'a Camera,
    items: Vec<(f32, Item)>,
}

impl Scene<'_> {
    /// A box, its faces turned toward the camera, each lit by its normal.
    fn cube(&mut self, center: V3, size: V3, color: Color32, lit: bool) {
        let h = size.map(|v| v / 2.0);
        let c = center;
        let corner =
            |sx: f32, sy: f32, sz: f32| [c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sz * h[2]];
        let faces: [(V3, [V3; 4]); 6] = [
            (
                [1.0, 0.0, 0.0],
                [
                    corner(1., -1., -1.),
                    corner(1., 1., -1.),
                    corner(1., 1., 1.),
                    corner(1., -1., 1.),
                ],
            ),
            (
                [-1.0, 0.0, 0.0],
                [
                    corner(-1., -1., 1.),
                    corner(-1., 1., 1.),
                    corner(-1., 1., -1.),
                    corner(-1., -1., -1.),
                ],
            ),
            (
                [0.0, 1.0, 0.0],
                [
                    corner(-1., 1., -1.),
                    corner(-1., 1., 1.),
                    corner(1., 1., 1.),
                    corner(1., 1., -1.),
                ],
            ),
            (
                [0.0, -1.0, 0.0],
                [
                    corner(-1., -1., 1.),
                    corner(-1., -1., -1.),
                    corner(1., -1., -1.),
                    corner(1., -1., 1.),
                ],
            ),
            (
                [0.0, 0.0, 1.0],
                [
                    corner(-1., -1., 1.),
                    corner(1., -1., 1.),
                    corner(1., 1., 1.),
                    corner(-1., 1., 1.),
                ],
            ),
            (
                [0.0, 0.0, -1.0],
                [
                    corner(1., -1., -1.),
                    corner(-1., -1., -1.),
                    corner(-1., 1., -1.),
                    corner(1., 1., -1.),
                ],
            ),
        ];
        for (n, quad) in faces {
            let mid = quad
                .iter()
                .fold([0.0; 3], |a, p| add(a, p.map(|v| v / 4.0)));
            if dot(n, sub(self.camera.eye, mid)) <= 0.0 {
                continue;
            }
            let shade = if lit { lit_color(color, n, mid) } else { color };
            self.poly(&quad, shade);
        }
    }

    fn poly(&mut self, points: &[V3], color: Color32) {
        let projected: Option<Vec<(Pos2, f32)>> =
            points.iter().map(|&p| self.camera.project(p)).collect();
        let Some(projected) = projected else { return };
        let depth = projected.iter().map(|p| p.1).sum::<f32>() / projected.len() as f32;
        let color = fogged(color, depth);
        self.items.push((
            depth,
            Item::Poly {
                points: projected.iter().map(|p| p.0).collect(),
                color,
            },
        ));
    }

    fn picture(&mut self, texture: TextureId, corners: [V3; 4]) {
        let projected: Option<Vec<(Pos2, f32)>> =
            corners.iter().map(|&p| self.camera.project(p)).collect();
        let Some(projected) = projected else { return };
        let depth = projected.iter().map(|p| p.1).sum::<f32>() / 4.0;
        self.items.push((
            depth - 0.01,
            Item::Picture {
                points: [
                    projected[0].0,
                    projected[1].0,
                    projected[2].0,
                    projected[3].0,
                ],
                texture,
                tint: fogged(Color32::WHITE, depth),
            },
        ));
    }

    fn line(&mut self, a: V3, b: V3, width: f32, color: Color32) {
        let (Some((pa, za)), Some((pb, zb))) = (self.camera.project(a), self.camera.project(b))
        else {
            return;
        };
        let depth = (za + zb) / 2.0;
        let px = (width * self.camera.scale / depth).max(0.8);
        self.items.push((
            depth,
            Item::Line {
                a: pa,
                b: pb,
                width: px,
                color: fogged(color, depth),
            },
        ));
    }

    fn dot(&mut self, p: V3, size: f32, color: Color32) {
        let Some((at, depth)) = self.camera.project(p) else {
            return;
        };
        let radius = (size * self.camera.scale / depth / 2.0).max(0.6);
        self.items.push((
            depth,
            Item::Dot {
                at,
                radius,
                color: fogged(color, depth),
            },
        ));
    }

    fn paint(&self, painter: &egui::Painter, mut items: Vec<(f32, Item)>) {
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, item) in items {
            match item {
                Item::Poly { points, color } => {
                    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
                }
                Item::Picture {
                    points,
                    texture,
                    tint,
                } => {
                    let mut mesh = Mesh::with_texture(texture);
                    let uv = [
                        Pos2::new(0.0, 0.0),
                        Pos2::new(1.0, 0.0),
                        Pos2::new(1.0, 1.0),
                        Pos2::new(0.0, 1.0),
                    ];
                    for (p, uv) in points.iter().zip(uv) {
                        mesh.vertices.push(Vertex {
                            pos: *p,
                            uv,
                            color: tint,
                        });
                    }
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    painter.add(Shape::mesh(mesh));
                }
                Item::Line { a, b, width, color } => {
                    painter.line_segment([a, b], Stroke::new(width, color));
                }
                Item::Dot { at, radius, color } => {
                    painter.circle_filled(at, radius, color);
                }
            }
        }
    }
}

/// A hoop at `z`, its board facing the middle (`turned` for the near end):
/// the post and arm in steel, the glass, the orange rim and the net.
fn hoop(scene: &mut Scene, z: f32, turned: bool, steel: Color32) {
    let s = if turned { -1.0 } else { 1.0 };
    let at = |p: V3| [s * p[0], p[1], z + s * p[2]];
    scene.cube(at([0.0, 1.7, -1.2]), [0.23, 3.4, 0.25], steel, true);
    scene.cube(at([0.0, 3.35, -0.6]), [0.16, 0.16, 1.3], steel, true);
    let glass = alpha_of(rgb(0xefdcff), 0.22);
    let (bw, bh) = (0.9, 0.53);
    let board = [
        at([-bw, 3.48 + bh, 0.0]),
        at([bw, 3.48 + bh, 0.0]),
        at([bw, 3.48 - bh, 0.0]),
        at([-bw, 3.48 - bh, 0.0]),
    ];
    scene.poly(&board, lit_color(glass, [0.0, 0.0, s], board[0]));
    let edge = rgb(0xd8c5ea);
    for k in 0..4 {
        scene.line(board[k], board[(k + 1) % 4], 0.02, edge);
    }
    let rim = rgb(0xff621e);
    let ring = |r: f32, y: f32, n: usize| -> Vec<V3> {
        (0..=n)
            .map(|k| {
                let t = k as f32 / n as f32 * std::f32::consts::TAU;
                at([r * t.cos(), y, 0.35 + r * t.sin()])
            })
            .collect()
    };
    let top = ring(0.24, 3.05, 24);
    for pair in top.windows(2) {
        scene.line(pair[0], pair[1], 0.04, rim);
    }
    let net = alpha_of(rgb(0xf4f1fa), 0.55);
    let (upper, lower) = (ring(0.23, 3.04, 14), ring(0.14, 2.6, 14));
    for (a, b) in upper.iter().zip(&lower) {
        scene.line(*a, *b, 0.01, net);
    }
    for pair in lower.windows(2) {
        scene.line(pair[0], pair[1], 0.01, net);
    }
}

#[derive(Clone, Copy)]
enum Shade {
    /// `MeshStandardMaterial`: lit and fogged.
    Lit,
    /// `MeshBasicMaterial`: fogged only.
    Basic,
}

/// A flat picture on the floor (`size` along x and z from `corner`), cut into
/// `cells` so its perspective reads true and each vertex gets its own light.
#[allow(clippy::too_many_arguments)]
fn textured_floor(
    painter: &egui::Painter,
    camera: &Camera,
    texture: TextureId,
    corner: V3,
    size: [f32; 2],
    cells: [usize; 2],
    shade: Shade,
    opacity: f32,
) {
    let mut mesh = Mesh::with_texture(texture);
    let [nx, nz] = cells;
    let mut index = vec![None; (nx + 1) * (nz + 1)];
    for j in 0..=nz {
        for i in 0..=nx {
            let (u, v) = (i as f32 / nx as f32, j as f32 / nz as f32);
            let p = [corner[0] + u * size[0], corner[1], corner[2] + v * size[1]];
            let Some((at, z)) = camera.project(p) else {
                continue;
            };
            let base = match shade {
                Shade::Lit => lit_color(Color32::WHITE, [0.0, 1.0, 0.0], p),
                Shade::Basic => Color32::WHITE,
            };
            let color = alpha_of(fogged(base, z), opacity);
            index[j * (nx + 1) + i] = Some(mesh.vertices.len() as u32);
            // The picture's top (v = 0) at the far end (-z), as the plane
            // turned flat lays it.
            mesh.vertices.push(Vertex {
                pos: at,
                uv: Pos2::new(u, v),
                color,
            });
        }
    }
    for j in 0..nz {
        for i in 0..nx {
            let k = |i: usize, j: usize| index[j * (nx + 1) + i];
            if let (Some(a), Some(b), Some(c), Some(d)) =
                (k(i, j), k(i + 1, j), k(i + 1, j + 1), k(i, j + 1))
            {
                mesh.add_triangle(a, b, c);
                mesh.add_triangle(a, c, d);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// A plain lit quad on the floor.
fn floor_quad(
    painter: &egui::Painter,
    camera: &Camera,
    corner: V3,
    size: [f32; 2],
    color: Color32,
) {
    let [x, y, z] = corner;
    let points = [
        [x, y, z],
        [x + size[0], y, z],
        [x + size[0], y, z + size[1]],
        [x, y, z + size[1]],
    ];
    let projected: Option<Vec<(Pos2, f32)>> = points.iter().map(|&p| camera.project(p)).collect();
    if let Some(projected) = projected {
        let depth = projected.iter().map(|p| p.1).sum::<f32>() / 4.0;
        painter.add(Shape::convex_polygon(
            projected.iter().map(|p| p.0).collect(),
            fogged(color, depth),
            Stroke::NONE,
        ));
    }
}

/// The site's lights on a surface of `color` facing `n` at `p`: the ambient
/// fill (0xa48acd, 0.55), the warm key from (5, 18, 8) (0xffe7c8, 2.4), the
/// violet point at (-10, 7, -5) (0x9b30ff, 380, reaching 55) and the lavender
/// one at (10, 6, 10) (0xd27bff, 220, reaching 42), tone-mapped (ACES, 0.95).
fn lit_color(color: Color32, n: V3, p: V3) -> Color32 {
    let mut light = linear(rgb(0xa48acd)).map(|c| c * 0.55);
    let key = norm([5.0, 18.0, 8.0]);
    let warm = linear(rgb(0xffe7c8));
    let lambert = dot(n, key).max(0.0) * 2.4;
    for k in 0..3 {
        light[k] += warm[k] * lambert;
    }
    for (at, c, power, reach) in [
        ([-10.0, 7.0, -5.0], rgb(0x9b30ff), 380.0, 55.0),
        ([10.0, 6.0, 10.0], rgb(0xd27bff), 220.0, 42.0),
    ] {
        let d = sub(at, p);
        let dist = dot(d, d).sqrt().max(0.01);
        let facing = dot(n, d.map(|v| v / dist)).max(0.0);
        let cutoff = (1.0 - (dist / reach).powi(4)).clamp(0.0, 1.0).powi(2);
        let strength = power / (dist * dist) * cutoff * facing;
        let lc = linear(c);
        for k in 0..3 {
            light[k] += lc[k] * strength;
        }
    }
    let albedo = linear(color);
    let out: [f32; 3] = std::array::from_fn(|k| aces(albedo[k] * light[k] * 0.95));
    Color32::from_rgba_unmultiplied(to_srgb(out[0]), to_srgb(out[1]), to_srgb(out[2]), color.a())
}

/// The fog's share at depth `z`, toward the near-black of the night.
fn fogged(c: Color32, z: f32) -> Color32 {
    let keep = Camera::clear(z);
    let fog = [0x07, 0x04, 0x0d];
    let mix = |v: u8, f: u8| (f32::from(v) * keep + f32::from(f) * (1.0 - keep)).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(c.r(), fog[0]),
        mix(c.g(), fog[1]),
        mix(c.b(), fog[2]),
        c.a(),
    )
}

/// ACES filmic (Narkowicz's fit).
fn aces(x: f32) -> f32 {
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

fn linear(c: Color32) -> [f32; 3] {
    [c.r(), c.g(), c.b()].map(|v| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn alpha_of(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (f32::from(c.a()) * a) as u8)
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    a.map(|v| v / l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_point_sits_a_fifth_right_of_the_middle_on_a_wide_window() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1040.0, 620.0));
        let camera = Camera::new(EYE, LOOK, screen);
        let (at, _) = camera.project(LOOK).unwrap();
        assert!((at.x - (520.0 + 208.0)).abs() < 0.01 && (at.y - 310.0).abs() < 0.01);
        // A narrow one keeps it in the middle.
        let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let (at, _) = Camera::new(EYE, LOOK, narrow).project(LOOK).unwrap();
        assert!((at.x - 400.0).abs() < 0.01);
    }

    #[test]
    fn the_court_picture_has_its_lines_and_paint() {
        let image = court_image(&mut Lcg(9));
        assert_eq!(image.size, [1024, 1912]);
        // The paint under the far basket, the centre line across the middle.
        let at = |x: usize, y: usize| image.pixels[y * 1024 + x];
        assert_eq!(at(512, 100), Color32::from_rgb(0x79, 0x2a, 0xc1));
        let mid = at(300, 956);
        assert!(mid.r() > 240 && mid.g() > 230, "{mid:?}");
    }

    #[test]
    fn the_generator_is_the_sites() {
        // seed 9: 9*1664525 + 1013904223.
        let mut r = Lcg(9);
        let want = 9 * 1_664_525 + 1_013_904_223_u64;
        assert!((r.next() - want as f32 / 4_294_967_296.0).abs() < 1e-7);
    }
}
