# Changelog

What changed in each release. The newest is first; the release page shows
its version's part.

## Unreleased

- **The launcher's court:** the MyBaller arena turns behind the launcher
  as you move the mouse, with the MB mark at center court.
- **Links in the launcher:** buttons for myballer.online (with the MB
  mark) and the Discord.
- **The launcher shows its release's version** (it read v0.1.0 in every
  release).
- **Smoother online play.** Your own moves no longer get pulled back when
  your connection or your PC hiccups: the game now sends your controls
  further ahead when it needs to, and less when the connection is steady.
  The host's server runs on its own thread, so a busy host PC no longer
  delays everyone. On a long connection the game adds a little input delay
  (up to a tenth of a second) so the other side has to guess your moves
  less. When a guess about the other players turns out wrong, the players
  and the ball glide to where they belong instead of snapping back and
  forth.
- **Steadier online play through lag spikes.** When your connection
  hiccups, the host now waits a moment for your controls instead of
  guessing them, so a spike no longer eats your moves, and the game no
  longer runs far ahead of the others for the rest of the match because
  of one spike. Other players glide back into place instead of jumping. In tests on
  a 115 ms connection with regular lag spikes, players jumped to new
  spots about twice in 90 seconds instead of more than once a second.
- **Steady online feel, like other sports games.** The input delay now
  covers your whole ping, so the other players' moves arrive before the
  game needs them and they almost never slide or jump. If a packet is
  late, the game corrects quietly instead of freezing. The delay is
  measured from your real ping (not the moment the match loads) and keeps
  adjusting during the match: about 67 ms on a 40 ms connection, 117 ms
  at 115 ms. To trade that steadiness for quicker presses, start the game
  with `--responsive`.
- **Online matches open with their intros,** as matches played alone do:
  the court's intro, then the away side's. Everyone sees the same intros
  at the same time, and anyone pressing Cross skips them for everyone.
  (Online needs everyone on this version.)
- **Fixed: static in the menu music.** The game now plays music and movies
  at the original's levels, measured from the original's own output. The
  menu songs had been about 5.5 dB louder than the original, so any sound
  on top of them clipped. The volume sliders now work like the original's
  (half the slider is half the volume).
- **Fixed: the hiss on hi-hats and cymbals in the music.** Songs, movies
  and sounds are now converted to 48 kHz the way the original's sound
  driver converts them, so the top end sounds like the console. Before,
  the conversion was left to your PC's audio, which adds grit on some
  setups.

## v0.1.4-beta

- **Fixed: controllers on Choose Players in the league challenge and Be A
  Legend.** Entering the mode with a controller left only the keyboard
  able to pick players; the controller that entered the mode now picks.
- **The ball spins:** shots, passes and loose balls turn in the air, and
  the ball rolls on the floor through the moves that carry it there, as
  in the original (it used to hang unturned).
- **Rucker Classic song:** once you own the court, switching its song
  off in Trax plays the next song instead, as in the original.
- **Replays after a rim hang** film from the baseline, as in the
  original.
- **Passes to a player in the air or posting up** are aimed where the
  original aims them.
- **Box-outs** hold the boxed player where the original's animation puts
  him.
- **Per-player dunks:** a legend's turbo-combo dunk is his own signature
  dunk and a created player's are the ones he equipped, as in the
  original (everyone used to dunk the same set).
- **Open shots:** a defender on the floor or in his counter stance no
  longer counts as guarding the shooter, so the shot keeps its open-shot
  bonus, as in the original.
- **Per-player jump shots:** women shoot their own set shots, one legend
  his own jumper, the legends with signature hooks throw them off a side
  fade, and created players' turbo shots play the shots they equipped,
  as in the original (everyone used to shoot the same men's set).
- **Passout pop-ups** show the points the passout scored, as in the
  original: 5,000 off a defender's head, 3,000 off the board, less when
  repeated (they always read 3,000).
- **The swing sound** plays when a blocker's hands first miss the ball
  and when a swipe knocks it loose, as in the original, not at every
  block jump's take-off.
- **The pick call:** when the computer sets a pick and closes on the
  defender, a player on the attacking team calls it out, as in the
  original.
- **Street School narration:** the School's narrator introduces each
  lesson, reads the tutorial pages and reacts to each pass or fail, as in
  the original. The School had no voice-over before.
- **Missing dunk calls:** four sets of the commentator's dunk calls and one
  special introduction never played (their lines were read from the wrong
  place in the disc's files). They play now.
- **Score calls:** the commentator's score updates now play for every
  margin between the teams; all but six of them were silent.
- **Gamebreaker sounds:** a Level 2 gamebreaker opens with its own
  transition instead of the Level 1 loop, and a gamebreaker's basket gets
  the crowd's roar, as in the original.
- **Litter on the courts** (leaves and trash) moves the way the original's
  does: leaves lie still, then a gust lifts them and they tumble back down;
  trash rolls along with the wind; pieces blown away come back in out of
  the camera's sight.
- **Steam on the Philly court** rises and fades the way the original's
  does, and leans with the court's wind; one vent's steam no longer shoots
  sideways ten times too fast.
- **Breath on the Foster court** comes out the way the original's does:
  every two to five seconds each player breathes out a small cloud from
  his mouth that follows his head, then drifts off on the wind.
- **The rim's fireworks** go off on the basket that wins the game (or any
  basket with Explosive Rims), as in the original: a burst of blue and
  grey sparks that bounce off the floor and backboard, with a moment of
  flames. Before, a steady fire burned after every gamebreaker basket.

## v0.1.3-beta

- **Controllers in matches (issue #1):** playing alone, every controller
  now drives your player in a match, as in the menus. Before, a match
  listened only to the controller that confirmed the setup screens, so an
  Xbox pad did nothing if the keyboard had been used there (or the pad
  reconnected). With several people each keeps their own controller; a
  pad that reconnects takes its seat back.
- **Choose Sides:** the controller that started the game takes the first
  slot (1P), as in the original. An Xbox pad used alone no longer shows up
  in the player 2 slot behind the keyboard.
- **Song changes:** after four minutes of a song the music moves on as a
  restart's inbounder starts his walk, as in the original, not at any
  catch in the middle of play.
- **Ball and rim sounds match the original:** the ball off the backboard,
  rim, floor and through the net now sounds by how hard it hits; a dunker
  grabbing and letting go of the rim, a dribble's bounce and a slam's shake
  have their own sounds; every catch, steal, rebound and pickup plays the
  take sound, a ball knocked loose its knock, and a player's head into
  the backboard its thud. Gone: the clank the rewrite played on every
  miss and the wrong "swish" on made jumpers.
- **Hanging on the rim:** a dunker's hands turn to grip the rim, and
  follow it as it gives, as in the original.
- **Dunk and passout sounds:** dunks no longer play a passout's sound, the
  commentator's passout lines or an extra crowd cheer; an off-the-heezay
  pass bouncing off the board or a defender's head now plays them, as in
  the original. The dunk's crowd roar now scales with the dunk's finish,
  and the game-winning basket plays the win or loss sting.
- **Trick sounds on time:** a trick's whoosh, the signature players' own
  trick sounds and the music sting for the biggest tricks now play as the
  move starts, not when it ends.
- **Trick and dunk pop-ups:** a dunk no longer pops up the last trick's
  name as it leaves the hands. A trick's name shows in crowns only when it
  faked a defender out, and only those tricks get the crowd's gasp, the
  commentator and the player's taunt, as in the original.
- **Passout names:** the pop-up comes when the passout is caught, not when
  it's thrown, and uses all four of the original's names: Back 2 Papa,
  Back at Ya (to a teammate off the board), Off the Heezay and Downtown
  Rebound (an alley-oop off the board).
- **Busted rim:** a game-winning dunk or alley-oop now shows the
  original's "Busted Rim" pop-up, and the profile keeps count of busted
  rims as the original does (it isn't shown on screen there either).
- **Advanced alley-oops** are named by the finisher's dunk in the pop-up,
  as in the original.
- **Replays:** the rim's give plays smoothly in slow motion.
- **Alley-oop music:** the music stays down from the lob to the slam
  instead of coming back at the catch.

## v0.1.2-beta

- **Ultrawide screens (21:9, 32:9):** the court widens to fill the screen
  instead of the picture stretching; the menus and the in-game scoreboard
  keep their shape in the middle. Ordinary and maximized windows fill the
  window as before.
- **Smooth Motion:** on 120 and 144 Hz screens the court is drawn between
  the game's 60 updates a second. Gameplay, online play and replays are
  unchanged. On by default; Options → Adjust Screen → Smooth Motion.
- **Big dunks drop the music:** as in the original, a power dunk or an
  alley-oop pass silences the music from the take-off until the slam.
- **Sounds now match the original:** the "no turbo" buzz, the pass sound,
  the dunk and alley-oop take-off sounds, the block sweetener and the
  signature players' own sounds; jump shots no longer grunt, and blocks and
  steals lost sounds the original doesn't play.
- **PC Settings:** the project notice no longer overlaps the last row.
- README: a "Made for PC" section and Vj on X/Twitter (@UnLuckvj).

## v0.1.1-beta

- **Online:** the host's join code now shows in the lobby (it was hidden),
  with a note when the router can't open the game and the address on your
  own network.

## v0.1.0-beta

- First public beta: the one-file Windows download, the launcher, and the
  getting-started guide.
