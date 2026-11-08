# Changelog

What changed in each release. The newest is first; the release page shows
its version's part.

## Unreleased

- Online: after a game, REMATCH is the first and lit choice, as after a Pick Up game.
- The tip-off jump animates as on the PS2: it blends in from the stance, runs on time, and keeps moving down to the landing (it used to lag a frame and freeze after the tip).

## v0.1.7-beta

Online hosting works again, and matches play even closer to the PS2's,
checked update by update against the original. Online, everyone in a
match needs this version (an older copy is told so when it joins).

- **Online hosting fixed.** In v0.1.6-beta a host was sent straight back
  to the menu ("This roster breaks the online rules ... gait profile"):
  the game set each side's three in the PS2's order when it built the
  match, but sent them online in the order picked, so its own roster
  check refused the match. The match now goes online in the order it was
  built, and joiners load it as sent. Join by code, a laggy connection
  and a three-person lobby were all played through.
- **Finding Online.** It's on the Main Menu's Select button; with the
  keyboard the legend now reads "Online (P)", since the keyboard's Select
  is P.
- **Pasting join codes.** Ctrl+V pastes a code (or an address) into Join
  With Code, and the host's Ctrl+C copies its join code to send to
  friends.
- **Jumps as on the PS2.** Block and rebound jumps take off, fly and land
  like the original's: the crouch starts on the PS2's frame, the jumper
  moves with his animation until he leaves the floor, the backboard lets
  go as he comes down, and he plays his landing out (turning with the
  stick) before he can run again. A dunk plays its landing out in full,
  and a dunker meets a blocker in the air at the PS2's moment, so the
  same man wins.
- **Knockdowns as on the PS2.** A player knocked over falls, slides and
  is thrown up and over like the original's, held back by whoever he
  hits, gets up one of the PS2's two ways and walks off on the same
  moment. The two jump-ball jumpers share their bump instead of one
  drifting off.
- **Catches and passes on the run.** A receiver catching on the run keeps
  running underneath the catch (legs on the run, arms and head on the
  catch, the ball in those hands), and a pass thrown on the run plays
  over the run; both ease back into the run over five frames. A running
  pass carries the passer on at the PS2's speed and direction, and a
  pass just before landing uses the ground animations.
- **Footwork closer to the original's.** Players blend into each new
  animation from the exact pose they show, start each walk, run and
  dribble on the PS2's frame, sway with their walk, pivot on a planted
  foot a moment later as on the PS2, and break out of a move, a restart
  stand or a trick into a run in step with their feet. Limbs between two
  animation frames now sit where the PS2 puts them.
- **The ball in the hands.** The ball sits in the holder's hands as on the
  PS2 while he stands, catches and starts a walk; the advanced dunk's toss
  is drawn and leaves the hand it shows.
- **Animations picked as on the PS2.** Mid-air passes and the running
  alley-oop pass use the receiver's side, five lay-ups and dunks leap to
  the ball hand's side, the spinning jump shot turns the PS2's way, and
  moves started in the air stay up as long as the original's.
- **The computer's play.** Computer players run in exactly the direction
  their AI picks, stop as they wind up a pass, catch and wait the PS2's
  extra update before dribbling, try trick steals on the original's
  updates, run to passes on the PS2's lines and timing, guard the rim at
  the PS2's point, and pick their stride before a shot on the move.
- **Smaller fixes:**
  - After a basket, a player still landing waits for it before walking
    to his restart spot, and a defender stopping right beside the holder
    shows off his flair move on the PS2's moment.
  - A blocker jumping at a dunker reads the ball where the dunker holds
    it, so he swings the same arm.
  - The tip-off's and restarts' passes go to where the receiver stands.
  - The match intro's dunk goes to the right player again.
  - The crowd's gasp after a hard fall is part of the game, so the
    random rolls after it stay the PS2's.
  - A player's idle face after a jump comes back on the PS2's moment.

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
