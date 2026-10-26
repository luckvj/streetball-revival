//! Online play: a match on a `street-net` server, hosted in this game or
//! run on its own (`street-server`).
//!
//! Every machine plays the match on its own disc's data: the referee (this
//! game when it hosts, or `street-server --vol2`) and every client install
//! the disc's animations, gait records and staged gamebreakers under the
//! match's own values, as a match played alone does (`setup.rs`,
//! `street-net`'s `disc`), so an online match plays the same rules. Only
//! digests and pads cross the network: the join carries this disc's
//! digest, and a server whose disc data differs refuses it.
//!
//! The one who creates the match loads it from their own disc first and
//! hands the server its setup: court, teams, line-ups, the world at tick 0
//! (the match's own values, not the disc's tables) and its roster
//! (`roster.rs`: the roster mod and `roster.json` as data, checked against
//! the online rules first). Everyone else gets that setup on joining,
//! checks the roster and the world against it, plays that roster for the
//! match over their own disc (names, team lists, moves, looks), loads the
//! same court and players, and takes the world as sent (so ratings and
//! rules are the creator's). The server refuses a setup whose roster breaks
//! the rules or whose world does not hold it. The lobby lists the seats;
//! the match starts once everyone seated is ready.
//!
//! In the match, [`net_step`] stands in for the offline stepping: each tick
//! it sends this machine's pads, steps the rollback session, and shows its
//! predicted world. A pad goes out raw (the pass button left in on defence;
//! the match turns it into a switch, `street-net`'s `game.rs`). The match
//! runs the game's replays too, on every machine, so a replay holds the
//! gameplay everywhere at once while the server's clock runs on; this game
//! shows its own drawing of the updates replayed (`tape.rs`). Nothing
//! pauses online.

use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};

use bevy::prelude::*;
use street_net::{
    Client, ClientConfig, ClientEvent, CloseReason, DEFAULT_PORT, Join, Joiner, Refusal, Server,
    ServerConfig, Status, UdpTransport,
};
use street_sim::{Input, Phase};

use crate::{
    Screen,
    controls::{Control, Seat},
    game::{Sim, Stats, TickEvents, note_events},
    menu::{self, MenuInput},
    setup::{self, Disc, PlayerNames, TEAM_SIZE, TeamNames},
};

/// How to go online.
#[derive(Clone, Debug)]
pub struct OnlineArgs {
    /// Host the server in this game, on this port.
    pub host: Option<u16>,
    /// The server to join (`host[:port]`), when not hosting.
    pub server: Option<String>,
    /// Create the match if the server has none by that name (hosting does).
    pub create: bool,
    pub match_name: String,
    pub name: String,
    /// 0 home, 1 away, or `None` for whichever has room.
    pub side: Option<u8>,
    /// Ready as soon as the match is loaded (for scripted runs).
    pub ready: bool,
    /// The computer plays this machine's seats (for testing).
    pub bot: bool,
    pub spectate: bool,
    /// People a hosted match seats: 2 or 6.
    pub people: u8,
}

impl Default for OnlineArgs {
    fn default() -> Self {
        Self {
            host: None,
            server: None,
            create: false,
            match_name: "street".into(),
            name: std::env::var("USERNAME")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "Player".into()),
            side: None,
            ready: false,
            bot: false,
            spectate: false,
            people: 6,
        }
    }
}

/// The Online page's choices (`menu.rs`'s `Page::Online`).
#[derive(Clone, Debug)]
pub struct OnlineForm {
    /// The join code (or an address) to join, as typed.
    pub server: String,
    pub name: String,
    pub side: Option<u8>,
    /// Host was picked: the match being chosen is for hosting.
    pub hosting: bool,
    /// People the hosted match seats: 2 (one on one) or 6 (three on three).
    pub people: u8,
    pub spectate: bool,
    pub match_name: String,
    pub matches: Vec<street_net::protocol::MatchInfo>,
    pub answered: bool,
    /// Matches hosted on this network, heard by [`LanFinder`].
    pub lan: Vec<(SocketAddr, street_net::lan::Beacon)>,
    /// The network's match the Join On Network row offers (left and right
    /// go through them).
    pub lan_pick: usize,
}

impl Default for OnlineForm {
    fn default() -> Self {
        let args = OnlineArgs::default();
        Self {
            server: String::new(),
            name: args.name,
            side: None,
            hosting: false,
            people: 6,
            spectate: false,
            match_name: args.match_name,
            matches: Vec::new(),
            answered: false,
            lan: Vec::new(),
            lan_pick: 0,
        }
    }
}

/// The Online page's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// Host a match for this many people (2 or 6).
    Host(u8),
    /// Join the `k`th match found on this network.
    Lan(usize),
    /// Type a join code (or address); joins once typed.
    Code,
    Name,
    Spectate,
    Browse,
}

/// The Main Menu's layout (class 3) the page is drawn on holds six rows.
pub const ROWS: usize = 6;

impl OnlineForm {
    /// The rows, top to bottom, on the Main Menu's six: hosting, the
    /// network's match (when one is heard), a code, a server's matches,
    /// the name, watching (when there is room). Circle backs out, as on
    /// Play It; the side is picked in the lobby.
    pub fn row_list(&self) -> Vec<Row> {
        let mut rows = vec![Row::Host(2), Row::Host(6)];
        if !self.lan.is_empty() {
            rows.push(Row::Lan(self.lan_pick.min(self.lan.len() - 1)));
        }
        rows.extend([Row::Code, Row::Browse, Row::Name, Row::Spectate]);
        rows.truncate(ROWS);
        rows
    }

    pub fn row(&self, k: usize) -> Row {
        self.row_list().get(k).copied().unwrap_or(Row::Host(2))
    }

    pub fn rows(&self) -> Vec<String> {
        self.row_list()
            .iter()
            .map(|row| match *row {
                Row::Host(2) => "Host 1 vs 1".into(),
                Row::Host(_) => "Host 3 vs 3".into(),
                Row::Lan(_) => "Join On Network".into(),
                Row::Code => "Join With Code".into(),
                Row::Name => "Change Name".into(),
                Row::Spectate if self.spectate => "Watch Only: On".into(),
                Row::Spectate => "Watch Only: Off".into(),
                Row::Browse => "Server Matches".into(),
            })
            .collect()
    }

    /// The help line for row `k`, with what the row stands for now (the
    /// network's match, the code typed, the name).
    pub fn help(&self, k: usize) -> String {
        match self.row(k) {
            Row::Host(2) => "Play one other person online.".into(),
            Row::Host(_) => "Up to six people, three a side.".into(),
            Row::Lan(i) => {
                let (_, b) = &self.lan[i];
                let size = if b.capacity <= 2 { "1 vs 1" } else { "3 vs 3" };
                let more = if self.lan.len() > 1 {
                    " Left/Right for others."
                } else {
                    ""
                };
                format!(
                    "{}'s {size}, {} of {} in.{more}",
                    b.host, b.people, b.capacity
                )
            }
            Row::Code if self.server.trim().is_empty() => {
                "Type the code the host sees to join.".into()
            }
            Row::Code => format!("Join {}.", self.server.trim()),
            Row::Name => format!("Your name online: {}.", self.name),
            Row::Spectate => "Join to watch the match without playing.".into(),
            Row::Browse => "The matches on the server typed under Join With Code.".into(),
        }
    }

    /// Left or right on row `k`.
    pub fn step(&mut self, k: usize, step: i32) {
        match self.row(k) {
            Row::Spectate => self.spectate = !self.spectate,
            Row::Lan(_) if !self.lan.is_empty() => {
                let n = self.lan.len() as i32;
                self.lan_pick = (self.lan_pick as i32 + step).rem_euclid(n) as usize;
            }
            _ => {}
        }
    }

    /// Going online from the menus: hosting on the default port, or
    /// joining the typed code. A code whose host is on this network joins
    /// over the network (home routers often can't loop back to
    /// themselves).
    pub fn args(&self, host: bool) -> OnlineArgs {
        let typed = self.server.trim();
        let server = self
            .lan
            .iter()
            .find(|(_, b)| !typed.is_empty() && same_code(&b.code, typed))
            .map_or_else(|| typed.to_owned(), |(addr, _)| addr.to_string());
        OnlineArgs {
            host: host.then_some(DEFAULT_PORT),
            server: (!host).then_some(server),
            name: self.name.clone(),
            side: if host {
                self.side.or(Some(0))
            } else {
                self.side
            },
            people: self.people,
            match_name: self.match_name.clone(),
            spectate: !host && self.spectate,
            ..OnlineArgs::default()
        }
    }

    /// Joining the `k`th match found on this network.
    pub fn args_lan(&self, k: usize) -> Option<OnlineArgs> {
        let (addr, beacon) = self.lan.get(k)?;
        Some(OnlineArgs {
            server: Some(addr.to_string()),
            name: self.name.clone(),
            side: self.side,
            match_name: beacon.match_name.clone(),
            spectate: self.spectate,
            ..OnlineArgs::default()
        })
    }
}

/// Two join codes the same, however typed.
fn same_code(a: &str, b: &str) -> bool {
    !a.is_empty()
        && street_net::code::decode(a).is_some()
        && street_net::code::decode(a) == street_net::code::decode(b)
}

/// Listens for matches hosted on this network while the Online page is
/// up (only then: one listener a machine, and a hosting game on this
/// machine mustn't hold it).
#[derive(Resource)]
pub(crate) struct LanFinder {
    finder: Option<street_net::lan::Finder>,
    start: Instant,
}

impl Default for LanFinder {
    fn default() -> Self {
        Self {
            finder: None,
            start: Instant::now(),
        }
    }
}

fn find_lan(mut finder: ResMut<LanFinder>, mut menus: ResMut<menu::Menus>) {
    if menus.top().0 != menu::Page::Online {
        finder.finder = None;
        return;
    }
    if finder.finder.is_none() {
        // Another game on this machine may be listening already; it then
        // has the list, and this one joins by code.
        finder.finder = street_net::lan::Finder::new()
            .map_err(|e| warn!("online: not listening for matches on this network: {e}"))
            .ok();
    }
    let now = finder.start.elapsed();
    let Some(f) = &mut finder.finder else { return };
    f.update(now);
    let found: Vec<_> = f
        .matches()
        .into_iter()
        .filter(|(_, b)| b.version == street_net::VERSION && b.people < b.capacity)
        .collect();
    if found != menus.online.lan {
        info!("online: on this network: {found:?}");
        menus.online.lan = found;
    }
}

/// A join code, or `host[:port]` with the port defaulting to
/// [`DEFAULT_PORT`].
pub fn resolve(server: &str) -> Result<SocketAddr, String> {
    if let Some(addr) = street_net::code::decode(server) {
        return Ok(SocketAddr::V4(addr));
    }
    let with_port = if server
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        server.to_owned()
    } else {
        format!("{server}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .map_err(|e| format!("{server}: {e}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| format!("{server}: no address"))
}

/// What identifies a loaded match: court, teams and line-ups.
type Key = (String, [usize; 2], [[usize; TEAM_SIZE]; 2]);

fn key(s: &setup::MatchSetup) -> Key {
    (s.court.to_ascii_uppercase(), s.teams, s.lineups)
}

/// The connection, while online.
#[derive(Resource)]
pub struct Online {
    start: Instant,
    client: Option<Client>,
    server: Option<Server>,
    addr: SocketAddr,
    join: Join,
    /// This game's disc: the hosted server's referee and the client play on it.
    disc: std::sync::Arc<street_net::Disc>,
    create: bool,
    hosting: Option<u16>,
    /// This machine's address on its network, for others to join.
    lan: Option<std::net::IpAddr>,
    /// The match last loaded from the disc.
    loaded: Option<Key>,
    /// The loaded match took the server's world and seats.
    synced: bool,
    ready: bool,
    auto_ready: bool,
    bot: bool,
    /// What the client reported since the last look.
    events: Vec<ClientEvent>,
    /// This machine voted to replay the finished round.
    rematch: bool,
    /// People the hosted match seats (2 or 6).
    people: u8,
    /// Hosting: the router opening our port, and the beacon others on this
    /// network see.
    reach: Option<crate::reach::Reach>,
    announcer: Option<street_net::lan::Announcer>,
    /// The setup this game sent to create the match.
    sent: Option<street_net::MatchSetup>,
    /// The match is picked in the lobby (`lobby.rs`), not ready-made
    /// (scripted runs with `--ready` play the setup they came with).
    lobby_flow: bool,
    /// Where this machine stands in the lobby, what it picks there, and
    /// the controller that set its side.
    pub(crate) stage: crate::lobby::Stage,
    duties: crate::lobby::Duties,
    device: crate::controls::Device,
}

impl Online {
    /// Go online as `args` says: bind the server if hosting, and get ready
    /// to join (the join goes out from the lobby). A hosted server
    /// referees on `disc`, this game's own, and checks rosters against
    /// `original` (the disc's tables).
    pub fn new(
        args: &OnlineArgs,
        original: Option<&crate::roster::Original>,
        disc: std::sync::Arc<street_net::Disc>,
    ) -> Result<Self, String> {
        let (server, addr) = match (args.host, &args.server) {
            (Some(port), _) => {
                let transport = UdpTransport::bind(("0.0.0.0", port))
                    .map_err(|e| format!("cannot host on port {port}: {e}"))?;
                let server = Server::new(
                    Box::new(transport),
                    ServerConfig {
                        seats_per_side: usize::from(args.people / 2).clamp(1, 3),
                        rosters: original.map(|o| o.for_server()).unwrap_or_default(),
                        ..ServerConfig::default()
                    },
                    disc.clone(),
                );
                (Some(server), SocketAddr::from(([127, 0, 0, 1], port)))
            }
            (None, Some(s)) => (None, resolve(s)?),
            (None, None) => return Err("neither hosting nor joining".into()),
        };
        let lan = args.host.and_then(|_| lan_address());
        Ok(Self {
            start: Instant::now(),
            client: None,
            server,
            addr,
            join: Join {
                match_name: args.match_name.clone(),
                players: if args.spectate {
                    Vec::new()
                } else {
                    vec![Joiner {
                        name: args.name.clone(),
                        team: args.side,
                    }]
                },
                create: None,
                disc: disc.digest(),
            },
            disc,
            create: args.create || args.host.is_some(),
            hosting: args.host,
            lan,
            loaded: None,
            synced: false,
            ready: false,
            auto_ready: args.ready,
            bot: args.bot,
            events: Vec::new(),
            rematch: false,
            people: args.people,
            reach: args
                .host
                .filter(|_| !args.bot)
                .map(|port| crate::reach::Reach::start(port, lan)),
            announcer: args
                .host
                .and_then(|_| street_net::lan::Announcer::new().ok()),
            sent: None,
            lobby_flow: !args.ready && !args.spectate,
            stage: crate::lobby::Stage::default(),
            duties: Default::default(),
            device: crate::controls::Device::Keyboard,
        })
    }

    /// The connection to the match, once joining.
    pub fn client(&self) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Whether this game makes the match (and so plays its own roster).
    pub fn creates(&self) -> bool {
        self.create
    }

    /// The join code friends type, once the router has answered.
    pub fn code(&self) -> Option<String> {
        self.reach.as_ref()?.outcome.as_ref()?.code()
    }

    /// Stop hosting: leave, close the server and the router's port (the
    /// handle of the router's closing, if it was open).
    fn shut(&mut self) -> Option<std::thread::JoinHandle<()>> {
        if let Some(client) = &mut self.client {
            client.leave();
        }
        if let Some(server) = &mut self.server {
            server.shutdown();
        }
        self.reach.take().and_then(|r| r.close())
    }

    /// Vote to replay the finished round. One vote covers this machine's
    /// seats; [`Online::rematch_voted`] says whether it went out.
    pub fn request_rematch(&mut self) -> bool {
        if let Some(client) = &mut self.client
            && client.request_rematch()
        {
            self.rematch = true;
        }
        self.rematch
    }

    /// Whether this machine has a seat and the match is still there to
    /// replay (and so may vote for a rematch).
    pub fn can_rematch(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|c| !c.seats().is_empty() && c.status() == street_net::Status::Playing)
    }

    /// Whether this machine voted to replay the round.
    pub fn rematch_voted(&self) -> bool {
        self.rematch
    }

    /// Whether this game loads its own match before joining (it creates the
    /// match), rather than waiting for the server's.
    pub fn loads_first(&self) -> bool {
        self.create
    }

    fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

/// The address this machine reaches the network from (a UDP "connect"
/// picks the route without sending anything).
fn lan_address() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// The match set up for play online.
pub fn is_online(online: Option<Res<Online>>) -> bool {
    online.is_some()
}

/// After the disc load: note what was loaded. Returns whether the game is
/// online (and so goes to the lobby rather than straight into the match).
pub(crate) fn mark_loaded(world: &mut World) -> bool {
    let k = key(world.resource::<setup::MatchSetup>());
    match world.get_resource_mut::<Online>() {
        Some(mut online) => {
            online.loaded = Some(k);
            online.synced = false;
            true
        }
        None => false,
    }
}

/// Our setup as the server's, for creating the match.
fn net_setup(
    s: &setup::MatchSetup,
    world: &street_sim::World,
    teams: [u16; 2],
    looks: &crate::net_looks::NetLooks,
    roster_data: Vec<u8>,
) -> street_net::MatchSetup {
    street_net::MatchSetup {
        court: s.court.clone(),
        teams,
        roster: s.lineups.iter().flatten().map(|&r| r as u16).collect(),
        seed: s.seed,
        seat_teams: [0, 1, 0, 1, 0, 1],
        world: world.clone(),
        extra: looks.encode(),
        roster_data,
    }
}

/// The world as an online match plays it: each player's ratings straight
/// from its record, as `setup.rs` makes them but without the story crew's
/// Gym development (a match against other people plays the roster as it
/// is, and the server holds the world to it), the cheat codes applied.
fn plain_world(
    world: &street_sim::World,
    lineup: &[street_assets::db::Player],
    cheats: &crate::cheats::Settings,
) -> street_sim::World {
    let mut w = world.clone();
    for (p, r) in w.players.iter_mut().zip(lineup) {
        let mut ratings = setup::ratings(r, &[0; 9]);
        cheats.apply_ratings(&mut ratings);
        p.ratings = ratings;
    }
    if let Some(gait) = &w.gait {
        let mut gait = (**gait).clone();
        for (profile, p) in gait.profiles.iter_mut().zip(&w.players) {
            profile.handles = (p.ratings.handles * 99.0).round() as i32;
        }
        w.gait = Some(std::sync::Arc::new(gait));
    }
    w
}

/// The server's setup as a match to load from our disc.
fn local_setup(net: &street_net::MatchSetup, disc: &Disc) -> Result<setup::MatchSetup, String> {
    if net.court.is_empty() {
        return Err("The server has no match by that name.".into());
    }
    if net.roster.len() != 2 * TEAM_SIZE || net.world.players.len() != 2 * TEAM_SIZE {
        return Err("The match is not three on three.".into());
    }
    let teams = net.teams.map(usize::from);
    if teams.iter().any(|&t| t >= disc.teams.len()) {
        return Err("The match has a team this disc does not.".into());
    }
    let record = |k: usize| usize::from(net.roster[k]);
    if (0..net.roster.len()).any(|k| record(k) >= disc.players.len()) {
        return Err("The match has a player this disc does not.".into());
    }
    Ok(setup::MatchSetup {
        teams,
        lineups: std::array::from_fn(|t| std::array::from_fn(|k| record(t * TEAM_SIZE + k))),
        court: net.court.clone(),
        rules: net.world.rules,
        seed: net.seed,
        demo: false,
        watch: false,
        seats: Vec::new(),
        users: Vec::new(),
        online: true,
    })
}

/// The people at this machine: one seat per seat the server gave us, on
/// every controller, each starting on the lowest-numbered teammate (as the
/// match seats them).
fn control_for(client: &Client, net: &street_net::MatchSetup) -> Control {
    let w = &net.world;
    let mut seats: Vec<Seat> = Vec::new();
    for &seat in client.seats() {
        let team = usize::from(net.seat_teams[usize::from(seat)]);
        let taken: Vec<usize> = seats.iter().map(|s| s.player).collect();
        let player = (0..w.players.len())
            .find(|&j| w.team_of(j) == team && !taken.contains(&j))
            .unwrap_or(0);
        // Numbered by the match's seat, so every machine shows a person
        // in the same colour.
        seats.push(Seat {
            number: usize::from(seat),
            device: None,
            team,
            player,
            pad: Input::default(),
            raw: Input::default(),
            edges: street_sim::input::ButtonEdges::default(),
        });
    }
    Control {
        seats,
        demo: false,
        remote: Vec::new(),
    }
}

pub struct OnlinePlugin;

#[derive(Resource)]
pub(crate) struct MatchBrowser {
    pub browser: street_net::Browser,
    start: Instant,
}
impl MatchBrowser {
    pub fn new(server: &str) -> Result<Self, String> {
        let addr = resolve(server)?;
        let socket = UdpTransport::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        Ok(Self {
            browser: street_net::Browser::new(Box::new(socket), addr),
            start: Instant::now(),
        })
    }
}
fn browse(mut browser: Option<ResMut<MatchBrowser>>, mut menus: ResMut<menu::Menus>) {
    if let Some(b) = browser.as_mut() {
        let now = b.start.elapsed();
        b.browser.update(now);
        menus.online.matches = b.browser.matches().to_vec();
        menus.online.answered = b.browser.answered();
    }
}

impl Plugin for OnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LanFinder>()
            .add_systems(OnEnter(Screen::Lobby), spawn_lobby)
            .add_systems(Update, (browse, find_lan).run_if(in_state(Screen::Menu)))
            .add_systems(OnEnter(Screen::Menu), go_offline)
            .add_systems(
                Update,
                (start_from_menu, back_to_lobby).run_if(in_state(Screen::Menu)),
            )
            .add_systems(Last, leave_on_exit.run_if(is_online))
            .add_systems(
                Update,
                (
                    pump.run_if(is_online),
                    lobby.run_if(in_state(Screen::Lobby)),
                    watch.run_if(in_state(Screen::Playing)).run_if(is_online),
                )
                    .chain(),
            );
    }
}

fn spawn_lobby(
    mut commands: Commands,
    art: Option<Res<crate::art::Art>>,
    online: Option<Res<Online>>,
) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.04, 0.04, 0.07)),
            ..default()
        },
        DespawnOnExit(Screen::Lobby),
    ));
    // User Setup's backdrop behind the lobby's columns (`lobby_page`); the
    // options backdrop behind a scripted match's list ([`lobby`]).
    let backdrop = if online.is_some_and(|o| o.lobby_flow) {
        crate::users::BACKDROP
    } else {
        crate::art::backdrop::OPTIONS
    };
    if let Some(art) = art {
        commands.spawn((
            ImageNode::new(art.backdrop(backdrop)).with_mode(NodeImageMode::Stretch),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            DespawnOnExit(Screen::Lobby),
        ));
    }
}

/// The server (if hosting) and the client, once a frame.
fn pump(mut online: ResMut<Online>) {
    let now = online.now();
    if let Some(reach) = &mut online.reach {
        let waiting = reach.outcome.is_none();
        if let Some(outcome) = reach.poll()
            && waiting
        {
            info!("online: router: {outcome:?}");
        }
    }
    let beacon = online.hosting.map(|port| street_net::lan::Beacon {
        version: street_net::VERSION,
        port,
        people: online.client.as_ref().map_or(0, |c| c.lobby().len()) as u8,
        capacity: online.people,
        started: online
            .client
            .as_ref()
            .is_some_and(|c| c.status() == Status::Playing),
        host: online
            .join
            .players
            .first()
            .map_or_else(|| "Someone".into(), |p| p.name.clone()),
        match_name: online.join.match_name.clone(),
        code: online.code().unwrap_or_default(),
    });
    if let (Some(announcer), Some(beacon)) = (&mut online.announcer, beacon) {
        announcer.update(now, &beacon);
    }
    if let Some(server) = &mut online.server {
        for e in server.update(now) {
            info!("server: {e:?}");
        }
    }
    if let Some(client) = &mut online.client {
        let events = client.update(now);
        for e in &events {
            info!("online: {e:?}");
        }
        online.events.extend(events);
    }
}

/// Closing the game leaves the match at once, rather than after the
/// server's timeout (the computer takes over our players).
fn leave_on_exit(mut exit: MessageReader<bevy::app::AppExit>, mut online: ResMut<Online>) {
    if exit.read().next().is_none() {
        return;
    }
    // The router's port closes before the game does (a few seconds at
    // most: the router search times out).
    if let Some(closing) = online.shut() {
        let _ = closing.join();
    }
}

/// Host or Join on the Online page: go online, to the disc load (hosting)
/// or the lobby (joining).
fn start_from_menu(
    mut commands: Commands,
    mut menus: ResMut<menu::Menus>,
    mut setup: ResMut<setup::MatchSetup>,
    mut message: ResMut<menu::Message>,
    mut next: ResMut<NextState<Screen>>,
    (original, host_check, clips): (
        Option<Res<crate::roster::Original>>,
        Option<Res<crate::roster::HostCheck>>,
        Option<Res<crate::setup::GameClips>>,
    ),
) {
    let Some(args) = menus.go_online.take() else {
        return;
    };
    // Hosting plays this game's roster: only a legal one goes online.
    if (args.host.is_some() || args.create)
        && let Some(Err(why)) = host_check.as_deref().map(|c| &c.0)
    {
        message.0 = why.clone();
        return;
    }
    let Some(clips) = clips else {
        message.0 = "The disc's animations are not loaded.".into();
        return;
    };
    match Online::new(&args, original.as_deref(), clips.sim.clone()) {
        Ok(online) => {
            next.set(if online.loads_first() {
                Screen::Loading
            } else {
                Screen::Lobby
            });
            commands.insert_resource(online);
            setup.online = true;
            setup.seats.clear();
            setup.demo = false;
        }
        Err(why) => message.0 = why,
    }
}

/// Picking for the lobby on the menus: done (the menus set
/// `online_picked`), or backed out past the first page, back to the
/// lobby.
fn back_to_lobby(
    online: Option<ResMut<Online>>,
    mut menus: ResMut<menu::Menus>,
    mut next: ResMut<NextState<Screen>>,
    (mut setup, disc, save): (
        ResMut<setup::MatchSetup>,
        Res<Disc>,
        Res<crate::story::Save>,
    ),
) {
    let Some(mut online) = online else {
        return;
    };
    if online.stage != crate::lobby::Stage::Picking {
        return;
    }
    mirror_picks(&online, &mut menus, &mut setup, &disc, &save.story);
    if menus.online_picked {
        next.set(Screen::Lobby);
    } else if menus.top().0 == menu::Page::Online {
        // Backed out: set a side again.
        menus.online_pick = None;
        online.stage = crate::lobby::Stage::Sides;
        next.set(Screen::Lobby);
    }
}

/// While this machine picks, the sides other machines pick for: whose they
/// are, and their team and three once in (`Menus::mirror_side`), so both
/// screens show the other side as it stands.
fn mirror_picks(
    online: &Online,
    menus: &mut menu::Menus,
    setup: &mut setup::MatchSetup,
    disc: &Disc,
    story: &street_sim::story::Story,
) {
    let Some(client) = online.client.as_ref() else {
        return;
    };
    let lobby = client.lobby();
    for side in 0..2usize {
        if online.duties.sides.contains(&side) {
            continue;
        }
        let first = lobby
            .iter()
            .filter(|e| usize::from(e.team) == side)
            .min_by_key(|e| e.seat);
        let Some(first) = first else {
            menus.online_others[side] = None;
            continue;
        };
        let pick = lobby
            .iter()
            .flat_map(|e| crate::lobby::decode(&e.pick).1)
            .find(|p| p.side == side);
        let picked = pick.is_some();
        if let Some(pick) = pick
            && (setup.teams[side], setup.lineups[side]) != (pick.team, pick.lineup)
        {
            menus.mirror_side(pick, setup, disc, story);
        }
        menus.online_others[side] = Some((usize::from(first.seat), picked));
    }
}

/// Leave the match and stop hosting; back to this game's own roster.
fn go_offline(
    mut commands: Commands,
    online: Option<ResMut<Online>>,
    mut setup: ResMut<setup::MatchSetup>,
    mut menus: ResMut<menu::Menus>,
    (mut disc, save): (ResMut<Disc>, Res<crate::story::Save>),
) {
    // Picking a lobby's teams on the menus is still online.
    if online
        .as_ref()
        .is_some_and(|o| o.stage == crate::lobby::Stage::Picking)
    {
        return;
    }
    setup.online = false;
    menus.online.hosting = false;
    if let Err(e) = crate::roster::leave_shared(&mut disc, &save.story) {
        warn!("roster: back to this game's own: {e}");
    }
    let Some(mut online) = online else {
        return;
    };
    let _ = online.shut();
    commands.remove_resource::<Online>();
    commands.remove_resource::<crate::net_looks::NetLooks>();
}

/// Back to the menus, saying why.
fn fail(message: &mut menu::Message, next: &mut NextState<Screen>, why: String) {
    message.0 = why;
    next.set(Screen::Menu);
}

fn refusal(r: Refusal, original: &crate::roster::Original) -> String {
    let text = match r {
        Refusal::Version => "The server runs another version of the game.",
        Refusal::Full => "That side is full.",
        Refusal::Busy => "The server is hosting as many matches as it will.",
        Refusal::Invalid => "The server turned the match down.",
        Refusal::Roster => {
            "The server refused the match's roster: it breaks the online rules \
             (sizes, ratings or moves), or the match does not play it."
        }
        Refusal::Disc => {
            "This disc's data differs from the server's (another region's disc, or a \
             modded one): every player in a match plays the same disc."
        }
        Refusal::NoMatch => "The server has no match by that name.",
        Refusal::RosterMod => {
            return format!(
                "This server doesn't have the {} roster mod this match is built on \
                 (its operator adds it with street-server --roster-db).",
                original
                    .mod_id()
                    .map_or_else(|| "same".into(), |id| street_assets::roster_share::hex(&id))
            );
        }
    };
    text.into()
}

fn closed(r: CloseReason) -> &'static str {
    match r {
        CloseReason::Over => "The match is over.",
        CloseReason::TimedOut => "The server stopped hearing from this game.",
        CloseReason::Shutdown => "The server shut down.",
        CloseReason::Fault => {
            "This match stopped because of an internal error. Other matches remain available."
        }
    }
}

/// The match this game creates, as the server takes it: the loaded
/// `world` made plain, the roster as data, the looks, checked as a server
/// would check it (saying why not).
fn creation(
    setup: &setup::MatchSetup,
    world: &street_sim::World,
    disc: &mut Disc,
    original: &crate::roster::Original,
    save: &crate::story::Save,
    cheats: &crate::cheats::Settings,
    options: Option<&crate::options::Options>,
) -> Result<street_net::MatchSetup, String> {
    let looks = crate::net_looks::NetLooks {
        cheats: cheats.clone(),
        ballers: save
            .story
            .ballers
            .iter()
            .filter(|b| setup.lineups.iter().flatten().any(|&r| r == b.record()))
            .cloned()
            .collect(),
        teams: setup
            .teams
            .map(|t| (disc.teams[t].city.clone(), disc.teams[t].nickname.clone())),
    };
    let teams = setup.teams.map(|t| {
        if t < disc.base_teams {
            t as u16
        } else {
            disc.teams[t].logo.clamp(0, disc.base_teams as i16 - 1) as u16
        }
    });
    // The roster goes along as data, with the line-up's records as the
    // world was made from them.
    let lineup: Vec<street_assets::db::Player> = setup
        .lineups
        .iter()
        .flatten()
        .map(|&r| disc.players[r].clone())
        .collect();
    let roster_data = crate::roster::match_roster(disc, original, &lineup)
        .map_err(|why| format!("Cannot go online: {why}"))?;
    let mut world = plain_world(world, &lineup, &looks.cheats);
    // The match plays its creator's Replays option, as its rules (every
    // machine runs the replays: street-net's `Match`).
    world.replays = options.map_or(1, |o| o.0.replays);
    let net = net_setup(setup, &world, teams, &looks, roster_data);
    // What a server would refuse is refused here, saying why.
    street_net::roster::check_setup(&net, &original.0)
        .map_err(|why| crate::roster::refusal(&why.reasons()))?;
    Ok(net)
}

/// The lobby: join, load the server's match if it is not ours, then wait
/// for everyone to be ready.
fn lobby(
    mut commands: Commands,
    mut online: ResMut<Online>,
    mut menus: ResMut<menu::Menus>,
    mut setup: ResMut<setup::MatchSetup>,
    (mut disc, original): (ResMut<Disc>, Res<crate::roster::Original>),
    (save, cheats, options): (
        Res<crate::story::Save>,
        Res<crate::cheats::Settings>,
        Option<Res<crate::options::Options>>,
    ),
    sim: Option<ResMut<Sim>>,
    mut stats: Option<ResMut<crate::game::Stats>>,
    mut running: ResMut<crate::story::Running>,
    input: MenuInput,
    mut message: ResMut<menu::Message>,
    mut next: ResMut<NextState<Screen>>,
    (look, mut labels, mut images, mut slots): (
        Option<Res<crate::look::Look>>,
        ResMut<crate::look::Labels>,
        ResMut<Assets<Image>>,
        Query<(
            &crate::look::Slot,
            &mut ImageNode,
            &mut Node,
            &mut Visibility,
        )>,
    ),
) {
    // Join: the creator brings its loaded match along.
    if online.client.is_none() {
        let create = match (&sim, online.create) {
            (Some(sim), true) => {
                match creation(
                    &setup,
                    &sim.world,
                    &mut disc,
                    &original,
                    &save,
                    &cheats,
                    options.as_deref(),
                ) {
                    Ok(net) => {
                        online.sent = Some(net.clone());
                        Some(net)
                    }
                    Err(why) => return fail(&mut message, &mut next, why),
                }
            }
            _ => None,
        };
        let transport = match UdpTransport::bind("0.0.0.0:0") {
            Ok(t) => t,
            Err(e) => return fail(&mut message, &mut next, format!("Cannot go online: {e}")),
        };
        let mut join = online.join.clone();
        join.create = create;
        if join.create.is_some() {
            let whole = street_net::Message::Join(join.clone()).encode().len();
            info!(
                "online: creating the match: {whole} bytes in {} datagrams",
                whole.div_ceil(street_net::protocol::MAX_DATAGRAM - 7)
            );
        }
        let addr = online.addr;
        online.client = Some(Client::new(
            Box::new(transport),
            addr,
            join,
            ClientConfig {
                // Loading and a first frame's shaders can stall a game for
                // seconds; be patient before calling the server gone.
                timeout: Duration::from_secs(10),
                ..ClientConfig::default()
            },
            online.disc.clone(),
        ));
        return;
    }
    for e in std::mem::take(&mut online.events) {
        match e {
            ClientEvent::Refused(r) => return fail(&mut message, &mut next, refusal(r, &original)),
            ClientEvent::Closed(r) => return fail(&mut message, &mut next, closed(r).into()),
            ClientEvent::TimedOut => {
                return fail(&mut message, &mut next, "No answer from the server.".into());
            }
            // The match as everyone picked it: load it.
            ClientEvent::Setup { version } => {
                info!("online: the lobby's match, version {version}");
                online.synced = false;
            }
            _ => {}
        }
    }
    let synced_before = online.synced;
    let online = &mut *online;
    let client = online.client.as_mut().expect("joined above");
    // Before the match is picked, the lobby (`lobby.rs`).
    if online.lobby_flow && client.setup_version() == 0 && client.status() == Status::Lobby {
        let view = LobbyView {
            look: look.as_deref(),
            labels: &mut labels,
            images: &mut images,
        };
        match decide(
            online,
            &mut menus,
            &mut setup,
            &input,
            &mut next,
            sim.as_deref().map(|s| &s.world),
            (&mut disc, &original, &save, &cheats, options.as_deref()),
            view,
        ) {
            Ok(pieces) => crate::look::show(&mut commands, &mut slots, &pieces, Screen::Lobby),
            Err(why) => fail(&mut message, &mut next, why),
        }
        return;
    }
    // Seated: load the server's match unless it is the one loaded.
    if let Some(net) = client.setup().cloned()
        && !online.synced
    {
        let looks = match crate::net_looks::NetLooks::decode(&net.extra) {
            Ok(looks) => looks,
            Err(why) => return fail(&mut message, &mut next, why),
        };
        commands.insert_resource(looks);
        // Someone else's match: check its roster and world, and play its
        // roster (`roster.rs`). Ours is the roster this game plays.
        let ours = online.sent.as_ref().is_some_and(|sent| {
            (&sent.roster_data, &sent.roster, sent.seed, &sent.court)
                == (&net.roster_data, &net.roster, net.seed, &net.court)
        });
        if !ours && let Err(why) = crate::roster::play_shared(&mut disc, &original, &net) {
            return fail(&mut message, &mut next, why);
        }
        let want = match local_setup(&net, &disc) {
            Ok(w) => w,
            Err(why) => return fail(&mut message, &mut next, why),
        };
        if online.loaded.as_ref() != Some(&key(&want)) {
            commands.insert_resource(want);
            next.set(Screen::Loading);
            return;
        }
        if let Some(mut sim) = sim {
            sim.world = net.world.clone();
            let names: Vec<String> = want
                .lineups
                .iter()
                .flatten()
                .map(|&r| disc.players[r].name())
                .collect();
            info!(
                "online: {} vs {}: {}",
                disc.teams[want.teams[0]].full_name(),
                disc.teams[want.teams[1]].full_name(),
                names.join(", ")
            );
            commands.insert_resource(control_for(client, &net));
            // A replayed round counts from scratch again.
            if let Some(stats) = &mut stats {
                **stats = crate::game::Stats::new(net.world.players.len());
            }
            online.synced = true;
            running.0 = None;
        }
    }
    // `--ready`: whoever made the match waits for someone to play. Through
    // the lobby, everyone is ready once the picked match is loaded.
    if online.synced
        && !online.ready
        && ((online.auto_ready && (!online.create || client.lobby().len() >= 2))
            || (online.lobby_flow && client.setup_version() > 0))
    {
        online.ready = true;
        client.set_ready(true);
    }
    if online.synced && synced_before && !online.lobby_flow && input.confirm() {
        online.ready = !online.ready;
        client.set_ready(online.ready);
    }
    if input.back() {
        return fail(&mut message, &mut next, String::new());
    }
    if online.synced && client.status() == Status::Playing {
        next.set(Screen::Playing);
    }

    // Through the lobby, its page to the end (`lobby_page`).
    if online.lobby_flow {
        let lobby = client.lobby().to_vec();
        let mine = client.seats().to_vec();
        let view = LobbyView {
            look: look.as_deref(),
            labels: &mut labels,
            images: &mut images,
        };
        let pieces = lobby_page(online, &lobby, &mine, view);
        crate::look::show(&mut commands, &mut slots, &pieces, Screen::Lobby);
        return;
    }
    // The page, in the menus' own fonts: the join code as the title, the
    // match, everyone seated, and what happens next.
    let code = online
        .reach
        .as_ref()
        .and_then(|r| r.outcome.as_ref()?.code());
    let title = match (&online.hosting, &code) {
        (Some(_), Some(code)) => format!("Code  {code}"),
        (Some(_), None) => "Hosting".to_owned(),
        (None, _) => "Online Match".to_owned(),
    };
    let mut rows: Vec<String> = Vec::new();
    if let Some(net) = client.setup()
        && !net.court.is_empty()
    {
        let team = |t: u16| {
            disc.teams
                .get(usize::from(t))
                .map_or_else(|| "?".into(), |t| t.full_name())
        };
        rows.push(format!(
            "{} vs {} at {}",
            team(net.teams[0]),
            team(net.teams[1]),
            disc.court_name(&net.court)
        ));
    }
    let seated = client.lobby();
    for s in seated {
        let ours = client.seats().contains(&s.seat);
        rows.push(format!(
            "{}P  {}  {}{}  {}",
            s.seat + 1,
            if s.team == 0 { "Home" } else { "Away" },
            s.name,
            if ours { " (you)" } else { "" },
            if s.ready { "READY" } else { "..." },
        ));
    }
    if online.hosting.is_some() {
        // For joining by address (a VPN, or a router that loops back).
        if let Some(ip) = online.lan {
            rows.push(format!("Address on your network: {ip}"));
        }
        let size = if online.people <= 2 {
            "1 vs 1"
        } else {
            "3 vs 3"
        };
        rows.push(format!(
            "{size}: {} of {} joined, the computer plays the rest",
            seated.len().min(usize::from(online.people)),
            online.people
        ));
    }
    let reach = online.reach.as_ref().and_then(|r| r.outcome.clone());
    let help = match client.status() {
        Status::Joining => "Joining...".to_owned(),
        _ if !online.synced => "Loading the match...".to_owned(),
        _ if online.reach.is_some() && reach.is_none() => {
            "Opening your router for friends online...".to_owned()
        }
        _ if online.ready => "Ready. Waiting for everyone else.".to_owned(),
        _ => match reach {
            Some(crate::reach::Outcome::Open { .. }) => {
                "Friends type the code under Online, Join With Code. Press Enter when ready.".into()
            }
            Some(crate::reach::Outcome::Closed { public: Some(_) }) => {
                "Your router would not open the game (UPnP). Online friends need UDP 7375 forwarded."
                    .into()
            }
            Some(crate::reach::Outcome::Closed { public: None })
            | Some(crate::reach::Outcome::Shared) => {
                "Only your own network can join this match. Let a friend host instead.".into()
            }
            None => "Press Cross / A / Enter when ready.".into(),
        },
    };
    let keys = match client.rtt() {
        Some(rtt) => format!("Enter: ready    Esc: leave    Ping {} ms", rtt.as_millis()),
        None => "Enter: ready    Esc: leave".to_owned(),
    };
    if let Some(look) = look.as_deref() {
        let view = crate::look::View {
            style: crate::look::Style::List,
            title: &title,
            warm: false,
            items: &rows,
            cursor: usize::MAX,
            help: &help,
            keys: &keys,
            left: crate::look::LIST_X,
            ticks: 0,
        };
        let pieces = crate::look::build(&view, look, &mut labels, &mut images);
        crate::look::show(&mut commands, &mut slots, &pieces, Screen::Lobby);
    }
}

/// What the lobby's page draws with.
struct LobbyView<'a> {
    look: Option<&'a crate::look::Look>,
    labels: &'a mut crate::look::Labels,
    images: &'a mut Assets<Image>,
}

/// The lobby before the match is picked (`lobby.rs`): sides, then each
/// side's picks on the menus, then the creator builds the match.
#[allow(clippy::too_many_arguments)]
fn decide(
    online: &mut Online,
    menus: &mut menu::Menus,
    setup: &mut setup::MatchSetup,
    input: &MenuInput,
    next: &mut NextState<Screen>,
    world: Option<&street_sim::World>,
    (disc, original, save, cheats, options): (
        &mut Disc,
        &crate::roster::Original,
        &crate::story::Save,
        &crate::cheats::Settings,
        Option<&crate::options::Options>,
    ),
    view: LobbyView,
) -> Result<Vec<crate::look::Piece>, String> {
    use crate::lobby::{self, SidePick, Stage};
    let creator = online.create;
    let Some(client) = online.client.as_mut() else {
        return Ok(Vec::new());
    };
    let lobby: Vec<street_net::SeatEntry> = client.lobby().to_vec();
    let mine: Vec<u8> = client.seats().to_vec();
    // Back from the menus with this machine's picks.
    if online.stage == Stage::Picking && menus.online_picked {
        menus.online_picked = false;
        menus.online_pick = None;
        setup.seats.clear();
        online.stage = Stage::Picked;
    }
    match online.stage {
        Stage::Sides => {
            // The side chooser: up for AWAY, down for HOME (`FUN_00245b88`).
            if input.up() {
                client.choose_side(1);
            } else if input.down() {
                client.choose_side(0);
            }
            if input.confirm() && !mine.is_empty() {
                online.device = input
                    .confirmed_by()
                    .unwrap_or(crate::controls::Device::Keyboard);
                online.stage = Stage::SidesDone;
            } else if input.back() {
                return Err(String::new());
            }
        }
        Stage::SidesDone if input.back() => online.stage = Stage::Sides,
        _ => {}
    }
    // Everyone's side set: each side's first person picks.
    if online.stage == Stage::SidesDone && lobby::everyone_at(&lobby, 1) {
        let duties = lobby::duties(&lobby, &mine, creator);
        online.duties = duties.clone();
        if duties.sides.is_empty() && !duties.court {
            online.stage = Stage::Picked;
        } else {
            online.stage = Stage::Picking;
            setup.seats = duties
                .sides
                .first()
                .map(|&s| vec![(online.device, s)])
                .unwrap_or_default();
            menus.online_picked = false;
            menus.begin_online_pick(duties, setup, disc, &save.story);
            next.set(Screen::Menu);
        }
    }
    // What this machine shows the lobby.
    let picks: Vec<SidePick> = if online.stage.shown() >= 3 {
        online
            .duties
            .sides
            .iter()
            .map(|&side| SidePick {
                side,
                team: setup.teams[side],
                lineup: setup.lineups[side],
            })
            .collect()
    } else {
        Vec::new()
    };
    client.set_pick(lobby::encode(online.stage, &picks));
    // The creator, with every side picked: build the match and load it,
    // then hand it to the server.
    if creator
        && online.stage == Stage::Picked
        && let Some((teams, lineups)) = lobby::compose(&lobby, (setup.teams, setup.lineups))
    {
        setup.teams = teams;
        setup.lineups = lineups;
        setup.seats.clear();
        online.stage = Stage::Building;
        next.set(Screen::Loading);
        return Ok(Vec::new());
    }
    if online.stage == Stage::Building
        && let Some(world) = world
        && online.loaded.as_ref() == Some(&key(setup))
    {
        let net = creation(setup, world, disc, original, save, cheats, options)?;
        online.sent = Some(net.clone());
        if let Some(client) = online.client.as_mut() {
            client.propose_setup(net);
        }
        online.stage = Stage::Proposed;
    }
    Ok(lobby_page(online, &lobby, &mine, view))
}

/// The lobby's page: User Setup's look (layout 11), a column per person by
/// seat in their colour, the side chooser while they choose, "Ready" under
/// their side once set; the join code and ping small along the bottom.
fn lobby_page(
    online: &Online,
    lobby: &[street_net::SeatEntry],
    mine: &[u8],
    view: LobbyView,
) -> Vec<crate::look::Piece> {
    use crate::lobby::{self, Stage};
    let Some(look) = view.look else {
        return Vec::new();
    };
    let mut people: Vec<&street_net::SeatEntry> = lobby.iter().collect();
    people.sort_by_key(|e| e.seat);
    let all_set = lobby::everyone_at(lobby, 1);
    let columns: Vec<crate::users::Column> = people
        .iter()
        .map(|e| {
            let side = usize::from(e.team);
            if lobby::stage_of(e) == 0 {
                crate::users::Column::Sides { side: Some(side) }
            } else {
                crate::users::Column::Ready { side, go: all_set }
            }
        })
        .collect();
    let numbers: Vec<usize> = people.iter().map(|e| usize::from(e.seat)).collect();
    let places = crate::users::places(&numbers);
    let lit = people.iter().position(|e| mine.contains(&e.seat));
    let title = look
        .text("entername.txt", 0)
        .unwrap_or("User Setup")
        .to_owned();
    let picking: Vec<&str> = people
        .iter()
        .filter(|e| lobby::stage_of(e) == 2)
        .map(|e| e.name.as_str())
        .collect();
    let picked = online
        .client
        .as_ref()
        .is_some_and(|c| c.setup_version() > 0);
    let help = match online.stage {
        _ if picked && online.ready => "Ready. Waiting for everyone else.".to_owned(),
        _ if picked => "Loading the match...".to_owned(),
        Stage::Sides => "Choose a side: up for AWAY, down for HOME. Cross when set.".to_owned(),
        Stage::SidesDone => "Waiting for everyone to choose a side.".to_owned(),
        _ if !picking.is_empty() => format!("{} picking...", picking.join(", ")),
        Stage::Building | Stage::Proposed => "Loading the match...".to_owned(),
        _ => "Waiting for the other side's picks.".to_owned(),
    };
    // The host's join code (or why there is none) under the help: User
    // Setup's layout draws the original's button legends, not the keys.
    let help = match hosting_line(online) {
        Some(line) => format!(
            "{help}
{line}"
        ),
        None => help,
    };
    let client = online.client.as_ref();
    let mut keys = Vec::new();
    if let Some(rtt) = client.and_then(|c| c.rtt()) {
        keys.push(format!("Ping {} ms", rtt.as_millis()));
    }
    let keys = keys.join("    ");
    let page = crate::look::View {
        style: crate::look::Style::Screen(11),
        title: &title,
        warm: false,
        items: &[],
        cursor: usize::MAX,
        help: &help,
        keys: &keys,
        left: crate::look::LIST_X,
        ticks: 0,
    };
    let mut pieces = crate::look::build(&page, look, view.labels, view.images);
    pieces.extend(crate::users::pieces_in(
        look,
        view.labels,
        view.images,
        &columns,
        &places,
        lit,
        0,
    ));
    pieces
}

/// The host's line on the lobby: the join code friends type, or what the
/// router said when there is none, and this machine's address on its own
/// network.
fn hosting_line(online: &Online) -> Option<String> {
    use crate::reach::Outcome;
    online.hosting?;
    let network = online
        .lan
        .map(|ip| format!("    Network {ip}"))
        .unwrap_or_default();
    let reach = online.reach.as_ref().map(|r| r.outcome.as_ref());
    Some(match reach {
        Some(None) => format!("Getting your join code...{network}"),
        Some(Some(Outcome::Open { public })) => {
            format!("Join code {}{network}", street_net::code::encode(*public))
        }
        Some(Some(Outcome::Closed {
            public: Some(public),
        })) => format!(
            "Join code {} (forward UDP {} on your router){network}",
            street_net::code::encode(*public),
            public.port()
        ),
        _ => format!("No join code: only your own network can join{network}"),
    })
}

/// In the match: the connection dropping ends it, unless it is over anyway.
fn watch(
    mut online: ResMut<Online>,
    sim: Res<Sim>,
    mut message: ResMut<menu::Message>,
    mut next: ResMut<NextState<Screen>>,
) {
    let over = matches!(sim.world.phase, Phase::Over { .. });
    for e in std::mem::take(&mut online.events) {
        let why = match e {
            ClientEvent::Closed(r) if !over => closed(r),
            ClientEvent::TimedOut if !over => "Lost the connection to the server.",
            ClientEvent::Resynced { tick } => {
                warn!("online: resynced with the server at tick {tick}");
                continue;
            }
            // Everyone voted to replay: back to the lobby for the new
            // round, which re-syncs the world and starts again.
            ClientEvent::Restarted { round } => {
                info!("online: the match restarts, round {round}");
                online.rematch = false;
                online.synced = false;
                next.set(Screen::Lobby);
                continue;
            }
            _ => continue,
        };
        message.0 = why.into();
        next.set(Screen::Menu);
    }
}

/// One fixed update online: the ticks due, each with this machine's pads,
/// and the predicted world shown. The box score counts confirmed ticks
/// only.
pub fn net_step(
    mut online: ResMut<Online>,
    mut sim: ResMut<Sim>,
    mut control: ResMut<Control>,
    mut tick_events: ResMut<TickEvents>,
    mut stats: ResMut<Stats>,
    names: Res<PlayerNames>,
    teams: Res<TeamNames>,
) {
    tick_events.events.clear();
    tick_events.replay = false;
    let bot = online.bot;
    let Some(client) = online.client.as_mut() else {
        return;
    };
    // This runs on the fixed 60 Hz update, which is the clock.
    let due = client.ticks_due_fixed();
    if due == 0 {
        return;
    }
    let seats = client.seats().to_vec();
    let mut events = Vec::new();
    for _ in 0..due {
        for (k, &seat) in seats.iter().enumerate() {
            let pad = match (control.seats.get(k), client.world()) {
                (Some(s), Some(w)) if bot => street_sim::ai::input(w, s.player),
                (Some(s), _) => s.raw,
                _ => Input::default(),
            };
            client.add_local_input(seat, pad);
        }
        if client.advance().is_some()
            && let Some(w) = client.world()
        {
            events.extend(w.events.iter().cloned());
        }
    }
    let Some(m) = client.session().map(|s| s.current()) else {
        return;
    };
    sim.world = m.world.clone();
    // A replay holds the gameplay (the match's, on every machine), and a
    // staged script's end after its replay.
    tick_events.replay = m.replays.holds_world();
    for (k, &seat) in seats.iter().enumerate() {
        if let (Some(s), Some(player)) = (
            control.seats.get_mut(k),
            m.seats.get(usize::from(seat)).and_then(|s| s.player),
        ) {
            s.player = usize::from(player);
        }
    }
    // Everyone at the other machines, for the HUD's bars and the markers.
    control.remote = m
        .seats
        .iter()
        .enumerate()
        .filter(|&(k, _)| !seats.contains(&(k as u8)))
        .filter_map(|(k, s)| {
            Some(crate::controls::Remote {
                number: k,
                team: usize::from(s.team),
                player: usize::from(s.player?),
            })
        })
        .collect();
    // Sounds and effects go with the prediction; the box score and the
    // play-by-play wait for what the server confirms.
    tick_events.events = events;
    for (_, events) in client.take_confirmed_events() {
        note_events(&mut sim, &mut stats, events, &names, &teams);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_address_gets_the_default_port() {
        assert_eq!(
            resolve("127.0.0.1").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT))
        );
        assert_eq!(
            resolve("127.0.0.1:9000").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], 9000))
        );
    }

    #[test]
    fn the_page_fits_the_main_menus_six_rows_and_hosts_at_home() {
        let mut form = OnlineForm {
            server: " 10.0.0.2 ".into(),
            ..Default::default()
        };
        assert_eq!(
            form.row_list(),
            [
                Row::Host(2),
                Row::Host(6),
                Row::Code,
                Row::Browse,
                Row::Name,
                Row::Spectate
            ]
        );
        assert!(form.help(2).contains("10.0.0.2"));
        form.step(5, 1);
        assert!(form.spectate);
        let join = form.args(false);
        assert_eq!(join.server.as_deref(), Some("10.0.0.2"));
        assert_eq!(join.host, None);
        let host = form.args(true);
        assert_eq!((host.host, host.side), (Some(DEFAULT_PORT), Some(0)));
    }

    #[test]
    fn codes_join_and_a_code_on_this_network_joins_over_it() {
        let public = std::net::SocketAddrV4::new([203, 0, 113, 5].into(), DEFAULT_PORT);
        let code = street_net::code::encode(public);
        assert_eq!(resolve(&code).unwrap(), SocketAddr::V4(public));
        let mut form = OnlineForm {
            server: code.to_lowercase(),
            ..Default::default()
        };
        assert_eq!(
            form.args(false).server.as_deref(),
            Some(&*code.to_lowercase())
        );
        // The host's beacon carries the same code: join it on the network.
        let lan: SocketAddr = ([192, 168, 1, 20], DEFAULT_PORT).into();
        form.lan.push((
            lan,
            street_net::lan::Beacon {
                version: street_net::VERSION,
                port: DEFAULT_PORT,
                people: 1,
                capacity: 2,
                started: false,
                host: "Player".into(),
                match_name: "street".into(),
                code: code.clone(),
            },
        ));
        assert_eq!(form.args(false).server, Some(lan.to_string()));
        // The network's match is a row of its own, between hosting and the
        // code.
        assert_eq!(
            form.row_list()[..4],
            [Row::Host(2), Row::Host(6), Row::Lan(0), Row::Code]
        );
        assert_eq!(form.args_lan(0).unwrap().server, Some(lan.to_string()));
        assert!(form.help(2).contains("Player"));
        assert!(form.row_list().len() <= ROWS);
    }

    #[test]
    fn a_setup_goes_to_the_server_and_back() {
        let net = street_net::MatchSetup::standard(Default::default(), 7);
        let mine = setup::MatchSetup {
            teams: [3, 9],
            lineups: [[1, 2, 3], [4, 5, 6]],
            court: "RUCKER".into(),
            rules: net.world.rules,
            seed: 7,
            demo: false,
            watch: false,
            seats: Vec::new(),
            users: Vec::new(),
            online: true,
        };
        let there = net_setup(&mine, &net.world, [3, 9], &Default::default(), vec![1]);
        assert_eq!(there.roster, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(there.teams, [3, 9]);
        assert_eq!(there.court, "RUCKER");
        assert_eq!(there.roster_data, vec![1]);
    }

    #[test]
    fn an_online_world_plays_the_records_as_they_are() {
        let mut world = street_net::MatchSetup::standard(Default::default(), 1).world;
        let mut record = street_assets::roster_share::blank(3);
        record.ratings = [80; 11];
        record.height_in = 82;
        let lineup = vec![record; 6];
        // The crew's Gym development in the world is left out.
        world.players[0].ratings.dunks = 1.0;
        let w = plain_world(&world, &lineup, &Default::default());
        for p in &w.players {
            assert_eq!(p.ratings.dunks, 80.0 / 99.0);
            assert_eq!(p.ratings.reach, street_sim::player::reach_for_height(82.0));
        }
    }
}
