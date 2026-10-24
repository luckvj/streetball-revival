//! Print the game database tables from the disc.
//!
//!     cargo run -p street-assets --example db_report -- <iso> [TABLE]
//!
//! `TABLE` is `players` (default), `teams`, `courts`, `moves`, `nicknames`,
//! `cards` (raw fields; not decoded) or a player record number for all of
//! that player's fields and model entries. With `STREET_ROSTER_DB` set to a
//! roster mod's folder, its players and teams stand in for the disc's.

use street_assets::{
    db::{self, PlayerModel, RATING_NAMES, Table},
    iso::Iso,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: db_report <iso> [TABLE]")?)?;
    let table = args.next().unwrap_or_else(|| "players".into());
    if let Some(dir) = std::env::var_os("STREET_ROSTER_DB") {
        let dir = db::find_roster_dir(dir.as_ref()).ok_or("no roster in STREET_ROSTER_DB")?;
        db::set_roster_override(Some(&dir))?;
    }
    match table.as_str() {
        "players" => {
            let teams = db::teams(&mut iso)?;
            println!(
                "rec name                     team                      #  pos ht  wt  kind       model"
            );
            for p in db::players(&mut iso)? {
                let team = usize::try_from(p.team)
                    .ok()
                    .and_then(|t| teams.get(t))
                    .map_or("-".into(), |t| t.full_name());
                let jersey = p.jersey().map_or("-".into(), |j| j.to_string());
                let pos = p.position().map_or("-", |q| q.abbrev());
                println!(
                    "{:3} {:24} {:24} {:>2} {:3} {:2}  {:3} {:10} {}",
                    p.index,
                    p.name(),
                    team,
                    jersey,
                    pos,
                    p.height_in,
                    p.weight_lb,
                    format!("{:?}", p.kind()),
                    model_summary(&p.model())
                );
            }
        }
        "teams" => {
            for t in db::teams(&mut iso)? {
                println!(
                    "{:2} {:20} {:14} {:14} logo {:2} roster {:?}",
                    t.index, t.name, t.city, t.nickname, t.logo, t.roster
                );
            }
        }
        "courts" => {
            for c in db::courts(&mut iso)? {
                println!(
                    "{:2} {:7} {:18} {:22} unknown {} tiers {:.2?}",
                    c.index, c.id, c.name, c.location, c.unknown, c.tier_mix
                );
            }
        }
        "moveraw" => {
            let t = db::Table::read(&mut iso, db::MOVES)?;
            println!(
                "kinds: {:?}",
                (0..t.field_count()).map(|f| t.kind(f)).collect::<Vec<_>>()
            );
            for r in 0..t.record_count() {
                let f: Vec<String> = (0..t.field_count())
                    .map(|f| match t.kind(f) {
                        Some(db::FieldKind::String) => {
                            format!("{:?}", t.string(r, f).ok().flatten())
                        }
                        _ => format!("{}", t.raw(r, f).unwrap_or(-1)),
                    })
                    .collect();
                println!("{r:3}: {}", f.join(" | "));
            }
        }
        "moves" => {
            for m in db::moves(&mut iso)? {
                println!(
                    "{:2} {:20} kind {} level {} cost {:3} id {}",
                    m.index, m.name, m.kind, m.level, m.cost, m.move_id
                );
            }
        }
        "nicknames" => {
            for n in db::nicknames(&mut iso)? {
                println!("{:2} {:16} {:?}", n.id, n.name, n.skill);
            }
        }
        "cards" => {
            let t = Table::read(&mut iso, db::CARDS)?;
            for i in 0..t.record_count() {
                println!("{i:3} {:?}", t.record(i)?);
            }
        }
        n => {
            let index: usize = n.parse().map_err(|_| format!("unknown table {n}"))?;
            let players = db::players(&mut iso)?;
            let p = players.get(index).ok_or("no such player")?;
            println!(
                "{} ({:?}), team {}, code {}",
                p.name(),
                p.kind(),
                p.team,
                p.code
            );
            for (name, r) in RATING_NAMES.iter().zip(p.ratings) {
                print!("{name} {r}  ");
            }
            println!();
            println!(
                "build {} look {} clothing {:#06x} shoes {} hair {}/{} facial hair {}/{} scale {:?}",
                p.build,
                p.look,
                p.clothing as u16,
                p.shoes,
                p.hair_style,
                p.hair_color,
                p.facial_hair_style,
                p.facial_hair_color,
                p.body_scale
            );
            println!("model {:#?}", p.model());
            println!("fields {:?}", p.fields);
        }
    }
    Ok(())
}

fn model_summary(m: &Option<PlayerModel>) -> String {
    match m {
        None => "-".into(),
        Some(PlayerModel::League(n)) => {
            let mut s = format!(
                "XF2 body {} morph {} sheets {}/{}",
                n.bodies[0], n.morphs[0], n.road_sheet, n.home_sheet
            );
            if n.dreads {
                s += " dreads";
            }
            for a in &n.accessories {
                s += &format!(" {}:{}", a.name, a.image);
            }
            s
        }
        Some(PlayerModel::Unique(u)) => {
            format!(
                "XF2 model {} sheets {}/{}",
                u.model, u.road_sheet, u.home_sheet
            )
        }
        Some(PlayerModel::Created(c)) => format!(
            "XF1 body {} head {:?} skin {} hair {:?}",
            c.body, c.head, c.skin_sheet, c.hair_sheet
        ),
    }
}
