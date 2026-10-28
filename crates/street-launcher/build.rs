//! The launcher's build: the Windows icon resource (`assets/icon.ico`,
//! linked as a `.res`), and, when the release build names it in
//! `STREET_GAME_EMBED`, the game program carried inside the launcher for the
//! one-file download, deflated (about a third of its size). And the version
//! the launcher shows (`STREET_RELEASE`): the release's tag on a release build
//! (GitHub names the pushed tag in `GITHUB_REF_NAME`), else the last tag the
//! checkout has, else the package's version.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo::rustc-check-cfg=cfg(embedded_game)");
    println!("cargo:rerun-if-env-changed=STREET_GAME_EMBED");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if let Some(game) = env::var_os("STREET_GAME_EMBED") {
        let game = PathBuf::from(game);
        println!("cargo:rerun-if-changed={}", game.display());
        let bytes = fs::read(&game).expect("STREET_GAME_EMBED names the built game");
        // FNV-1a, so each build unpacks to its own folder.
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        let packed = miniz_oxide::deflate::compress_to_vec(&bytes, 9);
        fs::write(out.join("street-game.deflate"), packed).unwrap();
        println!("cargo:rustc-env=STREET_GAME_HASH={hash:016x}");
        println!("cargo:rustc-env=STREET_GAME_LEN={}", bytes.len());
        println!("cargo:rustc-cfg=embedded_game");
    }
    let target = |k: &str| env::var(k).unwrap_or_default();
    println!("cargo:rerun-if-env-changed=GITHUB_REF_TYPE");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
    let release = Some(target("GITHUB_REF_NAME"))
        .filter(|name| target("GITHUB_REF_TYPE") == "tag" && !name.is_empty())
        .or_else(last_tag)
        .unwrap_or_else(|| format!("v{}", target("CARGO_PKG_VERSION")));
    println!("cargo:rustc-env=STREET_RELEASE={release}");
    if target("CARGO_CFG_TARGET_OS") == "windows" && target("CARGO_CFG_TARGET_ENV") == "msvc" {
        let res = out.join("icon.res");
        fs::write(&res, icon_res(&fs::read("assets/icon.ico").unwrap())).unwrap();
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}

/// The newest tag behind the checkout (`git describe --tags --abbrev=0`).
fn last_tag() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()?;
    let tag = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !tag.is_empty()).then_some(tag)
}

/// A `.res` holding the `.ico`'s images as icons 1.. and their group as
/// icon group 1, the one Explorer shows.
fn icon_res(ico: &[u8]) -> Vec<u8> {
    let u32_at = |i: usize| u32::from_le_bytes(ico[i..i + 4].try_into().unwrap());
    let count = u16::from_le_bytes([ico[4], ico[5]]);
    // The empty entry every `.res` opens with.
    let mut res = vec![0, 0, 0, 0, 32, 0, 0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0];
    res.extend([0; 16]);
    let mut group = vec![0, 0, 1, 0];
    group.extend(count.to_le_bytes());
    for id in 1..=count {
        let e = 6 + 16 * usize::from(id - 1);
        let (size, offset) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        entry(&mut res, 3, id, 0x1010, &ico[offset..offset + size]);
        group.extend(&ico[e..e + 4]);
        group.extend(1u16.to_le_bytes());
        group.extend(&ico[e + 6..e + 12]);
        group.extend(id.to_le_bytes());
    }
    entry(&mut res, 14, 1, 0x1030, &group);
    res
}

fn entry(res: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32u32.to_le_bytes());
    res.extend([0xff, 0xff]);
    res.extend(kind.to_le_bytes());
    res.extend([0xff, 0xff]);
    res.extend(id.to_le_bytes());
    res.extend(0u32.to_le_bytes());
    res.extend(flags.to_le_bytes());
    res.extend(0x0409u16.to_le_bytes());
    res.extend([0; 8]);
    res.extend(data);
    while !res.len().is_multiple_of(4) {
        res.push(0);
    }
}
