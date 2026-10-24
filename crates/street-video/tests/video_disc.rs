//! Checks of the movie decoder against a real USA disc image of the original game.
//! Skipped unless STREET_ISO points at the image.

use std::{fs::File, io::BufReader};

use street_assets::iso::Iso;
use street_video::{mpeg2::Decoder, nfx::Movie};

fn disc() -> Option<Iso<BufReader<File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// A decoded picture is a picture: neither flat nor noise. Neighbouring
/// pixels are close (mean step well under the spread of the values).
fn looks_like_a_picture(y: &[u8], width: usize) -> bool {
    let n = y.len() as f64;
    let mean = y.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let spread = (y
        .iter()
        .map(|&v| (f64::from(v) - mean).powi(2))
        .sum::<f64>()
        / n)
        .sqrt();
    let step = y
        .chunks(width)
        .flat_map(|r| r.windows(2))
        .map(|w| (f64::from(w[0]) - f64::from(w[1])).abs())
        .sum::<f64>()
        / n;
    spread > 8.0 && step < spread * 0.5
}

#[test]
fn the_boot_movies_decode_to_the_end() {
    let Some(mut iso) = disc() else { return };
    for (path, frames, sound) in [
        ("/MOVIES/XMT0017.NFX", 75, true),
        ("/MOVIES/XNUFX.NFX", 181, true),
        ("/MOVIES/XMT0016.NFX", 399, false),
        ("/MOVIES/XMT0000.NFX", 77, false),
        ("/MOVIES/XLP0005.NFX", 101, false),
        ("/MOVIES/FILMROOM/XMT0000.NFX", 210, true),
    ] {
        let movie = Movie::parse(&iso.read(path).unwrap()).unwrap();
        assert_eq!(movie.audio.is_some(), sound, "{path}");
        let mut d = Decoder::new(&movie.video).unwrap();
        let (mut n, mut pictures) = (0, 0);
        while let Some(f) = d.next_frame().unwrap() {
            assert_eq!((f.width, f.height), (d.width, d.height));
            if n % 25 == 12 {
                pictures += usize::from(looks_like_a_picture(&f.y, f.width));
            }
            n += 1;
        }
        assert_eq!(n, frames, "{path}");
        assert_eq!(
            street_video::mpeg2::pictures(&movie.video),
            frames,
            "{path}"
        );
        assert!(pictures > 0, "{path}: no frame looks like a picture");
    }
}
