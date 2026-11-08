# Changelog

What changed in each release. The newest is first; the release page shows
its version's part.

## Unreleased

- A defender who stops right beside the ball's holder in a restart shows
  off with his flair move on the same moment as on the PS2.
- After a basket, a player still landing from his dunk or shot waits for
  his landing to finish before he walks to his restart spot, as on the PS2.
- Players breaking into a run from the brief restart stand after a basket
  start their stride on the PS2's step, so they head off on the same line.
- Players' limbs between two animation frames now sit where the PS2 puts
  them: the in-between turns are no longer stretched back to full length,
  so hands and heads match the PS2 (a rebounder's hand under the
  backboard was 0.03 too high and the board held him back).
- A block or rebound-goaltend jump plays its landing out as on the PS2:
  the jumper finishes the animation after touching down, moved by it and
  turning where his stick points, before he can run again; his last frames
  in the air are no longer one moment behind.
- A player's idle face after a jump comes back at the same moment as on
  the PS2.
- The two jump-ball jumpers bump each other as on the PS2 once the ball is
  tipped: they share the push instead of one taking all of it, so neither
  drifts off his line through the rest of the opening play.
- A player who turns while his feet are planted (starting a pass, a
  shot or a dunk) now swings round his planted foot one moment later,
  as on the PS2, instead of sliding the instant he turns.
- A player who starts running straight out of a move (the jump ball, a
  shot, a dunk) now settles into his stride at the same moment as on the
  PS2, so his steps stay in time with the PS2's for the rest of the run.
- A player knocked over falls and slides as on the PS2: he lands where
  the PS2 puts him, held back by whoever he hits on the way.
- Block and rebound jumps fly as on the PS2: a jumper no longer sticks to
  the floor right after takeoff, his crouch starts on the animation frame
  the PS2 uses (several jumps leaned too far forward and pushed nearby
  players away), he moves with his animation until he leaves the floor
  and only with his jump after it, and the backboard stops holding him
  as soon as he starts to come down. A rebounder who lands near the ball
  after a basket now picks it up as on the PS2.
- Computer players' footwork and runs follow the PS2 more closely: a
  run or dribble that starts out of another stride, out of a stand that
  is still settling, or right at the end of a stride's loop starts on the
  same step as the PS2; a receiver running to a pass keeps his exact
  line; a defender heading back to guard the rim aims at the same point
  the PS2 does (just short of the rim); and a computer that shoots on the
  move picks his stride before the shot, as the PS2 does.
- A defender who comes to a stand picks his stance as on the PS2 (by the
  way he means to face, not the way he happens to face), a block or
  rebound jump goes up after the jumper's stride is picked, and a dunker
  meeting a blocker in the air is matched against him at the same moment
  as on the PS2 (by their hips' heights), so the same man wins.
- A dunk now plays its landing out in full as on the PS2 (the dunker
  stays in his finish until his feet are down and his animation is over,
  instead of being cut short), while the game stops counting him as
  dunking at the same point the PS2 does.
- A player knocked over in the air (by a dunker, or tumbling forward)
  is thrown up and over as on the PS2, instead of dropping straight down.
- The crowd's gasp when a player hits the floor is now part of the game
  itself (as on the PS2) rather than only the sound, so the game's random
  rolls after a hard fall stay the same as the PS2's.
- A player who catches a pass on the run now keeps running underneath
  the catch, as on the PS2: his legs and hips stay on his run while his
  arms and head play the catch, and the ball sits in those hands (it had
  been drawn up to two feet off to one side, the whole body turned
  toward the pass).
- When that catch finishes, his arms ease back into his run over five
  frames as on the PS2, with the ball staying in his hands, instead of
  snapping.
- A pass thrown on the run carries the passer on at the speed and in the
  direction the PS2 gives him as he lets go (his stick's speed for his
  run, along the way he turns to pass), instead of the speed he had.
- A pass thrown on the run plays over the run, as on the PS2: the
  passer's legs keep running (his run, or a jog with the stick only part
  way) while his arms throw, and the ball stays in those hands.
- After a pass on the run his arms ease back into his run over five
  frames, as on the PS2, instead of snapping.
- A pass thrown just before landing (within a sixth of a second) takes
  the animations for passing on the ground, and a passer moving in the
  air no longer gets the standing animations, as on the PS2.
- The advanced dunk's toss is drawn: the player's arms throw the ball up
  off the board over his stand, as on the PS2, and the ball leaves the
  hand those arms show (he had kept walking with his arms down).
- Moves started in the air keep the player up as long as the PS2 does:
  a pass or tip-in in mid-air, a dunk turned into a lay-up, and a player
  knocked flying forward all stay up for their animation's minimum time
  (the original's "stay up" script step had been read wrongly and left
  out), so they land when the original's do.
- A pass thrown in the air, and the alley-oop pass thrown on the run,
  play the animation for the side the receiver is on, and the spinning
  jump shot turns the way the PS2 picks from his facing toward the rim,
  as on the PS2 (each had always used one side's).
- Five lay-ups and dunks leap to the side of the hand the player has the
  ball in, as on the PS2, instead of always the same side.
- A player knocked flat gets up one of the PS2's two ways, picked at
  random as the original picks, instead of always the same one.
- A player who dribbles off out of a trick move (a crossover, a
  freestyle, a spin out of the post) starts his dribbling run in step with
  his feet, as on the PS2, instead of from a fixed point of it.
- The tip-off's pass and a restart's pass are thrown at where the
  receiver stands, as on the PS2, not to the spot he is heading for.
- Defenders guarding a pass's receiver close in on him on the PS2's
  timing: the AI counts the pass's flight from the release, not from the
  press.
- Computer players run in exactly the direction their AI picks, as on
  the PS2 (the direction had been rounded through a stick and could be a
  hair off), and their aim at a spot uses the original's own angle maths.
- The ball sits in the holder's hands as on the PS2 while he stands, as a
  catch plays out and as a new walk blends in, and players who turn to
  watch the ball only do so when they are far enough off it, as the
  original's animations ask.
- The opening tip's jumpers land and run off as on the PS2: the landing
  animation plays to its last frame and their run starts from it, facing
  the way the original turns them.
- A standing catch animation now plays out on the PS2's clock to its last
  frame, and the stand that follows blends out of it, so a player who
  catches and then passes keeps his feet where the original's are.
- A player whose shot, pass or other move ends moves on at once in the
  same update, as on the PS2 (the move's last step starts his run, turned
  to his new direction), instead of standing for an extra frame.
- After a running catch the receiver keeps running in a straight line
  until his catch animation lets go, as on the PS2, instead of turning at
  once. Shots, dunks and passes now blend out of the exact pose the
  player was showing.
- The match intro's dunk goes to the right player again (the side that
  loads the script fills its first three places, as the PS2's own memory
  shows), and a receiver who catches on the run keeps running through
  the catch instead of stopping to a stand under it.
- A passer now turns as on the PS2 when his pass starts: his facing snaps
  to the pass animation, and he turns toward where the receiver will be
  when the ball arrives. The receiver starts his run toward the ball the
  moment the pass is pressed, so the pass is judged on that run. A
  receiver catching on the run keeps his stride's footing, and a passer
  no longer starts walking before his pass animation lets him.
- Walking players sway with their walk animation as on the PS2 (the
  clip's own sideways drift now carries them), and the players around the
  opening tip keep the PS2's exact time on their standing animations, so
  all six now stand where the original's do through the whole opening.
- Players blend into a new animation from the exact pose they were
  showing, and the new animation holds its first frame through the blend,
  as on the PS2. Each walk, run and dribble now starts its animation at
  the PS2's own frame (from a standstill, or in step with the stride it
  replaces), so players stop, start and change pace in step with the
  original. A player who catches a pass standing keeps stepping with the
  catch animation after the catch, as on the PS2.
- Defenders judge whether they stand in front of the ball handler with
  the PS2's own angle rounding, so they try trick steals on the same
  updates as the original. A computer player who catches a pass waits the
  PS2's one extra update before he starts to dribble.
- Computer passers stop as they wind up a pass instead of drifting toward
  the receiver, and the passer and receiver glance at each other on the
  press, as on the PS2.

## v0.1.6-beta

Matches now play much closer to the PS2's, checked update by update
against the original running in an emulator. This release also brings
PlayStation controller support, Xbox button prompts and an Unlock
Everything option. Online, everyone in a match needs this version.

- **PlayStation controllers on Windows.** DualSense, DualSense Edge and
  DualShock 4 pads now connect even when Windows' own controller layer
  doesn't pass them to the game. They're read straight from the pad over
  USB or Bluetooth, with PlayStation button names, and vibration over
  USB.
- **Xbox button prompts** (issue #3). Every button picture in the menus,
  the pause screen, Help and Street School shows A, B, X, Y, LB/RB/LT/RT
  and View/Menu when an Xbox-style pad is in use, including Steam Input
  and the Steam Deck. PC Settings has a new Button Prompts row: Auto,
  PlayStation or Xbox.
- **Unlock Everything** (issue #3). A PC Settings row that gives you
  every reward card at once (legends, courts, jerseys, codes and tips).
  Press it twice to confirm. The original's unlock codes still work too.
- **Players and courts look like the PS2's.** The players get the
  original's gloss from each court's own highlight maps, so shoulders and
  arms catch the light as they should; the ball too. The picture keeps
  the PS2's exact colours, with no extra tone curve or dither, so
  shadows, fences, nets and the crowd are as dark or light as the
  original's (shadows were too light).
- **The opening tip plays as on the PS2.** Each side's three take their
  original places (the tallest centre jumps, the best ball handler takes
  the third spot; some teams had two players swapped, with each other's
  sizes and ratings). The players stand in their own poses at their
  spots, the jumpers move exactly as the original's from run-up to
  landing, the tip lands in the same update, and everyone starts
  reacting the moment it's caught.
- **Players turn as on the PS2** when a new animation starts: settling
  into a stance or starting to walk, they take the new clip's facing at
  once and pivot on the planted foot, instead of turning slowly the
  short way round.
- **The shot clock runs at the PS2's speed:** its 24 seconds tick down
  in 16 real seconds, one and a half times as fast as before.
- **Advanced dunks play as on the PS2:** from 8.5 or more out with four
  turbo bars, the dunker tosses the ball off the backboard, leaps for it,
  catches it in the air and slams it. (Before, an ordinary dunk played
  in its place.) The commentator now calls it a dunk.
- **Movement closer to the original's:**
  - A player who passes or switches to a lay-up in mid-air loses half his
    speed, so he lands where the original's does.
  - A dribbler who stops slides into his stance, and after a long run
    plays one of the three dribble-stop animations.
  - A running pass picks up the passer's stride.
  - An alley-oop leaper turns toward the rim twice as fast as before.
  - A tall player's dive was about 7% too fast.
  - A lob nobody caught slows down at the end of its flight.
  - A standing catch that plays its clip out holds for the right length.
- **The computer's choices follow the original's.** The camera's cuts,
  the crowd's reactions, the court's effects (steam, litter, breath, the
  hoop's fire, gamebreaker sparks), the match intros and the "21 on the
  clock" call now take their random numbers from the game's own at the
  PS2's moments, and the six players update in the original's order. So
  a match's random choices stay in step with the original's for much
  longer.
- **Fixes:**
  - Online matches no longer stop with an "invalid state" fault when a
    Level 2 gamebreaker ends in a dunk.
  - Level 2 gamebreaker replays end on the original's frame, and play
    the PS2's extra sound cue at the shot, the take-off and a high pass.
  - The match intros' ball follows the scripts again (in the players'
    hands, passed and shot) instead of sitting at centre court.
  - The court camera no longer bobs following a loose ball bouncing.
  - Street School's games have no spectators, as on the PS2.
  - After a pass goes loose, players watching it keep looking at the
    intended receiver.
  - Computer players read a pump-faked dribble as the PS2's do.

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
