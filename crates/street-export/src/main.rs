//! `street-export`: turns the player's own discs into the export folder the
//! MyBaller launcher's builder reads (see `docs/PLAYER_EDITOR.md` and the
//! crate documentation).

use std::path::PathBuf;
use std::process::ExitCode;

use street_export::export::{self, Options};
use street_export::hires;

const USAGE: &str = "\
usage:
  street-export characters --out <dir> [--vol2 <iso>] [--roster-db <dir>]
                           [--hires <dir>] [--v3 <disc>] [--v1 <disc>]
                           [--only <id,...>] [--all-anims] [--no-ui] [--threads <n>]
  street-export save-info <street_save.dat>
  street-export check-roster <roster.json> [--vol2 <iso>] [--roster-db <dir>]
                             [--save <street_save.dat>]

  --vol2       the original game's USA PS2 disc image. Default: $STREET_ISO.
  --roster-db  a roster mod (the folder, or its database folder), e.g. the
               Legendary Edition; its xdbplyr.adf/xdbteam.adf replace the disc's.
  --hires      a Dolphin HD texture pack folder (Textures/GNZE69).
  --v3, --v1   Street V3 / the original game GameCube discs (.rvz, .ciso, .iso):
               their characters on Vol. 2's skeleton (models/v3gc, models/v1gc),
               animation libraries (anims/v3gc, anims/v1gc), head shots, teams.
  --only       character ids, e.g. vol2:0,v3gc:mario,v1gc:shaquille_o_neal
               (default: every character of every disc given).
  --all-anims  also write every animation bank whole (anims/vol2/<bank>.glb)
               and V3's intro and judges banks.
  --no-ui      leave out ui/ (front-end art).
  --threads    worker threads (default: the machine's cores).

  check-roster prints a JSON report (version, legal, fingerprint, problems)
  and exits 0 when the roster may be played online, 2 when not, 1 on an error.
  --save       a street-rs save, for created:<n> characters.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("check-roster") {
        return check_roster(&args[1..]);
    }
    let result = match args.first().map(String::as_str) {
        Some("characters") => characters(&args[1..]),
        Some("save-info") => match args.get(1) {
            Some(path) => street_export::save_info::read(std::path::Path::new(path)).map(|v| {
                println!("{}", serde_json::to_string_pretty(&v).expect("JSON"));
            }),
            None => Err("save-info needs a save file".into()),
        },
        Some("-h" | "--help") | None => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("street-export: {e}");
            if e.contains("usage") || e.starts_with("unknown") || e.contains("needs") {
                eprint!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}

/// `check-roster`: the report on stdout; exit 0 legal, 2 illegal, 1 error.
fn check_roster(args: &[String]) -> ExitCode {
    let report = street_export::check::parse(args).and_then(|o| street_export::check::run(&o));
    match report {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report.to_json()).expect("JSON")
            );
            if report.legal() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(e) => {
            eprintln!("street-export: {e}");
            if e.contains("usage") {
                eprint!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}

fn characters(args: &[String]) -> Result<(), String> {
    let mut out = None;
    let mut vol2 = std::env::var_os("STREET_ISO").map(PathBuf::from);
    let mut opts = Options {
        out: PathBuf::new(),
        vol2: PathBuf::new(),
        roster_db: None,
        hires: None,
        v3: None,
        v1: None,
        only: Vec::new(),
        all_anims: false,
        no_ui: false,
        threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value (see usage)"))
        };
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value()?)),
            "--vol2" => vol2 = Some(PathBuf::from(value()?)),
            "--roster-db" => opts.roster_db = Some(PathBuf::from(value()?)),
            "--hires" => opts.hires = Some(PathBuf::from(value()?)),
            "--v3" => opts.v3 = Some(PathBuf::from(value()?)),
            "--v1" => opts.v1 = Some(PathBuf::from(value()?)),
            "--only" => {
                opts.only = value()?
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "--threads" => {
                opts.threads = value()?
                    .parse()
                    .map_err(|_| "--threads needs a number (see usage)".to_string())?
            }
            "--all-anims" => opts.all_anims = true,
            "--no-ui" => opts.no_ui = true,
            other => return Err(format!("unknown option {other} (see usage)")),
        }
    }
    opts.out = out.ok_or("characters needs --out <dir> (see usage)")?;
    opts.vol2 = vol2.ok_or("characters needs --vol2 <iso> or $STREET_ISO (see usage)")?;
    let pack: Box<dyn hires::Hires> = match &opts.hires {
        Some(dir) => Box::new(hires::Pack::open(dir)?),
        None => Box::new(hires::None_),
    };
    eprintln!(
        "street-export: {} from {} into {}",
        export::describe(&opts.only),
        opts.vol2.display(),
        opts.out.display()
    );
    let report = export::characters(&opts, pack.as_ref()).map_err(|e| e.to_string())?;
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!(
        "street-export: {} characters, {} clips, {} files, {:.1} MB in {:.1} s",
        report.characters,
        report.clips,
        report.files,
        report.bytes as f64 / 1e6,
        report.seconds
    );
    Ok(())
}
