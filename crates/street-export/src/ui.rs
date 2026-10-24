//! `ui/`: Vol. 2's front-end art for the builder, so it can look like the
//! game's Create A Baller, and `ui/manifest.json` naming every piece.
//!
//! - backdrops (`XAFEBG`, 320x240, shown doubled on the 640x480 screen),
//! - the fonts (`XFONTSEN`) as white atlases with their metrics,
//! - the widget atlases (`XACOMMON`: star bullets, tabs, rating bars,
//!   sliders, button glyphs, the controller) with their sprite frames, and crops of the pieces
//!   the menus use, the HUD atlas, the 1P..4P badges, the Rewards pictures
//!   (`XAFESCRN`),
//! - team logos (`XATEAMS`),
//! - the screens' layouts (`XLAY_FE`) and the front end's strings
//!   (`XLOC_FE`),
//! - Create A Baller swatches: hair, facial hair, shoes, tops and bottoms
//!   from the create-a-player kit (`/XF/XF000001`), and the accessories'
//!   images (`/XF/XF000002`).
//!
//! Menu sounds (`/SFX/FE_EVT.AEM`) are AEMS scripts over a bank, not plain
//! samples, and are left out.

use std::io::{Read, Seek};

use serde_json::{Map, Value, json};
use street_assets::{big, db, fe, font, iso::Iso, locale, ssh};

use crate::out::Out;

/// Backdrops by the screens that use them (`street-game`'s `art::backdrop`,
/// from each screen constructor's call to `FUN_002d7720`).
const BACKDROPS: [(&str, usize); 14] = [
    ("createBaller", 3),
    ("main", 18),
    ("title", 17),
    ("choosePlayer", 5),
    ("chooseCourt", 4),
    ("rules", 25),
    ("options", 28),
    ("region", 6),
    ("story", 2),
    ("map", 19),
    ("rewards", 21),
    ("createTeam", 9),
    ("controls", 10),
    ("credits", 31),
];

/// Fonts by role (`street-game`'s `look::fonts`).
const FONTS: [(&str, usize); 7] = [
    ("small", 0),
    ("listSmall", 1),
    ("list", 2),
    ("entry", 4),
    ("main", 4),
    ("titleSmall", 5),
    ("title", 6),
];

/// Screen classes whose layouts the builder wants by name.
const LAYOUTS: [(&str, u32); 6] = [
    ("createBaller", 0x3d),
    ("mainMenu", 3),
    ("playIt", 5),
    ("choosePlayer", 0x1d),
    ("options", 0x28),
    ("createTeam", 0x35),
];

const PARTS: &str = "/XF/XF000001";
const PLAYERS: &str = "/XF/XF000002";

fn texture_json(t: &ssh::Texture, file: &str) -> Value {
    json!({ "file": file, "width": t.width, "height": t.height })
}

fn crop(t: &ssh::Texture, x: u32, y: u32, w: u32, h: u32) -> Option<ssh::Texture> {
    if w == 0 || h == 0 || x + w > t.width || y + h > t.height {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * t.width + x) * 4) as usize;
        rgba.extend_from_slice(&t.rgba[at..at + (w * 4) as usize]);
    }
    Some(ssh::Texture {
        name: t.name.clone(),
        width: w,
        height: h,
        rgba,
        indices: None,
    })
}

fn frame_crop(sheet: &ssh::Sheet, texture: usize, frame: usize) -> Option<ssh::Texture> {
    let t = sheet.textures.get(texture)?;
    let [x, y, w, h] = sheet.frames.get(texture)?.get(frame)?.bounds();
    crop(
        t,
        u32::try_from(x).ok()?,
        u32::try_from(y).ok()?,
        u32::try_from(w).ok()?,
        u32::try_from(h).ok()?,
    )
}

/// Every texture of `sheets` as `<dir>/<sheet>_<texture>.png`, with frames.
fn atlases(out: &Out, dir: &str, sheets: &[ssh::Sheet]) -> std::io::Result<Vec<Value>> {
    let mut list = Vec::new();
    for (s, sheet) in sheets.iter().enumerate() {
        for (t, tex) in sheet.textures.iter().enumerate() {
            let file = format!("{dir}/{s}_{t}.png");
            out.png(&file, tex)?;
            let mut v = texture_json(tex, &file);
            v["sheet"] = s.into();
            v["texture"] = t.into();
            v["name"] = tex.name.clone().into();
            if let Some(frames) = sheet.frames.get(t).filter(|f| !f.is_empty()) {
                v["frames"] = frames
                    .iter()
                    .map(|f| json!({ "rect": f.bounds(), "corners": f.corners }))
                    .collect::<Vec<_>>()
                    .into();
            }
            list.push(v);
        }
    }
    Ok(list)
}

/// Write `ui/` and return the manifest (also written as `ui/manifest.json`).
pub fn export<R: Read + Seek>(
    iso: &mut Iso<R>,
    out: &Out,
    players: &[db::Player],
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut named = Map::new();
    let mut manifest = Map::new();
    manifest.insert("version".into(), 1.into());
    manifest.insert("screen".into(), json!({ "width": 640, "height": 480 }));

    // Backdrops.
    let backdrops = fe::backdrops(&fe::read(iso, "/FE/XAFEBG.BIN")?);
    let mut list = Vec::new();
    for (n, t) in backdrops.iter().enumerate() {
        let file = format!("ui/backdrops/{n}.png");
        out.png(&file, t)?;
        let mut v = texture_json(t, &file);
        v["display"] = json!([640, 480]);
        list.push(v);
    }
    for (name, n) in BACKDROPS {
        if let Some(v) = list.get(n) {
            named.insert(format!("backdrop.{name}"), v.clone());
        }
    }
    manifest.insert("backdrops".into(), list.into());

    // Fonts: coverage as white with alpha, and the metrics.
    let fonts = font::fonts(iso)?;
    let mut list = Vec::new();
    for (i, f) in fonts.iter().enumerate() {
        let rgba: Vec<u8> = f
            .coverage
            .iter()
            .flat_map(|&a| [255, 255, 255, a])
            .collect();
        let file = format!("ui/fonts/{i}.png");
        out.png(
            &file,
            &ssh::Texture {
                name: format!("font{i}"),
                width: f.width,
                height: f.height,
                rgba,
                indices: None,
            },
        )?;
        let glyphs: Map<String, Value> = f
            .glyphs
            .iter()
            .map(|(c, g)| {
                (
                    c.to_string(),
                    json!({
                        "x": g.x, "y": g.y, "width": g.width, "height": g.height,
                        "advance": g.advance, "xOffset": g.x_offset, "top": g.top,
                    }),
                )
            })
            .collect();
        let mut kerning: Vec<(char, char, i8)> =
            f.kerning.iter().map(|(&(a, b), &k)| (a, b, k)).collect();
        kerning.sort();
        let metrics = json!({
            "atlas": file,
            "width": f.width,
            "height": f.height,
            "lineHeight": f.line_height,
            "glyphs": glyphs,
            "kerning": kerning
                .iter()
                .map(|(a, b, k)| json!([a.to_string(), b.to_string(), k]))
                .collect::<Vec<_>>(),
        });
        let json_file = format!("ui/fonts/{i}.json");
        out.json(&json_file, &metrics)?;
        list.push(json!({ "atlas": file, "metrics": json_file, "lineHeight": f.line_height }));
    }
    for (name, i) in FONTS {
        if let Some(v) = list.get(i) {
            named.insert(format!("font.{name}"), v.clone());
        }
    }
    manifest.insert("fonts".into(), list.into());

    // Widgets.
    let common = fe::read(iso, "/FE/XACOMMON.BIN")?;
    manifest.insert("common".into(), atlases(out, "ui/common", &common)?.into());
    if let Some(sheet) = common.first() {
        for (name, texture, frame) in [
            ("star", 5, 0),
            ("starLit", 5, 2),
            ("tab", 2, 0),
            ("barEnd", 6, 6),
            ("barMiddle", 6, 7),
        ] {
            if let Some(t) = frame_crop(sheet, texture, frame) {
                let file = format!("ui/widgets/{name}.png");
                out.png(&file, &t)?;
                named.insert(format!("widget.{name}"), texture_json(&t, &file));
            }
        }
    }
    if let Some(sheet) = common.get(1) {
        // The pad's button glyphs (with their frames in `common`) and the
        // controller picture.
        for (name, t) in ["buttons", "controller"].into_iter().zip(&sheet.textures) {
            let file = format!("ui/widgets/{name}.png");
            out.png(&file, t)?;
            named.insert(format!("widget.{name}"), texture_json(t, &file));
        }
    }
    if let Some(t) = common.get(2).and_then(|s| s.textures.first()) {
        for (name, (x, y, w, h)) in [
            ("sliderTrack", (4, 45, 175, 19)),
            ("sliderLeft", (8, 8, 8, 10)),
            ("sliderRight", (8, 21, 8, 10)),
        ] {
            if let Some(c) = crop(t, x, y, w, h) {
                let file = format!("ui/widgets/{name}.png");
                out.png(&file, &c)?;
                named.insert(format!("widget.{name}"), texture_json(&c, &file));
            }
        }
    }
    let screens = fe::read(iso, "/FE/XAFESCRN.BIN")?;
    manifest.insert(
        "screens".into(),
        atlases(out, "ui/screens", &screens)?.into(),
    );
    for (name, path) in [("hud", "/FE/HUD.SSH"), ("badges", "/FE/OFFIND.SSH")] {
        if let Ok(sheet) = fe::sheet(iso, path) {
            let list = atlases(out, &format!("ui/{name}"), std::slice::from_ref(&sheet))?;
            manifest.insert(name.into(), list.into());
        }
    }

    // Team logos: the 64x64 tile of each entry.
    let logos = fe::read(iso, "/FE/XATEAMS.BIN")?;
    let mut list = Vec::new();
    for (n, s) in logos.iter().enumerate() {
        let Some(t) = s
            .textures
            .iter()
            .find(|t| t.width == 64 && t.height == 64)
            .or_else(|| s.textures.first())
        else {
            continue;
        };
        let file = format!("ui/logos/{n}.png");
        out.png(&file, t)?;
        list.push(texture_json(t, &file));
    }
    manifest.insert("logos".into(), list.into());

    // Layouts and strings.
    let texts = locale::read(iso, "/LOCALE/XLOC_FE.EUS")?;
    let text_of = |id: u16| -> Option<&str> {
        let file = locale::FE_TEXT_FILES.get(usize::from(id >> 8))?;
        texts
            .iter()
            .find(|f| f.name == *file)?
            .texts
            .get(usize::from(id & 0xff))
            .map(String::as_str)
    };
    let layouts = font::layouts(iso, "/FE/XLAY_FE.BIN")?;
    let layouts: Vec<Value> = layouts
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let elements: Vec<Value> = l
                .elements
                .iter()
                .map(|e| {
                    let mut v = json!({ "kind": e.kind, "id": e.id, "align": e.align });
                    if let Some((x, y)) = e.position() {
                        v["position"] = json!([x, y]);
                    }
                    if let Some((w, h)) = e.size() {
                        v["size"] = json!([w, h]);
                    }
                    if let Some(c) = e.colour() {
                        v["colour"] = json!(c);
                    }
                    let texts: Vec<Value> = e
                        .texts()
                        .into_iter()
                        .map(|id| json!({ "id": id, "text": text_of(id) }))
                        .collect();
                    if !texts.is_empty() {
                        v["texts"] = texts.into();
                    }
                    v["props"] = e
                        .props
                        .iter()
                        .map(|(k, v)| (k.to_string(), Value::from(*v)))
                        .collect::<Map<_, _>>()
                        .into();
                    v
                })
                .collect();
            json!({ "index": i, "class": l.class, "elements": elements })
        })
        .collect();
    for (name, class) in LAYOUTS {
        if let Some(i) = layouts.iter().position(|l| l["class"] == class) {
            named.insert(
                format!("layout.{name}"),
                json!({ "file": "ui/layouts.json", "index": i, "class": class }),
            );
        }
    }
    out.json(
        "ui/layouts.json",
        &json!({ "textFiles": locale::FE_TEXT_FILES, "layouts": layouts }),
    )?;
    let strings: Map<String, Value> = texts
        .iter()
        .map(|f| (f.name.clone(), json!(f.texts)))
        .collect();
    out.json("ui/text.json", &Value::Object(strings))?;
    named.insert("text.frontEnd".into(), json!({ "file": "ui/text.json" }));

    // Create A Baller swatches.
    let kit = kit(iso, out, players)?;
    manifest.insert("kit".into(), kit);
    let acc = accessories(iso, out)?;
    manifest.insert("accessories".into(), acc);

    manifest.insert("named".into(), Value::Object(named));
    let manifest = Value::Object(manifest);
    out.json("ui/manifest.json", &manifest)?;
    Ok(manifest)
}

/// Sheets of an archive by entry, decoded on demand.
struct Archive {
    path: &'static str,
    entries: Vec<big::BigEntry>,
}

impl Archive {
    fn open<R: Read + Seek>(iso: &mut Iso<R>, path: &'static str) -> street_assets::Result<Self> {
        Ok(Self {
            path,
            entries: big::read_index(iso, path)?,
        })
    }

    fn sheet<R: Read + Seek>(&self, iso: &mut Iso<R>, i: usize) -> Option<ssh::Sheet> {
        let e = self.entries.get(i)?;
        ssh::parse(&big::read_entry(iso, self.path, e).ok()?).ok()
    }
}

/// Hair, facial hair, shoes, tops and bottoms, found the way the game finds
/// them (`db::Player::created_model` on a created record with each choice).
fn kit<R: Read + Seek>(
    iso: &mut Iso<R>,
    out: &Out,
    players: &[db::Player],
) -> Result<Value, Box<dyn std::error::Error>> {
    let Some(template) = players
        .iter()
        .find(|p| p.team == db::CREATED_PLAYER_TEAM)
        .cloned()
    else {
        return Ok(Value::Null);
    };
    let parts = Archive::open(iso, PARTS)?;
    let mut write = |file: String, sheet: Option<usize>, slot: &str| -> Option<Value> {
        let s = parts.sheet(iso, sheet?)?;
        let t = s
            .textures
            .iter()
            .find(|t| t.name == slot)
            .or_else(|| s.textures.first())?;
        out.png(&file, t).ok()?;
        Some(texture_json(t, &file))
    };
    let with = |build: i16, female: bool, f: &dyn Fn(&mut db::Player)| {
        let mut p = template.clone();
        p.build = build;
        p.female = female;
        p.look = 0;
        p.hair_style = 0;
        p.hair_color = 0;
        p.facial_hair_style = -1;
        p.facial_hair_color = 0;
        p.shoes = 0;
        p.clothing = 0;
        f(&mut p);
        p.created_model(None)
    };
    let mut kit = Map::new();
    for (gender, build, female) in [("male", 0i16, false), ("female", 4, true)] {
        let (_, hair_styles, beards) = db::kit_limits(build as usize);
        let mut hair = Vec::new();
        for h in 0..hair_styles {
            let mut colours = Vec::new();
            for c in 0..db::HAIR_COLORS {
                let m = with(build, female, &|p| {
                    p.hair_style = h as i16;
                    p.hair_color = c as i16;
                });
                colours.push(
                    write(
                        format!("ui/kit/hair/{gender}/{h}_{c}.png"),
                        m.and_then(|m| m.hair_sheet),
                        "hair",
                    )
                    .unwrap_or(Value::Null),
                );
            }
            hair.push(Value::from(colours));
        }
        kit.insert(format!("hair.{gender}"), hair.into());
        if beards > 0 {
            let mut list = Vec::new();
            for b in 0..beards {
                let mut colours = Vec::new();
                for c in 0..db::FACIAL_HAIR_COLORS {
                    let m = with(build, female, &|p| {
                        p.facial_hair_style = b as i16;
                        p.facial_hair_color = c as i16;
                    });
                    colours.push(
                        write(
                            format!("ui/kit/facialHair/{b}_{c}.png"),
                            m.and_then(|m| m.beard_sheet),
                            "berd",
                        )
                        .unwrap_or(Value::Null),
                    );
                }
                list.push(Value::from(colours));
            }
            kit.insert("facialHair".into(), list.into());
        }
        let mut tops = Vec::new();
        let mut bottoms = Vec::new();
        for c in 0..11i16 {
            let mut top = Vec::new();
            let mut bottom = Vec::new();
            for v in 0..3i16 {
                let m = with(build, female, &|p| p.clothing = c | (v << 14));
                top.push(
                    write(
                        format!("ui/kit/tops/{gender}/{c}_{v}.png"),
                        m.and_then(|m| m.top_sheet),
                        "shir",
                    )
                    .unwrap_or(Value::Null),
                );
                bottom.push(
                    write(
                        format!("ui/kit/bottoms/{gender}/{c}_{v}.png"),
                        m.map(|m| m.bottom_sheet),
                        "shor",
                    )
                    .unwrap_or(Value::Null),
                );
            }
            tops.push(Value::from(top));
            bottoms.push(Value::from(bottom));
        }
        kit.insert(format!("tops.{gender}"), tops.into());
        kit.insert(format!("bottoms.{gender}"), bottoms.into());
    }
    let mut shoes = Vec::new();
    for s in 0..db::SHOE_STYLES {
        let m = with(0, false, &|p| p.shoes = s as i16);
        shoes.push(
            write(
                format!("ui/kit/shoes/{s}.png"),
                m.map(|m| m.shoe_sheet),
                "shoe",
            )
            .unwrap_or(Value::Null),
        );
    }
    kit.insert("shoes".into(), shoes.into());
    Ok(Value::Object(kit))
}

/// Base name of an accessory slot pair (`elpdL` -> `elpd`).
pub fn accessory_base(name: &str) -> &str {
    name.strip_suffix('L')
        .or_else(|| name.strip_suffix('R'))
        .unwrap_or(name)
}

/// Each accessory slot's images (`ui/acc/<name>/<image>.png`).
fn accessories<R: Read + Seek>(
    iso: &mut Iso<R>,
    out: &Out,
) -> Result<Value, Box<dyn std::error::Error>> {
    let players = Archive::open(iso, PLAYERS)?;
    let mut done = std::collections::BTreeMap::new();
    for slot in db::ACCESSORY_SLOTS.iter() {
        let Some(sheet) = slot.sheet else { continue };
        let base = accessory_base(slot.name);
        if done.contains_key(base) {
            continue;
        }
        let Some(s) = players.sheet(iso, sheet) else {
            continue;
        };
        let mut list = Vec::new();
        for (i, t) in s.textures.iter().enumerate() {
            let file = format!("ui/acc/{base}/{i}.png");
            out.png(&file, t)?;
            list.push(texture_json(t, &file));
        }
        done.insert(base.to_owned(), Value::from(list));
    }
    Ok(Value::Object(done.into_iter().collect()))
}

/// The number of images each accessory slot offers, for the catalog.
pub fn accessory_choices(manifest: &Value, base: &str) -> usize {
    manifest["accessories"][base].as_array().map_or(0, Vec::len)
}
