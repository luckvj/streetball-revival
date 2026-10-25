"""Turn a played-match camera capture into row-helper cases for the Rust check.

Reads `tools/capture_camera_played.py` JSON lines (raw words, original frame)
and writes one case a line for `view::played_match_row_follows_the_original`
(set STREET_CAMERA_PLAYED_CASES to the output):

    S <frame> <state> <inputs> <expected>   one ordinary row update
    T <frame> <state> <inputs> <inputs> <expected>   a selection: the settle on
        the focus of the first read with the row on screen (the settle loop
        runs long enough for reads to land inside it), then the update of
        the first read whose focus moved (a selection from the intro's blend
        is the blend's handover, not a settle, and is skipped)
    P <frame> <courtside state> <its reads> <expected>   one update of the
        courtside (positioned) helper (`view::played_match_courtside_...`)

Every field is a hex word (floats as their bits, ints as two's complement).
A pair is skipped, and counted, when the capture's own integration identity
(eye/angles(k+1) = eye/angles(k) + rates(k+1)) fails on it or a neighbour:
those reads straddled a camera update. A shake started by an event in the
game update (the row's +0x200 count went up) is taken from the later sample.
Coordinates stay the original's (team 0's side byte +1); the Rust side reads
the side byte from the case, so no mirroring is applied.
"""
import argparse
import json
import struct
from pathlib import Path

ROWS = 0x498478
SHOT_POINTER = 0x401080
STATE = [0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C, 0xA4, 0xA8, 0xAC, 0x160, 0x164,
         0x1FC, 0x204, 0x208]
STATE_INT = [0x1F0, 0x1F4, 0x12C, 0x15C, 0x148, 0x1F8, 0x200]
GLOBALS = ["street_height_400b58", "street_lens_400b5c", "pass_depth_scale_4d8d04",
           "shot_depth_scale_4d8d08", "pass_yaw_half_4d8d0c"]
EXPECTED = [0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C, 0xA4, 0xA8, 0xAC, 0x160, 0x164]
EXPECTED_INT = [0x12C, 0x1F4, 0x15C]


def f32(word):
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


def s32(word):
    return word - 2**32 if word >= 2**31 else word


def hexw(value):
    if isinstance(value, float):
        return "%08x" % struct.unpack("<I", struct.pack("<f", value))[0]
    return "%08x" % (value & 0xFFFFFFFF)


def load(path):
    """The pointers, the last read of each game update, and every read."""
    pointers, samples, reads = [], [], []
    for line in open(path, encoding="utf-8"):
        record = json.loads(line)
        if record.get("event") == "pointers":
            pointers.append(record)
        elif "row" in record:
            reads.append(record)
            # A later read of the same game update (the camera finished, or
            # a replay moves it while the game update waits) replaces it.
            if samples and samples[-1]["frame"] == record["frame"]:
                samples[-1] = record
            else:
                samples.append(record)
    return pointers[-1], samples, reads


def selections(reads, row, blend):
    """Each selection of the row: the helper on screen before it, the first
    read with the row on screen (its focus is the settle's: the listener
    settles the row inside the game update, and the loop runs long enough
    for reads to land inside it), and the first read after the manager's
    next update (manager +0x60, the update counter it records at its end,
    moves on by one)."""
    for k in range(1, len(reads)):
        before, settle = reads[k - 1], reads[k]
        if before["active"] == row or settle["active"] != row or before["active"] == blend:
            continue
        counter = settle["manager"][0x60 // 4]
        if abs(counter - settle["frame"]) > 1:
            # A replay's recorder writes its frames' +0x60 back
            # (0x49b890): no update count to follow here.
            continue
        for after in reads[k + 1:k + 400]:
            if after["active"] != row:
                break
            if after["manager"][0x60 // 4] == counter + 1:
                yield before, settle, after
                break
            if after["manager"][0x60 // 4] != counter:
                break


def word(sample, offset):
    return sample["row"][offset // 4]


def state_words(sample):
    words = [word(sample, o) for o in STATE] + [word(sample, o) for o in STATE_INT]
    words += [sample.get(name, 0) for name in GLOBALS]
    return words


def inputs(sample, actors):
    """FUN_002f8370's reads, from the sample that holds this update's state."""
    phase = s32(sample["phase"])
    possession = s32(sample["possession"])
    sides = [(sample["sides"] >> (8 * k)) & 0xFF for k in range(4)]
    side = 0 if possession == -1 else (sides[possession] - 256 if sides[possession] >= 128
                                        else sides[possession])
    index = {a: i for i, a in enumerate(actors)}
    holder = index.get(sample["ball_holder"])
    target = index.get(sample.get("ball_target", 0))
    shooter = index.get(sample.get("ball_shooter", 0))
    bits = [sample[f"a{i}_bits"] for i in range(6)]

    def bit(actor, n):
        return actor is not None and (bits[actor] >> n) & 1 == 1

    ball_state = sample["ball_state"]
    distances = [f32(w) for w in sample.get("distances", [0] * 6)]
    # Older captures lack the controllers: the person is taken to control
    # his side's carrier, as the original hands him the ball's owner.
    controllers = sample.get("controllers")
    people = sides[2:4]
    computer_carrier = (controllers is not None and holder is not None
                        and people[holder // 3] != 0 and controllers[holder] == 4)
    flier = possession in (0, 1) and any(bit(a, 15) for a in range(3 * possession, 3 * possession + 3))
    # The row's focus descriptor (+0x1a0): type 3, the ball's owner (else
    # the ball, free), raises the focus by the owner's height (FUN_00305428);
    # type 0, a fixed point (the basket after a gamebreaker basket), by
    # nothing, and no free ball.
    owner_focus = word(sample, 0x1A0) == 3
    margin = (f32(sample[f"a{holder}_height"]) * 1.2
              if holder is not None and owner_focus else 0.0)
    focus = [word(sample, 0xD0), word(sample, 0xD4), word(sample, 0xD8)]
    motion = [word(sample, 0xF0), word(sample, 0xF4), word(sample, 0xF8)]
    reference = [word(sample, 0x100), word(sample, 0x104), word(sample, 0x108)]
    ball = sample["ball_position"][:3]
    return (focus + motion + reference + [margin, int(owner_focus and sample["ball_holder"] == 0)] + ball + [
        int(word(sample, 0x158) != 0), int(phase == 1), possession, side,
        int(2 <= phase <= 5 and sample.get("game_19c", 0) == 0), ball_state,
        int(sample["ball_gamebreaker"] != 0), sample.get("ball_distance", 0),
        int(bit(holder, 7) or bit(holder, 13) or bit(holder, 15)),
        int(ball_state == 4 and bit(target, 15)),
        int(ball_state == 5 and shooter is not None and distances[shooter] > 42.0),
        int(computer_carrier or flier)])


def expected(sample):
    return [word(sample, o) for o in EXPECTED] + [word(sample, o) for o in EXPECTED_INT]


COURTSIDE = [0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0xA8, 0xAC, 0x118, 0x11C, 0x1C0]
COURTSIDE_SOURCE = [0xC0, 0xC4, 0xC8, 0xD0, 0xD4, 0xD8, 0xF0, 0xF4, 0xF8, 0xE0, 0xE4, 0xE8]
COURTSIDE_EXPECTED = [0x90, 0x94, 0x98, 0xA8, 0xAC, 0x118, 0x11C, 0x1C4]


def helper(sample, offset):
    return sample["active_helper"][offset // 4]


def courtside_moving(sample, actors):
    """FUN_00305360's source vector for the helper's descriptor moves."""
    kind = helper(sample, 0x1A0)
    if kind == 2:
        entity = helper(sample, 0x180)
        if entity in actors:
            i = actors.index(entity)
            return any(f32(w) != 0.0 for w in sample[f"a{i}_motion"][:4])
        return False
    if kind == 3:
        holder = sample["ball_holder"]
        if holder in actors:
            i = actors.index(holder)
            return any(f32(w) != 0.0 for w in sample[f"a{i}_motion"][:4])
        if holder == 0:
            return any(f32(w) != 0.0 for w in sample["ball_motion"][:4])
    return False


def courtside_cases(samples, positioned, actors):
    """One update of the courtside (positioned) helper between two updates
    it is on screen for, with no selection between them."""
    for a, b in zip(samples, samples[1:]):
        if b["frame"] != a["frame"] + 1 or a["active"] != positioned or b["active"] != positioned:
            continue
        if "active_helper" not in a or "active_helper" not in b:
            continue
        if helper(a, 0x1C0) != helper(b, 0x1C0) or helper(a, 0x84) != helper(b, 0x84):
            continue  # re-placed
        if b["manager"][0x60 // 4] != a["manager"][0x60 // 4] + 1 or helper(b, 0x140) != 0:
            continue  # not one manager update apart (a replay holds the count)
        yield (["P", b["frame"]] + [helper(a, o) for o in COURTSIDE] + [helper(a, 0x134)]
               + [helper(b, o) for o in COURTSIDE_SOURCE] + [int(courtside_moving(b, actors))]
               + [helper(b, o) for o in COURTSIDE_EXPECTED] + [helper(b, 0x134)])


def straddled(samples, row):
    """Frames whose read straddled a camera update (identity fails)."""
    bad = set()
    for a, b in zip(samples, samples[1:]):
        if b["frame"] != a["frame"] + 1 or a["active"] != row or b["active"] != row:
            continue
        for pos, rate in ((0x8C, 0xA4), (0x90, 0xA8), (0x94, 0xAC)):
            value = f32(word(b, pos))
            if abs(f32(word(a, pos)) + f32(word(b, rate)) - value) > 4e-6 * max(1.0, abs(value)):
                bad.add(b["frame"])
    return bad


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs="+", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    counts = {"S": 0, "T": 0, "skipped_straddled": 0, "skipped_behind": 0}
    with args.out.open("w", encoding="utf-8") as out:
        for path in args.captures:
            pointers, samples, reads = load(path)
            row = pointers["helpers"][1]
            actors = pointers["actors"]
            bad = straddled(samples, row)
            lines = []
            for a, b in zip(samples, samples[1:]):
                if b["frame"] != a["frame"] + 1 or b["active"] != row or a["active"] != row:
                    continue
                if s32(word(a, 0x20C)) >= 0 or word(a, 0x1F0) == 4:
                    counts["skipped_behind"] += 1
                    continue
                if {a["frame"], b["frame"], b["frame"] + 1} & bad:
                    counts["skipped_straddled"] += 1
                    continue
                before = state_words(a)
                if word(b, 0x200) > word(a, 0x200):
                    # An event started a shake inside the game update.
                    for k, o in enumerate(STATE):
                        if o in (0x1FC, 0x204, 0x208):
                            before[k] = word(b, o)
                    before[len(STATE) + STATE_INT.index(0x200)] = word(b, 0x200) + 1
                lines.append((b["frame"], ["S", b["frame"]] + before + inputs(b, actors)
                              + expected(b)))
                counts["S"] += 1
            for before, settle, after in selections(reads, row, pointers["helpers"][3]):
                if word(before, 0x1F0) == 4:
                    continue
                lines.append((after["frame"], ["T", after["frame"]] + state_words(before)
                              + inputs(settle, actors) + inputs(after, actors) + expected(after)))
                counts["T"] += 1
            for fields in courtside_cases(samples, pointers["helpers"][2], actors):
                lines.append((fields[1], fields))
                counts["P"] = counts.get("P", 0) + 1
            # In update order, so that a case can carry the last one's state.
            lines.sort(key=lambda item: (item[0], item[1][0] == "S"))
            for _, fields in lines:
                out.write(" ".join(f if isinstance(f, str) else hexw(f) for f in fields) + "\n")
    print(json.dumps({"out": str(args.out), **counts}))


if __name__ == "__main__":
    main()
