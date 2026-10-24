//! The game's movies (`/MOVIES/*.NFX`, decoded by `street-video`): the
//! screens' transition movies ([`TransitionClip`], played by
//! [`crate::transition`]: the boot's, the title's, the DVD pages', the
//! credits' NuFX logo, the blurs) and the loops and film-room clips in the
//! menus' panels ([`FilmRoom`]).
//!
//! Each movie decodes on its own thread a few frames ahead; the picture
//! shows the frame its clock has reached, and the soundtrack (a SCHl
//! stream between the pictures) plays alongside.
//!
//! The film room ([`FilmRoom`]) plays a move's clip in a panel of the
//! menus, looping and silent: `/MOVIES/FILMROOM/XMT00nn.NFX`, `nn` the
//! move's id (`XDBMOVE` field 5), printed as `xmt000%d` or `xmt00%d`
//! (`0x1d6a30`), and `TEMP.NFX` for none. The screens' loops
//! ([`crate::menu::loop_movie_for_page`]) play through it too.

use std::{
    sync::{
        Mutex,
        mpsc::{Receiver, TryRecvError, TrySendError, sync_channel},
    },
    thread,
};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use street_audio::{Clip, schl};
use street_video::{mpeg2::Decoder, nfx::Movie};

use crate::{Screen, setup::Disc};

/// Inside Vol. 2's movies, as its three screens' keys name them
/// (`FUN_00289f98`, `FUN_0028aee0`, `FUN_00289100`, into `DAT_003fdf44`; the
/// PAL builds play `xmtplnn` instead), each played as its page's transition
/// into itself: the main page's Vol. 2 Trailer (the intro) and SSX 3
/// Preview (an SSX Tricky trailer); What's New In 2's New Game, Take On The
/// league, Be A Legend and Vol. 2 Rewards (Play All, "xmtall", plays the four,
/// `FUN_002ddb40`); Bobbito Garcia's New York City and Vol. 2.
pub const TRAILER: &[&str] = &["xmt0018.nfx"];
pub const SSX: &[&str] = &["xmt0023.nfx"];
pub const WHATS_NEW: &[&str] = &["xmt0024.nfx", "xmt0025.nfx", "xmt0026.nfx", "xmt0027.nfx"];
pub const BOBBITO: [&[&str]; 2] = [&["xmt0022.nfx"], &["xmt0021.nfx"]];

/// A movie playing.
struct Playing {
    /// Frames from the decoding thread (a mutex only to be `Sync`).
    frames: Mutex<Receiver<(usize, Vec<u8>)>>,
    image: Handle<Image>,
    rate: f32,
    started: f64,
    /// The next frame, decoded but not yet due.
    ahead: Option<(usize, Vec<u8>)>,
    /// The movie's frames: its clock ends it at the last one's time, as
    /// the original's player's does, even if decoding lags behind.
    frames_total: usize,
    ended: bool,
}

/// The film room: a move's clip in a menu panel. Set `want` to the clip to
/// show (or `None`); the picture is [`FilmRoom::image`].
#[derive(Resource, Default)]
pub struct FilmRoom {
    pub want: Option<String>,
    playing: Option<(String, Playing)>,
}

impl FilmRoom {
    /// The clip's picture, while one plays.
    pub fn image(&self) -> Option<Handle<Image>> {
        self.playing.as_ref().map(|(_, p)| p.image.clone())
    }

    /// A move's clip.
    pub fn path(move_id: i16) -> String {
        if move_id < 0 {
            "/MOVIES/FILMROOM/TEMP.NFX".into()
        } else {
            format!("/MOVIES/FILMROOM/XMT00{move_id:02}.NFX")
        }
    }

    /// Plays the wanted clip again from its first frame, in a picture of
    /// its own: a screen's loop made anew by its transition
    /// (`FUN_002ddb40` stops the running loop unless the new screen answers
    /// "sameloop", and starts the new one as the transition ends).
    pub fn restart(&mut self) {
        self.playing = None;
    }
}

pub struct MoviePlugin;

impl Plugin for MoviePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FilmRoom>()
            .add_systems(
                Update,
                film_room
                    .run_if(in_state(Screen::Menu))
                    .run_if(crate::transition::loop_runs),
            )
            .add_systems(OnExit(Screen::Menu), |mut film: ResMut<FilmRoom>| {
                *film = FilmRoom::default();
            });
    }
}

/// The soundtrack, whole: its SCHl streams one after another.
fn soundtrack(data: &[u8]) -> Option<Clip> {
    let streams = schl::streams(data).ok()?;
    let first = streams.first()?.patch.clone();
    let mut samples = Vec::new();
    for s in streams
        .iter()
        .filter(|s| s.patch.channels == first.channels && s.patch.rate == first.rate)
    {
        samples.extend(s.decode(data).samples);
    }
    Some(Clip {
        rate: first.rate,
        channels: first.channels,
        samples,
        looping: None,
    })
}

/// Opaque black: the full-screen movies' picture before their first frame.
const BLACK: [u8; 4] = [0, 0, 0, 255];
/// Clear: a menu panel's picture before its first frame, so the backdrop
/// under it shows (the city loop repaints part of the backdrop).
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// Where a movie's frames go.
enum Picture {
    /// A new image, filled with this colour until the first frame.
    Fresh([u8; 4]),
    /// The image of the run before, which keeps its last frame until the
    /// next run's first: a loop wraps without a blank frame.
    Reuse(Handle<Image>),
}

/// Opens the movie `path` (its bytes `data`): its picture and a thread
/// decoding it a few frames ahead; and its soundtrack, if it has one.
fn decode(
    path: &str,
    data: Vec<u8>,
    images: &mut Assets<Image>,
    now: f64,
    picture: Picture,
) -> Option<(Playing, Option<Clip>)> {
    let movie = Movie::parse(&data).map_err(|e| warn!("{path}: {e}")).ok()?;
    let (width, height, rate) = {
        let d = Decoder::new(&movie.video)
            .map_err(|e| warn!("{path}: {e}"))
            .ok()?;
        (d.width, d.height, d.rate)
    };
    let sound = movie.audio.as_deref().and_then(soundtrack);
    let frames_total = street_video::mpeg2::pictures(&movie.video);
    let (tx, rx) = sync_channel(6);
    let name = path.to_owned();
    thread::spawn(move || {
        let Ok(mut d) = Decoder::new(&movie.video) else {
            return;
        };
        let mut rgba = Vec::new();
        let mut n = 0;
        loop {
            match d.next_frame() {
                Ok(Some(f)) => {
                    f.to_rgba(&mut rgba);
                    let mut item = (n, std::mem::take(&mut rgba));
                    // Wait for room; stop if nobody is watching any more.
                    loop {
                        match tx.try_send(item) {
                            Ok(()) => break,
                            Err(TrySendError::Full(back)) => {
                                item = back;
                                thread::sleep(std::time::Duration::from_millis(4));
                            }
                            Err(TrySendError::Disconnected(_)) => return,
                        }
                    }
                    n += 1;
                }
                Ok(None) => return,
                Err(e) => {
                    eprintln!("{name}: {e}");
                    return;
                }
            }
        }
    });
    let image = match picture {
        Picture::Reuse(image) if images.contains(&image) => image,
        Picture::Reuse(_) => blank(images, width as u32, height as u32, CLEAR),
        Picture::Fresh(fill) => blank(images, width as u32, height as u32, fill),
    };
    let playing = Playing {
        frames: Mutex::new(rx),
        image,
        rate,
        started: now,
        ahead: None,
        frames_total,
        ended: false,
    };
    Some((playing, sound))
}

/// A `width` x `height` picture of one colour.
fn blank(images: &mut Assets<Image>, width: u32, height: u32, fill: [u8; 4]) -> Handle<Image> {
    images.add(Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &fill,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ))
}

/// A screen transition's movie ([`crate::transition`]): decoding from when
/// it opens, its clock from [`TransitionClip::start`]. The blurs carry no
/// sound; the boot's movies, the title's intro, the DVD pages' and NuFX's
/// logo carry their soundtracks, which play from their first frames.
pub struct TransitionClip {
    playing: Playing,
    shown: bool,
    sound: Option<Clip>,
}

impl TransitionClip {
    /// Opens `/MOVIES/<name>` (its file name as the original's tables give
    /// it, `xmtblur.nfx`) and starts decoding its first frames.
    pub fn open(name: &str, disc: &mut Disc, images: &mut Assets<Image>) -> Option<Self> {
        let path = format!("/MOVIES/{}", name.to_ascii_uppercase());
        let data = disc
            .iso
            .read(&path)
            .map_err(|e| warn!("{path}: {e}"))
            .ok()?;
        let (playing, sound) = decode(&path, data, images, 0.0, Picture::Fresh(BLACK))?;
        Some(Self {
            playing,
            shown: false,
            sound,
        })
    }

    /// The soundtrack, once: to start with the first frame.
    pub fn take_sound(&mut self) -> Option<Clip> {
        self.sound.take()
    }

    /// Starts the movie's clock: its first frame is due now.
    pub fn start(&mut self, now: f64) {
        self.playing.started = now;
    }

    /// Shows the frame due at `now`.
    pub fn step(&mut self, now: f64, images: &mut Assets<Image>) {
        self.shown |= step(&mut self.playing, now, images);
    }

    /// A frame is in the picture (it is black until then).
    pub fn shown(&self) -> bool {
        self.shown
    }

    /// The last frame has been shown and its time is over.
    pub fn ended(&self) -> bool {
        self.playing.ended
    }

    pub fn image(&self) -> Handle<Image> {
        self.playing.image.clone()
    }
}

/// Shows the frame the movie's clock has reached; marks it ended when the
/// decoder has nothing more. Whether a frame went into the picture.
fn step(p: &mut Playing, now: f64, images: &mut Assets<Image>) -> bool {
    let due = ((now - p.started) * f64::from(p.rate)) as usize;
    let mut show = None;
    let frames = p.frames.get_mut().expect("the movie's frame queue");
    loop {
        let f = match p.ahead.take() {
            Some(f) => f,
            None => match frames.try_recv() {
                Ok(f) => f,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    p.ended = true;
                    break;
                }
            },
        };
        if f.0 > due {
            p.ahead = Some(f);
            break;
        }
        show = Some(f.1);
    }
    // The last frame's time reached: over, on the clock.
    if p.frames_total > 0 && due + 1 >= p.frames_total {
        p.ended = true;
    }
    if let Some(rgba) = show
        && let Some(mut img) = images.get_mut(&p.image)
    {
        img.data = Some(rgba);
        return true;
    }
    false
}

/// The film room's clip: started when asked for, looped, stopped when not
/// wanted. The picture stays on the last frame while the next loop opens.
fn film_room(
    mut film: ResMut<FilmRoom>,
    mut disc: ResMut<Disc>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs_f64();
    let read = |path: &str| disc.iso.read(path).map_err(|e| warn!("{path}: {e}")).ok();
    run_film(&mut film, read, &mut images, now);
}

/// [`film_room`]'s work, with the disc read by `read`: (re)opens the wanted
/// clip when it changes or its run ends, then shows the frame due. A new
/// clip starts clear (the panel's backdrop shows until its first frame); a
/// clip that ends and runs again keeps its picture, so a loop never blinks.
fn run_film(
    film: &mut FilmRoom,
    mut read: impl FnMut(&str) -> Option<Vec<u8>>,
    images: &mut Assets<Image>,
    now: f64,
) {
    let same = film.playing.as_ref().map(|(path, _)| path) == film.want.as_ref();
    let ended = film.playing.as_ref().is_some_and(|(_, p)| p.ended);
    if !same || ended {
        let picture = match film.playing.take() {
            Some((_, p)) if same => Picture::Reuse(p.image),
            _ => Picture::Fresh(CLEAR),
        };
        film.playing = film.want.clone().and_then(|path| {
            let data = read(&path)?;
            decode(&path, data, images, now, picture).map(|(p, _)| (path, p))
        });
    }
    if let Some((_, p)) = &mut film.playing {
        step(p, now, images);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The city loop on the menus: it opens clear over the backdrop, and
    /// when a run ends the next one keeps drawing into the same picture
    /// (still holding the last frame), so the loop wraps without a blank;
    /// a restart (a transition making the screen's loop anew) opens it
    /// afresh.
    #[test]
    fn film_room_loop_wraps_into_the_same_picture() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let mut read = |p: &str| iso.read(p).ok();
        let mut images = Assets::<Image>::default();
        let mut film = FilmRoom {
            want: Some("/MOVIES/XLP0005.NFX".into()),
            ..default()
        };
        // A new clip's picture is clear until its first frame.
        let data = read("/MOVIES/XLP0005.NFX").unwrap();
        let (fresh, _) = decode("loop", data, &mut images, 0.0, Picture::Fresh(CLEAR)).unwrap();
        let picture = images.get(&fresh.image).unwrap();
        assert_eq!((picture.width(), picture.height()), (320, 240));
        assert!(picture.data.as_ref().unwrap().iter().all(|&b| b == 0));

        run_film(&mut film, &mut read, &mut images, 0.0);
        let first = film.image().expect("the loop opens");

        // The run's last frame on show, then the run ends: the next run
        // draws into the same picture, which is never cleared meanwhile.
        images.get_mut(&first).unwrap().data = Some(vec![7; 320 * 240 * 4]);
        film.playing.as_mut().unwrap().1.ended = true;
        run_film(&mut film, &mut read, &mut images, 10.0);
        assert_eq!(film.image().as_ref(), Some(&first));
        let started = film.playing.as_ref().unwrap().1.started;
        assert_eq!(started, 10.0, "the next run restarts its clock");
        // Either the held frame or the new run's first: opaque, not clear.
        assert_ne!(images.get(&first).unwrap().data.as_ref().unwrap()[3], 0);

        // Made anew: a picture of its own, its clock from the restart.
        film.restart();
        run_film(&mut film, &mut read, &mut images, 10.5);
        assert_ne!(film.image().as_ref(), Some(&first));
        assert_eq!(film.playing.as_ref().unwrap().1.started, 10.5);

        // Another clip gets a picture of its own.
        let before = film.image();
        film.want = Some("/MOVIES/XLP0009.NFX".into());
        run_film(&mut film, &mut read, &mut images, 11.0);
        assert_ne!(film.image(), before);
        film.want = None;
        run_film(&mut film, &mut read, &mut images, 12.0);
        assert_eq!(film.image(), None);
    }
}
