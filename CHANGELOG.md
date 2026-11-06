# Changelog

What changed in each release. The newest is first; the release page shows
its version's part.

## Unreleased

- Players turn as on the PS2 when a new animation starts: a player
  settling into his stance or starting to walk takes the new clip's
  facing at once (and pivots on his planted foot), instead of turning
  slowly the short way round. A defender after the opening tip used to
  turn the wrong way and drift out of place; he now follows the PS2's
  path within a few hundredths of a unit.
- Each side's three take their places as on the PS2: the tallest of the
  centres jumps, the best ball handler of the guards plays the third
  slot, the other the second (the rewrite kept the line-up's order, so a
  team like Minnesota or Indiana had its forward and guard swapped, with
  each other's spots, sizes and ratings).
- The tip-off jump moves as on the PS2, checked against the PS2's own
  positions update by update: the run-up carries the jumpers forward on
  the clip, the take-off comes one update later, the aim faces down the
  court, the take-off settles the body, and after landing the clip's
  play-out slides them on. Both jumpers now follow the PS2 within 0.03
  from the run-up to their first steps afterwards.
- A stand that replaces another blends from the clip the player was
  showing, not from his walk.
- The tip-off comes in the PS2's update: the jumper's hands are tested
  as the last update posed them (the rewrite tipped one update early in
  all three captured openings). The loser is barred from the ball for the
  rest of the toss's fall and the taker for 70 updates, and the jumpers
  stay on their tip scripts until the clip plays out after landing.
- The players around the tip stand as on the PS2: the sixth of the six
  stands is the placement script (clip 65, swaying on its root) rather
  than no stand, and the warm-up, kick and stretch don't plant a foot
  during the opening, so they hold still through their first blend.
- A player's stand and gait choices come before his catch and steal
  tests in his update, as on the PS2 (the tests still see last update's
  pose), and a standing catch row keeps its catcher still until it ends.
- The fast break's spot past half court is the paint spot by the row's
  own deep flag, without the extra near/far coin.

- The camera's random placements (the courtside and baseline views, the
  demo's cuts) take their random numbers from the game's own, as on the
  PS2, so the players' choices after a camera cut no longer drift from
  the original's.
  A gamebreaker's camera and the shot-clock violation's take theirs at
  the moment it happens, inside the game's update, as on the PS2.

- The crowd's reactions pick their spectators at the moment of the
  basket, block or steal, inside the game's update as on the PS2, so the
  random numbers the players draw after them in the same update match
  the original's. After a basket the players' faces now draw before the
  instant replay's roll, in the PS2's order.

- The court camera no longer bobs up and down following a loose ball
  bouncing on the floor, as on the PS2.

- The shot clock runs at the PS2's speed: its 24 seconds tick down in 16
  real seconds, one and a half times as fast as before.
- The defence's "21 on the clock" call now takes its chance from the
  game's own random numbers at the PS2's moment, so the court's litter
  and other effects that share them follow the original's.

- After a pass goes loose, players who were watching it keep looking at
  the intended receiver as on the PS2, instead of turning their heads
  back to their animation.

- Level 2 gamebreaker sequences play the PS2's extra sound cue at the
  shot, the take-off and a high pass.

- An alley-oop lob the leaper hasn't caught by the end of its flight
  slows to half speed for its last few updates as on the PS2, instead of
  flying on at full speed.
- The commentator calls an advanced dunk's finish a dunk, as on the PS2,
  not an alley-oop.
- Computer players read a dribble picked up by a pump fake as the PS2's
  do: the ball handler no longer steers off the baseline out of it, and
  a defender no longer backs off him as from a live dribbler.

- The running forward pass picks up the passer's stride as on the PS2:
  its animation starts on the frame that matches the step he was in,
  instead of always from its first frame.

- A player who passes or switches to a lay-up in mid-air loses half his
  speed as on the PS2, so he drifts and lands where the original's does.

- Advanced dunks play as on the PS2: from 8.5 or more out with four turbo
  bars the dunker tosses the ball up off the backboard while he walks on,
  then leaps for it, catches it in the air and slams it on the original's
  updates. A toss from outside the board's zone comes off the glass loose,
  as it does on the PS2. (Before, a drawn ordinary dunk played in its
  place.)

- A diving player hops at the original's speed (a tall player's dive was
  about 7% too fast) and stays on the court in the air as on the PS2.
- Level 2 gamebreaker replays end on exactly the original's frame.
- Online matches no longer stop with an "invalid state" fault when a
  Level 2 gamebreaker ends in a dunk.
- A dribbler who stops now slides into his stance as on the PS2: a
  quarter of his speed through the stance's blend, and after a long run
  away from the basket one of the three dribble-stop animations, which
  carry him further.
- A player leaping for an alley-oop turns toward the rim as fast as on
  the PS2 (twice the old rate), so his run-up and the lob to him land
  where the original's do.
- A standing catch that plays its clip out to the end (`catch_stand_5`)
  now holds the catch for its own clip's length (12 updates) instead of
  another catch's (9), as on the PS2.
- The match intros pick their scripts as the PS2 does, from the game's
  own random numbers, and a streetball legend's own intro plays for the
  same side's legend the PS2 picks it for. The intros' dunks take their
  random numbers from the game's too (the dunker's face, the make roll,
  the crowd's cheer), so a match's choices follow the original's from the
  first update.
- The match intros' ball follows the scripts again (in the players'
  hands, passed and shot) instead of sitting at centre court.
- PlayStation controllers (DualSense, DualSense Edge, DualShock 4) now
  connect on Windows even when Windows' own game-controller layer doesn't
  hand them to the game: they're read straight from the pad, over USB or
  Bluetooth, with PlayStation button names and vibration over USB.
- The opening tip runs as the PS2's does: the players take their own
  standing poses at their spots (the jumpers one of the two tip-off
  stances), their faces keep time through the referee's wait, and the
  others start reacting the moment the tip is caught. A match's random
  choices now follow the original's through the whole tip.
- Street School's games have no spectators, as on the PS2 (the stands
  were full behind the lessons).
- The court's effects (steam, litter, breath, the hoop's fire, the
  gamebreaker ball's sparks) and the crowd take their random numbers from
  the game's own, after each frame's play, as the PS2 does, so the
  computer's later choices in a match follow the original's sequence. The
  sparks now start, drift and blink as the original's do.
- Each match lines its six players up in the original's update order,
  drawn as the PS2 draws it while building the game, so who reacts first
  within a frame (a lob's leaper, a pass's receiver) follows the
  original's.
- Players look as they do on the PS2: no lighting on the first pass, then
  the original's gloss from each court's own specular maps (skin and
  clothes), so shoulders and arms catch the highlight as they should. The
  ball gets the same treatment with its own map.
- Courts, crowds and players keep their exact colours: the 3D picture no
  longer goes through a tone curve and dither the original never had, and
  it now blends and shades in the PS2's own colour values, so shadows,
  fences, nets, the crowd and effects come out as dark or light as on the
  PS2 (shadows were too light).
- Xbox button prompts (issue #3): every button picture on the screens
  (the menus' legends, the pause screen, Help, Street School) shows A, B,
  X and Y in their colours, LB/RB/LT/RT and View/Menu when an Xbox-style
  pad is in use, including Steam Input and the Steam Deck. PC Settings has
  a new Button Prompts row: Auto (follow the controller), PlayStation or
  Xbox.
- Unlock Everything (issue #3): a PC Settings row that gives you every
  reward card at once (legends, courts, jerseys, codes and tips). Press
  it twice to confirm; the original's unlock codes still work too.

## v0.1.5-beta

Online play had a big pass this release. Everyone in a match needs this
version.

- **Steady online feel, like other sports games.** The input delay now
  covers your ping, so the other players' moves arrive before the game
  needs them and they almost never slide or jump. If a packet is late,
  the game corrects quietly instead of freezing. The delay comes from your
  real ping and keeps adjusting during the match: about 67 ms on a 40 ms
  connection, 117 ms at 115 ms. To trade steadiness for quicker presses,
  start the game with `--responsive`.
- **No more yo-yo ball.** Lag spikes no longer pull your own moves back:
  the host waits a moment for late controls instead of guessing them, and
  runs its server on its own thread so a busy host PC doesn't delay
  anyone. When a guess about another player turns out wrong, they glide
  back into place instead of snapping. In tests over a 115 ms connection
  with regular lag spikes, players jumped to new spots about twice in 90
  seconds, down from more than once a second.
- **Less stutter online:** about one hitch every 4 seconds on that same
  connection, down from about one a second.
- **Fixed: the other player's moves were often silent online.** Their
  shots, passes, jukes and dunks now play their sounds and effects, once
  each.
- **A player who drops is taken over by the computer** after a second,
  instead of running in the last direction they held. If they come back,
  they take over again. When the host's game closes, everyone is told
  within 5 seconds instead of after a 10-second freeze.
- **Host without playing.** Turn on Watch Only on the Online page, then
  host: your game runs and shows the match, and the people who join play
  it on equal terms, with nobody getting the host's ping advantage.
- **Online matches open with their intros,** as matches played alone do.
  Everyone sees them at the same time, and anyone pressing Cross skips
  them for everyone.
- **Fixed: a buzz under the match's sounds** (reported on GitHub). Sound
  volumes changed in steps many times a second instead of sliding, as the
  original's sound driver slides them, and looping sounds clicked at each
  pass of their loop. Both now play as the original does.
- **Music and movies at the original's levels,** measured from the
  original's own output. The menu songs had been about 5.5 dB too loud,
  so sounds on top of them clipped. The volume sliders now work like the
  original's (half the slider is half the volume).
- **Songs, movies and sounds are converted to 48 kHz the way the
  original's sound driver does it,** so the top end sounds like the
  console. (The crunchy hi-hats in "Chief Rocka" are in the record
  itself.)
- **The launcher:** the MyBaller arena turns behind it as you move the
  mouse, it has buttons for myballer.online and the Discord, and it shows
  its release's version (it read v0.1.0 in every release).

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
