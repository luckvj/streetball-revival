# Player editor: the contract between street-rs and the launcher's builder

Board row F1. The user wants (2026-10-14): every character from the original game
(Vol. 1), Vol. 2 and V3, including V3 GameCube's Nintendo characters, editable
in the MyBaller launcher's builder with 3D models, and playable in
street-game; the "Legendary Edition" roster mod (updated league rosters and HD
textures for the GameCube Vol. 2) in the rewrite; a builder that is "highly
configurable", looks like Vol. 2's own Create A Baller, and lets a player
take another character's animations.

Nothing here ships game data. Everything below the "export" folder is made
on the player's machine from their own discs and mods.

## Pieces and who reads what

| Piece | Where | Makes | Reads |
|---|---|---|---|
| Roster override | `street-assets::db::set_roster_override` | the Legendary tables in place of the disc's | `xdbplyr.adf`, `xdbteam.adf` (either byte order) |
| HD textures | `street-assets::hires` + a hook in `street-render` | PS2 texture → PNG replacements | a Dolphin pack (`Textures/GNZE69`) |
| GameCube characters | `street-assets` (`gcdisc`, `gsh`, `eagl_gc`, ...) | V3 and Vol. 1 characters as meshes + skeletons | the player's `.rvz`/`.ciso`/`.iso` |
| Exporter | new crate `street-export` (binary `street-export`) | the export folder below | discs, roster mod, texture pack |
| Builder | MyBaller `web/launcher` + server routes | `roster.json` | the export folder |
| Game | `street-game --roster <roster.json>` | plays the edited roster | `roster.json`, discs, roster mod, texture pack |

## Character ids

`<game>:<key>`, stable across runs:

- `vol2:<record>`: a record of Vol. 2's player table (0..239), whichever
  roster (disc or mod) is active. The roster is named in the catalog.
- `v3gc:<name>`: a V3 GameCube character by its database name, lower case
  (`v3gc:mario`, `v3gc:luigi`, `v3gc:peach`, then V3's whole roster).
- `v1gc:<name>`: a Vol. 1 GameCube character, same rule.
- `created:<n>`: a created baller from a street-rs save (`save-info`).

## The export folder

`street-export characters --out <dir> --vol2 <iso> [--v3 <rvz>] [--v1 <ciso>]
[--roster-db <dir>] [--hires <Textures/GNZE69>] [--gc-vol2 <ciso>]` writes:

```
<dir>/catalog.json
<dir>/models/<game>/<key>.glb       one skinned, textured character each
<dir>/anims/<game>/<set>.glb        animation libraries (clips only + skeleton)
<dir>/ui/...                        Vol. 2 front-end art for the builder
<dir>/portraits/<game>/<key>.png    head shots (XAPORTS for Vol. 2)
```

Models: glTF 2.0 binary, Y up, metres (game units scaled so a 78 in player
is 1.98 m), one skin, joints named with the game's own bone names so clips
from any library play on any character that shares the names. Every model,
V3's and the original game's included, is on Vol. 2's skeleton (see "GameCube
games" below), so every library plays on every model. Materials
unlit-friendly (base colour texture, alpha mask where the game uses it).
Hidden groups (blink lids, unused accessories) are left out, not hidden.
`extras` on the scene: `{ "id", "game", "heightIn", "skeleton" }`.

Animation libraries: each holds the skeleton and many named
`AnimationClip`s. Clip names are the game's own (`createplayer_stand`,
script/move names). The catalog says which clips a move or slot uses.

`catalog.json` (camelCase; arrays in game order):

```json
{
  "version": 1,
  "generated": "2026-10-14T00:00:00Z",
  "roster": "legendary",            // or "original"
  "hires": true,
  "games": [{ "id": "vol2", "title": "the original game" }],
  "teams": [{ "id": "vol2:0", "game": "vol2", "index": 0, "name": "Atlanta",
              "city": "Atlanta", "nickname": "Hawks", "roster": ["vol2:2"],
              "logo": "ui/logos/0.png" }],
  "moves": [{ "game": "vol2", "row": 0, "name": "Rope-a-Dope",
              "kind": "layup|dunk|trick", "level": 2, "cost": 50,
              "moveId": 12, "clips": ["..."], "anims": "anims/vol2/moves.glb" }],
  "animationSets": [{ "id": "vol2:158", "game": "vol2", "label": "Julius Erving",
                      "skeleton": "vol2", "file": "anims/vol2/base.glb",
                      "clips": { "idle": "...", "walk": "...", "run": "...",
                                 "shot": "...", "celebrate": ["..."] } }],
  "characters": [{
    "id": "vol2:0", "game": "vol2", "record": 0, "kind": "league",
    "name": "Michael Redd", "firstName": "Michael", "lastName": "Redd",
    "team": "vol2:14", "model": "models/vol2/0.glb",
    "portrait": "portraits/vol2/0.png", "skeleton": "vol2",
    "fields": { "position": 3, "jersey": 22, "heightIn": 78, "weightLb": 215,
                "ratings": [/* 11, RATING_NAMES order */], "skinTone": 2,
                "leftHanded": true, "style": 0, "star": false, "build": 1,
                "hairStyle": 0, "hairColor": 0, "facialHairStyle": -1,
                "facialHairColor": 0, "bodyScale": [0, 0, 0],
                "accessories": [/* 22 */], "shoes": 0, "clothing": 0, "look": 0 },
    "raw": [/* vol2 only: all 101 fields */],
    "moves": { "layups": [/* 6 XDBMOVE rows, -1 empty */],
               "dunks": [/* 11 */], "tricks": [/* 11 */] },
    "animationSet": "vol2:0"
  }],
  "options": {
    "ratings": ["shots", "threes", "dunks", "rebounds", "power", "blocks",
                "steals", "handles", "speed", "rating9", "rating10"],
    "positions": ["C", "PF", "SF", "SG", "PG"],
    "accessorySlots": [{ "name": "headband", "choices": 4, "thumbs": "ui/acc/headband/" }]
  }
}
```

Fields not known for a game are left out. `raw` lets the builder edit
fields that have no name yet ("Advanced"), so nothing the table holds is
out of reach.

### What `street-export` writes on top of the sketch

- Libraries: `anims/vol2/base.glb` (the match roles, `createplayer_stand`,
  celebrations, outros and every gait clip) and `anims/vol2/moves.glb`
  (every clip a move script plays); `--all-anims` adds each bank whole as
  `anims/vol2/<bank>.glb`. A clip named `<name>@mirror` is the game's
  mirrored (left-handed) twin of `<name>`, baked.
- `clips`: `{ "<clip>": { "file", "bank", "frames", "seconds" } }`, so any
  clip name resolves to its library.
- `scripts`: `{ "layups" | "dunks" | "tricks": [{ "id", "name", "clips" }] }`,
  the game's move scripts by native id. A character's `moves` lists
  `XDBMOVE` rows per slot (the equipped row, else the row that buys the
  script the slot plays by default, else `-1`); `moveScripts` lists the
  script id each slot actually plays (equipped row, the legends' own slot-1
  override, else the native default), so a slot with no buyable row still
  previews.
- `moves[].script`: the script id the row plays (`0x45f9c8`).
- `animationSets[]` carry `clips` by role (`stand`, `idle`, `walk`, `jog`,
  `run`, `walkBall`, ..., `shot`, `dunk`, `celebrate`, `outro`), `gaits`
  (every walk/jog/run the game's selector offers that character, its own
  first) and `own` (clips made for that player, the legends' walks).
- `skeletons.vol2`: joint names, parents, `metresPerUnit`, `fps`.
- `options` also carries Create A Baller's builds (sizes, outfits, prices,
  kit limits), skills, skill bar levels and prices, and move slot counts;
  `ui` names `ui/manifest.json`, whose `named` map gives semantic keys
  (`backdrop.createBaller`, `font.main`, `widget.star`, `layout.createBaller`,
  ...); `warnings` lists anything left out.
- Character extras on the glTF scene also give `record`, `name`,
  `modelHeightIn` (GameCube: also `sourceSkeleton`) and the optional
  `groups` shown. The armature node holds
  the uniform scale and a half turn (the game's models face -z); the root
  joint's rest translation stands the body on the floor, matching every
  clip's root track.

### GameCube games (`--v3`, `--v1`)

`street-export characters --v3 <Street V3 disc> --v1 <the original game disc>`
(any image the `nod` crate reads: `.rvz`, `.ciso`, `.iso`, ...) adds every
character of both games: 226 from V3 (league players, legends, street
legends, the Nintendo All-Stars Mario, Luigi and Peach, the Beastie Boys,
generic ballers, create-a-player presets) and 175 from the original game.
`--only v3gc:mario,v1gc:shaquille_o_neal` exports just those (the
libraries are always written). A full run with both discs takes about 12 s
in a release build (V3 5 s, the original game 2 s) and writes about 500 MB:
V3 models 138 MB, the original game models 82 MB, their libraries 42 MB and 14 MB.

- **One skeleton for everyone** (`street_assets::canonical`). Each
  character is assembled from its disc (`gc_character`) and rebound onto
  Vol. 2's 43-bone skeleton: Vol. 2's joint names, parents and local rest
  rotations, so a Vol. 2 clip's rotation means the same on it; the
  character's own bone positions (the body scaled so its hips stand at
  Vol. 2's 4.05 units over the soles). A body bone the character lacks
  (Mario's and Luigi's collarbones, lower and mid torso, head, toes) sits
  between its neighbours and carries no weight; weights on bones Vol. 2
  lacks (V3's face, shirt, shorts and necklace bones) move to the nearest
  body bone. The rest pose draws exactly the character's bind pose (every
  game's rigs are T poses facing the same way). The glTF is then laid out
  like a Vol. 2 player's: root lifted onto the floor and half turned,
  armature half turned and scaled so the crown is at `modelHeightIn`.
- **Heights**: `modelHeightIn` is the table height, except for V3's
  Nintendo stars, whose bodies are not the stock one: table height times
  their model's height over a stock body's (Mario 75 in → 55 in, Luigi
  67 in, Peach 63 in), which is how big V3 draws them next to the others.
- **Libraries**, every clip baked onto Vol. 2's skeleton through
  `retarget` (each bone takes its source's model-space rotation relative
  to rest) with root motion scaled by hip height, named as the game names
  it, 30 fps, 22 body rotation tracks and the root's translation:
  `anims/v3gc/base.glb` (V3's front-end, in-game and second in-game banks,
  920 clips; a name in two banks keeps the first), `anims/v3gc/nintendo.glb`
  (`anibtrex.bin`, 836 clips: the Nintendo rig's versions of the in-game
  set, under the same names), `anims/v1gc/base.glb` (`anibot.ani`, 614,
  upper-case names such as `JUMPSHOT_R_3`) and `anims/v1gc/front.glb`
  (`anifront.ani`, 38, `front_<n>`). `--all-anims` adds V3's intro and
  judges banks (`intro.glb`, `judges.glb`). Face, cloth and prop rigs are
  left out. Any clip of any game plays on any model.
- **Catalog**: `games` gains `{ "id": "v3gc" | "v1gc", "title",
  "platform": "gamecube", "disc" }`; `teams` gains `v3gc:<team id>` /
  `v1gc:<row>` (name, city, nickname, roster); `characters` entries carry
  `game`, `record`, `kind` (`league`, `legend`, `street`, `nintendo` for the
  Nintendo All-Stars, `guest` for the other guests, `generic`, `preset`),
  names and `nickname`, `team`, `model`, `portrait`, `skeleton: "vol2"`,
  `sourceSkeleton` (`v3gc`, `nintendo`, `v1gc`), `fields` (`playerId`,
  `position`, `jersey`, `heightIn`, `weightLb`, `modelHeightIn`,
  `female`, `kid`, `ratings` as the game stores them), `raw` (every
  field) and `strings` (string fields by number). `animationSets` get one
  per character on `skeleton: "vol2"` with role clips from its game's
  library (the Nintendo All-Stars' from `nintendo.glb`). `clips` indexes
  GameCube clips as `v3gc:<name>`, `v3gc:nintendo:<name>`, `v1gc:<name>`.
- **Portraits**: neither disc's portrait art is decoded yet, so each
  character gets a 128×128 head shot rendered from its bind pose
  (`portraits/<game>/<key>.png`).
- Not carried over: V3's jersey numbers and names (drawn from TrueType
  fonts at run time), morph targets (body types, created faces),
  accessories on the original game players.

## roster.json (the builder writes, street-game reads)

```json
{
  "version": 1,
  "base": "legendary",              // "original" | "legendary"
  "players": [{
    "slot": 0,                      // Vol. 2 record it replaces; null = added
    "character": "vol2:0",          // whose data the edits start from
    "team": "vol2:14",              // for added players (slot null)
    "model": "v3gc:mario",          // optional: wear another character's model
    "edits": { "firstName": "Michael", "ratings": [/* 11 */], "...": 0,
               "raw": { "33": 2 } },  // only the fields that changed
    "animations": {
      "from": "vol2:158",           // take all of this character's animations
      "idle": "vol2:145", "gait": "vol2:12", "shot": "vol2:56",
      "celebrate": "vol2:140",      // per-category sources, override "from"
      "moves": { "layups": [/* rows */], "dunks": [], "tricks": [] }
    }
  }]
}
```

Rules: a field missing from `edits` keeps the character's value; `raw` keys
are field numbers. `animations.moves` lists are XDBMOVE rows by slot (6, 11,
11), `-1` for an empty slot; a list that is missing keeps the source's.
In game, animation sources must share a playable skeleton with the
simulation (Vol. 2's); V3 and Vol. 1 sets may be preview-only in the builder
and fall back to the character's base Vol. 2 set in game, which the builder
says next to the choice.

## Saves (`save-info`)

`street-export save-info <street_save.dat>` prints JSON:
`{ "path", "modified", "users": [...], "ballers": [{ "id": "created:0",
"name", "fields": {...}, "moves": {...}, "dp": 0 }], "progress": {...} }`
using street-sim's public save decoding only.

## In game

```
street-game --roster-db "<Legendary Edition>" [--roster roster.json] [--hires <dir>]
```

Both work at the database layer, set before the disc loads, so the menus,
team select, Create Team, the story and every match see them:

- `--roster-db <dir>` (or `STREET_ROSTER_DB`): the roster mod's
  `xdbplyr.adf` / `xdbteam.adf` (the folder itself, or its `database` or
  `ISO Files/database`) replace the disc's
  (`street_assets::db::set_roster_override`).
- `--roster <roster.json>` (or `STREET_ROSTER`): `street_assets::roster_edits`
  applies it inside every `db::players` / `db::teams`, over the active
  roster. A `base` of `legendary` needs `--roster-db`; `original` refuses it.
  - `edits`: the catalog's named fields (`firstName`, `lastName`,
    `position`, `jersey`, `heightIn`, `weightLb`, `ratings`, `skinTone`,
    `leftHanded`, `style`, `star`, `build`, `hairStyle`, `hairColor`,
    `facialHairStyle`, `facialHairColor`, `bodyScale`, `accessories`,
    `shoes`, `clothing`, `look`, plus `id`, `team`, `female`, `portrait`,
    `modelHeightIn`, `modelWeightLb`, `code`) and `raw` by field number;
    named fields win over `raw`. A new height or weight redraws the model
    unless the model's own are given. Unknown keys are reported and skipped.
  - `slot`: the record replaced (the player stays on the slot's team unless
    the edits move it). `slot: null` appends a record from 244 on (after
    the career stars' records), on `team` if given.
  - Three or more players make a "Custom Roster" team of everyone the file
    names, after the database's teams (`--teams Custom,Lakers`), so added
    players are always reachable.
  - `character` `created:<n>` starts from the save's created baller `n`;
    GameCube characters start from the create-a-player record with their
    name, position, jersey, height, weight and sex from the disc's table
    when the disc is given (ratings stay the record's: the scales differ).
  - `model`: `vol2:<record>` draws that record's model and sheets, at the
    player's own height and weight (a worn model is an appearance, never a
    hidden giant; a league model wears the player's league team's uniform); without it the
    player is drawn as its `character` (`roster_edits::look_for`, called by
    `street_assets::player::look`). A `v3gc:`/`v1gc:` model, or a
    GameCube `character` with no `model`, is kept in
    `roster_edits::gamecube_model(record)` and drawn from the disc (below);
    without the disc the player draws its Vol. 2 stand-in (reported).
- `--hires <dir>` (or `STREET_HIRES`) is accepted and reported; the texture
  pack is not wired in yet.
- `--v3 <disc>` / `--v1 <disc>` (or `STREET_V3_GC`, `STREET_V1_GC`): the
  GameCube discs (`street_assets::gc_discs`). `street-render`'s
  `PlayerAssets::load` draws a player that `gamecube_model` names as that
  character on Vol. 2's skeleton (`canonical::rebind`, the same rebind the
  exporter uses), with its own textures (V3's away look for the road
  side), sized to `modelHeightIn` scaled by the record's model height over
  the table height (so a height edit still applies). Vol. 2's animations
  then drive it bone for bone, mirrored clips included; the ball hand and
  mouth are its own `RHand` and `jaw`. A missing disc or a character that
  fails to load logs a line and keeps the Vol. 2 stand-in.

Animations (each source a `vol2:` record or `created:<n>`; a missing one is
`from`, then the player's `character`):

| Category | In game |
|---|---|
| `from` (dunks, dribble moves) | honoured: the native legends' slot-1 signature dunk and trick (keyed on the record in `clip::move_script_id`), a created source's equipped dunk and trick rows, and the trick ladder the simulation latches from them |
| `shot` | honoured: the lay-up / shot slots (`Kind::Shot`) use a created source's equipped shot rows, else the shared defaults |
| `moves.layups/dunks/tricks` | honoured: the equipped rows slot by slot, over the source's (a row of the wrong kind plays the slot's default) |
| `gait` | worked out (`roster_edits::gait_source`: the source's record as the gait records' player restriction, its weight and sex for the category) but not yet used: `setup.rs` builds the gait profile from the player's own record |
| `idle`, `celebrate` | not per player in Vol. 2 (one stand, a shared celebration table): accepted and reported, no effect |

`leftHanded` is stored like any edit, but nothing in the rewrite's animation
choice reads it yet. V3 and Vol. 1 animation sources fall back to the
character's own Vol. 2 set (reported).

## Online

The match creator's roster is the match's roster. Nobody needs the
creator's mod files or `roster.json`: the roster travels as data.

- **What travels** (`street_assets::roster_share`): the creator's tables as
  the database layer gives them (the disc's, or a roster mod's, with
  `roster.json` applied), as differences from the disc's own tables. Each
  changed record is "source record + the fields that differ" (the source
  is the record itself, or its `roster.json` character); changed or added
  teams go whole; then what is not a table value, by record: each roster
  player's look (`Look::As` / a worn model as data), animation sources
  (`AnimProfile`), GameCube model names, and the created ballers' equipped
  rows that animation sources use. The save's own created ballers in the
  line-up and created teams travel as before (`NetLooks` in `extra`). A
  match setup also carries the line-up's six records whole, so a server
  without the disc knows the values the world must hold.
- **Where**: `street_net::MatchSetup::roster_data` (protocol version 10),
  a `MatchRoster` deflated, at most 60 KiB (the setup's message is split
  into parts as before). The Legendary Edition is 177 changed records and
  30 teams: 12 KB, 7.3 KB packed (a whole setup is about 130 KB in 110
  datagrams, mostly the staged gamebreaker data; `UdpTransport` asks for
  4 MiB socket buffers, since Windows' default 64 KiB dropped such a burst
  and a join with a setup never arrived). An edited roster carries at most
  64 added players (records 244..307) and one added team.
- **Joining** (`street-game` `roster.rs`, `play_shared`): the joiner checks
  the roster and the world against it with its own disc
  (`street_net::roster::check_setup`), then plays it for the match:
  `roster_edits::set_shared` makes every `db::players` / `db::teams` the
  creator's tables over the joiner's disc, in place of its own mod and
  `roster.json`, and the tables are re-read. Names, team lists, ratings,
  moves and animation choices are the creator's. What the joiner lacks to
  draw (an HD pack, GameCube models, other discs) falls back locally; none
  of it reaches the simulation. A match without roster data plays the
  disc's own tables, whatever the joiner has. Back at the menus
  (`leave_shared`) the joiner's own roster, created ballers and teams
  return.
- **Hosting**: a game whose roster breaks the rules does not host or
  create a match (the message names the first problems; `street-game
  --host` exits with it). Offline anything goes. The creator's own world is
  checked against what it sends before it goes out.
- **The world**: the simulation's world travels in the setup, so every
  machine runs the same numbers. An online world plays the records as they
  are: the story crew's Gym development is left out (`online.rs`,
  `plain_world`); the cheat codes, which set every player's speed or long
  range alike, stay.
- **Fingerprint**: `SharedRoster::fingerprint` (SHA-256 of the shared
  roster's canonical bytes): the same roster gives the same fingerprint on
  any machine. `check-roster` prints it.

Verified with two games, the creator on the Legendary Edition and a
`roster.json` (edits and an added player), the joiner with no roster flags:
the `game: tick N:` log lines match with the tick stripped, and the joiner
logs the creator's player names. Also through `street-server`.

## Rules

`street_assets::roster_rules` is the one source of truth for what a roster
must satisfy to play online. It is enforced in three places:

1. `street-game` will not host an illegal roster, and joiners re-check what
   they are sent (roster and world).
2. `street-server` (and the server a hosting game runs) checks the roster
   data and the world in every match setup and refuses an illegal one
   (`Refusal::Roster`, logged with the reasons): the real anti-cheat, since
   a client can be modified. Without the disc it checks what the data
   shows: only fields a roster may change differ from their source, each
   within its bounds, move lists, counts and team lists, and the line-up's
   six records in full (sizes, the rating-total cap, Create A Baller's
   limits). With `--vol2 <iso>` it also rebuilds every changed record over
   the disc's tables, checks each whole, and requires the line-up to be the
   rebuilt roster's own records. Either way the world must hold exactly the
   line-up's values: each player's ratings (record / 99), reach (from the
   height), gait profile (weight, sex, record, handles) and equipped
   dribble moves (the roster's rows where it declares them; rows of the
   right kind and level always).

   A roster built on a roster mod names it by the mod's id (SHA-256 of
   its parsed tables, `db::roster_mod_id`). A server accepts it only when
   it holds that mod, and checks the rating-total cap against the mod's own
   records; any other mod is refused (`Refusal::RosterMod`; the server logs
   "this server doesn't have the <id> roster mod", the game says so too)
   unless the server runs with `--trust-roster-mods` (off by default), when
   a record over the cap is taken to be the mod's own. Disc-based rosters
   need nothing but the server (and `--vol2` for the full checks).
3. `street-export check-roster` tells the launcher before it starts the
   game, and the catalog's `rules` object gives the builder the bounds to
   clamp its sliders to.

| Rule | Bound | Evidence (disc and Legendary Edition records) |
|---|---|---|
| height | 63..90 in | Mugsy Bogues (Legendary #5) 63, Biggie Little (#147) 64; Yao Ming (#42) 89 on the disc, 90 in Legendary |
| weight | 90..338 lb | Biggie Little 90; Shaquille O'Neal (#55) 338 on the disc |
| weight for height | 1.40..3.97 lb per inch | Biggie Little 90/64; O'Neal 338/85 |
| model height | shown height -2..+2 | 0 on the disc; Legendary Joel Embiid -2, Donovan Mitchell +2 |
| model weight | shown weight +0..+30 | the disc draws celebrities 30 lb heavier; Legendary up to +30 |
| body scale (fields 43, 44, 45) | 0..18, 0..15, 0..22 | Keith Van Horn / Luka Doncic (#101) 18; O'Neal 15 and 22 |
| each rating | 0..99 | the game clamps every rating read to 0..99 (`0x1d7d00`) |
| rating total (shots, threes, dunks, rebounds, power, blocks, steals, handles, speed) | at most the cap: the best real league player's, legend's or streetballer's (records 0..183) in the base roster. A record no better than its own unedited base record, rating by rating, passes too (so it stays legal exactly as the base has it, and may be edited in anything but upward ratings) | disc: Bonafide 698; Legendary: Wes Unseld 794. Legendary's generic record #231 (Gianna Bryant, all 99s, 891) plays as the mod has her; nobody can be edited up to her, and editing her is held to 794 or to no rating above hers |
| build | 0..5 | league records 0..2, created 0..5 |
| created ballers (records 234..243, `created:<n>`) | Create A Baller: each skill at most 98, long range 90, speed 85, total 861, height and weight within the build's sizes. GameCube characters (`v3gc:`, `v1gc:`; they start from the create-a-player record with their own disc's name and size, travel online by id and draw the Vol. 2 stand-in on a machine without that disc) keep Create A Baller's rating limits and the general size bounds | `street_sim::cab` (`SKILL_LEVELS`, `SIZES`); speed by build `[85, 75, 65, 55, 80, 70]` |
| moves | lay-up slots: lay-up rows; dunk and dribble-move slots: rows of their kind at the slot's level (slot 0 level 4, 1..4 level 3, 5..10 level 2); a row once per kind | `XDBMOVE` (rows 0..9 lay-ups, 10..53 dunks, 54..92 dribble moves, grouped by level), the slot classes at `0x43d408`, Edit Moves' categories |
| fields | cosmetic fields change freely; bounded ones within the bounds; move fields (68..73, 79..100) as moves; fields 31, 33..35, 37 and 74..78 must equal the source record's | what the rewrite's simulation reads (`setup.rs`); Legendary itself changes only names, ids, team, position, jersey, skin, sizes, portrait, ratings, style, star and code |
| counts | at most 64 added players, 304 `roster.json` entries, 50 teams; team lists name existing records once | |

Every disc record is a legal player under these rules, and the Legendary
Edition with no edits is a legal roster (disc-gated tests in
`roster_rules`).

### Running a server for mod rosters (MyBaller operators)

`street-server --vol2 <iso> --roster-db <mod folder> [--roster-db <another>]`
accepts rosters built on the disc and on each mod given (the folder, or the
one holding `xdbplyr.adf`; the Legendary Edition's `ISO Files/database` is
found from its top folder). It prints the id of each mod it accepts. In
the MyBaller launcher's `server.json`, the street server's arguments carry
them:

```json
"street": { "args": ["--port", "7375", "--vol2", "/srv/street/vol2.iso",
                     "--roster-db", "/srv/street/rosters/legendary"] }
```

Without `--roster-db`, a match built on a mod is refused; `--trust-roster-mods`
accepts any mod unchecked (only for servers among friends). A game hosting
a match runs the same server, holding its own mod and no other.

`street-export check-roster <roster.json> --vol2 <iso> [--roster-db <dir>]
[--save <street_save.dat>]` prints

```json
{"version": 1, "legal": false, "fingerprint": "<64 hex digits>",
 "ratingTotalCap": 794,
 "players": [{"player": 0, "record": 0, "character": "vol2:0",
              "total": 640, "baseTotal": 612}],
 "problems": [{"player": 0, "record": 0, "character": "vol2:0",
               "field": "heightIn", "value": 96, "allowed": [63, 90],
               "message": "height 96 is outside 63..90"}]}
```

and exits 0 when legal, 2 when not, 1 on an error (no disc, a file that
does not parse or apply, a `base` the flags do not match). `player` is the
index in `roster.json`'s `players` (`null` for a problem of the whole
roster), `record` the record in the effective table; `field` is the
catalog's name (`heightIn`, `ratings[2]`, `bodyScale[0]`,
`moves.dunks[3]`, `raw.33`, `ratings` for the total); `allowed` is the
range when the rule is one, else `null` (for the total, `[0, max(cap,
baseTotal)]`). `players` gives each `roster.json` player's total and its
starting character's total in the base roster. It checks each `roster.json`
player against its `character`'s record, and the roster as a match would
share it.

The catalog's `rules` object:

```json
"rules": { "version": 1, "heightIn": [63, 90], "weightLb": [90, 338],
  "lbPerInch": [1.4, 3.97], "modelHeightOffset": [-2, 2],
  "modelWeightOffset": [0, 30], "bodyScale": [[0, 18], [0, 15], [0, 22]],
  "build": [0, 5], "rating": [0, 99],
  "ratingTotalCap": 794,
  "ratingTotal": { "ratings": 9, "cap": 794, "capOriginal": 698, "capLegendary": 794,
                   "capRecords": [0, 183], "rule": "..." },
  "created": { "records": [234, 243], "ratingCaps": [98, 90, 98, 98, 98, 98, 98, 98, 85, 99, 99],
               "totalCap": 861, "sizes": [{ "heightIn": [69, 81], "weightLb": [200, 260] }] },
  "moves": { "slots": { "layups": 6, "dunks": 11, "tricks": 11 },
             "slotLevels": [4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2],
             "rows": [{ "kind": "layups", "level": 2, "rows": [0, 9] }], "unique": true },
  "editableOnline": [0, 1, 2, "..."],
  "fields": { "cosmetic": [], "bounded": [], "moves": [[68, 73], [79, 100]], "fixed": [31, 33, 34, 35, 37, 74, 75, 76, 77, 78] },
  "fieldNames": ["id", "lastName", "firstName", "team", "..."],
  "maxAddedPlayers": 64, "maxPlayers": 304, "maxRecords": 308, "maxTeams": 50 }
```

`ratingTotalCap` is the exported roster's own (698 on the disc, 794 with
Legendary); each catalog character also carries `baseTotal`, its own total:
a builder lets an unedited character keep it, and never lets a rating rise
past the base's while the total is over the cap.
