"""Bounded, read-only PCSX2 PINE snapshots of the original US game's state.

Protocol source: PCSX2 v2.8.2 pcsx2/PINE.cpp. EnablePINE uses localhost.
No memory writes, gamepad input, or code patches are issued by this tool.
Reads are not a paused/atomic EE snapshot; pointer stability is reported.
"""
import argparse
import json
import socket
import struct
import time
from pathlib import Path


class Pine:
    def __init__(self, port):
        self.socket = socket.create_connection(("127.0.0.1", port), 5)
        self.socket.settimeout(5)
        self.socket.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

    def exact(self, count):
        data = bytearray()
        while len(data) < count:
            part = self.socket.recv(count - len(data))
            if not part:
                raise RuntimeError("PINE disconnected during response")
            data.extend(part)
        return bytes(data)

    def rpc(self, commands):
        self.socket.sendall(struct.pack("<I", len(commands) + 4) + commands)
        size = struct.unpack("<I", self.exact(4))[0]
        if not 5 <= size <= 65536:
            raise RuntimeError(f"Invalid PINE response size: {size}")
        reply = self.exact(size - 4)
        if reply[0] != 0:
            raise RuntimeError("PINE rejected the request; VM may be unavailable")
        return reply[1:]

    def words(self, addresses):
        addresses = list(addresses)
        if not addresses:
            return []
        payload = b"".join(b"\x02" + struct.pack("<I", a) for a in addresses)
        reply = self.rpc(payload)
        if len(reply) != 4 * len(addresses):
            raise RuntimeError("Unexpected PINE read response length")
        return list(struct.unpack(f"<{len(addresses)}I", reply))

    def word(self, address):
        return self.words([address])[0]

    def string(self, opcode):
        reply = self.rpc(bytes([opcode]))
        count = struct.unpack_from("<I", reply)[0]
        return reply[4:4 + count].rstrip(b"\0").decode("utf-8")


def float_word(word):
    return struct.unpack("<f", struct.pack("<I", word))[0]


def pointer(value):
    return 0 < value < 0x02000000 and value % 4 == 0


def fields(pine, base, layout):
    words = pine.words(base + offset + i * 4
                       for _, offset, kind in layout
                       for i in range({"vec4": 4, "vec3": 3}.get(kind, 1)))
    result = {"address": f"0x{base:08x}"}
    at = 0
    for name, _, kind in layout:
        if kind in ("vec4", "vec3"):
            count = 4 if kind == "vec4" else 3
            result[name] = [float_word(w) for w in words[at:at + count]]
            at += count
        else:
            word = words[at]
            result[name] = float_word(word) if kind == "f32" else word
            at += 1
    return result


CAMERA = [
    ("basis_forward", 0x10, "vec3"), ("basis_right", 0x20, "vec3"),
    ("basis_up", 0x30, "vec3"),
    ("eye", 0x84, "vec3"), ("pitch", 0x90, "f32"), ("yaw", 0x94, "f32"),
    ("lens_tangent", 0x98, "f32"), ("focus_current", 0xc0, "vec4"),
    ("velocity_x", 0x9c, "f32"), ("velocity_z", 0xa4, "f32"),
    ("velocity_pitch", 0xa8, "f32"), ("velocity_yaw", 0xac, "f32"),
    ("focus_saved", 0xd0, "vec4"), ("motion", 0xf0, "vec4"),
    ("count", 0x124, "u32"), ("previous_focus_type", 0x128, "u32"),
    ("transition", 0x12c, "u32"), ("inhibited", 0x13c, "u32"),
    ("ownerless", 0x14c, "u32"), ("vertical_gate", 0x158, "u32"),
    ("initializing", 0x140, "u32"), ("frozen", 0x144, "u32"),
    ("settled", 0x148, "u32"), ("screen_damping", 0x15c, "u32"),
    ("screen_accumulator", 0x160, "f32"), ("yaw_extra", 0x164, "f32"),
    ("descriptor", 0x174, "u32"),
    ("focus_type", 0x1a0, "u32"), ("vtable", 0x1b8, "u32"),
    ("mode", 0x1f0, "u32"), ("depth_offset_pointer", 0x1f4, "u32"),
]
PLAYER = [
    ("position", 0x30, "vec4"), ("motion", 0x40, "vec4"),
    ("orientation", 0x50, "vec4"), ("animation_state", 0xa4, "u32"),
    ("root_compensation", 0xe0, "vec4"), ("skeleton", 0x108, "u32"),
    ("branch_role", 0x194, "u32"), ("catch_delay", 0x1a8, "u32"),
    ("contact_delay", 0x1cc, "u32"), ("selected_hand", 0x25c, "u32"),
    ("pass_kind", 0x268, "u32"),
]
CHANNEL = [
    ("script_pc", 0x10, "u32"),
    ("state", 0x14, "u32"), ("flight", 0x20, "u32"),
    ("flags", 0x48, "u32"), ("animation", 0x50, "u32"),
    ("target_animation", 0x54, "u32"), ("link", 0x58, "u32"),
    ("frame_word", 0x5c, "u32"), ("target_frame_word", 0x60, "u32"),
    ("duration_word", 0x6c, "u32"), ("counter_word", 0x70, "u32"),
]
BALL = [
    ("position", 0x30, "vec4"), ("velocity", 0x40, "vec4"),
    ("target_position", 0x90, "vec4"), ("holder", 0xf4, "u32"),
    ("previous_holder", 0xfc, "u32"), ("target_player", 0x100, "u32"),
    ("timer", 0x158, "u32"), ("state", 0x180, "u32"),
    ("flight_updates", 0x64, "u32"), ("gravity", 0x118, "f32"),
    ("radius", 0x11c, "f32"), ("launch_flag", 0x184, "u32"),
    ("gravity_enabled", 0x188, "u32"), ("collision_flag", 0x18c, "u32"),
    ("bounce", 0x198, "u32"),
    ("special_drain", 0x1ac, "u32"),
]

# FUN_0014d1f8 advances this integer progress counter only up to +0x64.
# Nonzero +0x190 bypasses progress/timer handling, not position integration.
FLIGHT_CLOCK = [("progress", 0x60, "u32"), ("timer_bypass", 0x190, "u32")]
FLIGHT_CLOCK_GUARD = [
    ("progress", 0x60, "u32"), ("flight_updates", 0x64, "u32"),
    ("timer", 0x158, "u32"), ("state", 0x180, "u32"),
    ("timer_bypass", 0x190, "u32"),
]


def flight_clock_guard(pine, game, ball):
    # Identity and clock words share a packet, but EE execution is not paused.
    names = ["game", "ball"] + [name for name, _, _ in FLIGHT_CLOCK_GUARD]
    addresses = [0x3d30bc, game + 0x130] + [
        ball + offset for _, offset, _ in FLIGHT_CLOCK_GUARD]
    return dict(zip(names, pine.words(addresses)))


# Optional native ordinary-movement inputs. Unsigned words retain exact EE bits;
# field names distinguish floating values and pointers from opaque state words.
GAIT_PLAYER = [
    ("motion", 0x40, "vec4"), ("orientation", 0x50, "vec4"),
    ("option_bits", 0x78, "u32"), ("requested_heading", 0x88, "u32"),
    ("requested_intensity", 0x90, "u32"), ("actual_heading", 0xa8, "u32"),
    ("previous_heading", 0xac, "u32"), ("motion_heading", 0xb0, "u32"),
    ("desired_motion_heading", 0xb4, "u32"), ("held_ball", 0x70, "u32"),
    ("root_compensation", 0xe0, "vec4"), ("physical_magnitude", 0x190, "f32"),
    ("branch_state", 0x194, "u32"), ("held_transition", 0x1a0, "u32"),
    ("animation_flags", 0xa4, "u32"), ("root_state", 0x60, "u32"),
    ("flight_updates", 0x64, "u32"), ("script_state_198", 0x198, "u32"),
    ("script_state_19c", 0x19c, "u32"), ("script_state_1a4", 0x1a4, "u32"),
    ("selected_hand", 0x25c, "u32"), ("player_record", 0x274, "u32"),
    ("conditional_update_counter", 0x2a4, "u32"),
    ("frame_step", 0x32c, "f32"), ("previous_effective_row", 0x330, "u32"),
]
GAIT_CHANNEL = [
    ("script_pc", 0x10, "u32"), ("wait_updates", 0x4c, "u32"),
    ("target_start", 0x24, "u32"), ("target_key", 0x60, "u32"),
    ("row_variable", 0x28, "u32"), ("configured_step", 0x3c, "f32"),
    ("flags", 0x48, "u32"), ("animation", 0x50, "u32"),
    ("target_animation", 0x54, "u32"), ("link", 0x58, "u32"),
    ("frame_word", 0x5c, "u32"), ("frame_float", 0x64, "f32"),
    ("current_step", 0x68, "f32"), ("duration_word", 0x6c, "u32"),
    ("counter_word", 0x70, "u32"), ("previous_local_root", 0x90, "vec4"),
    ("captured_ball_offset", 0xa0, "vec4"), ("captured_ball_offset_enabled", 0xb0, "u32"),
    ("outgoing_pose_pointer", 0xbc, "u32"),
]
GAIT_RECORD = [
    ("script", 0, "u32"), ("row_a", 4, "u32"), ("row_b", 8, "u32"),
    ("category_mask", 0xc, "u32"), ("player_restriction", 0x10, "u32"),
    ("minimum_handles", 0x14, "u32"), ("root_distance", 0x18, "f32"),
    ("minimum_motion", 0x1c, "f32"), ("maximum_motion", 0x20, "f32"),
    ("flags", 0x24, "u32"),
]

def guarded_fields_match(packet, layout, expected):
    """Compare each typed field, including duplicate reads, to raw guard words."""
    for name, offset, kind in layout:
        values = packet[name] if kind in ("vec3", "vec4") else [packet[name]]
        for i, value in enumerate(values):
            at = offset + i * 4
            if at not in expected:
                continue
            word = (struct.unpack("<I", struct.pack("<f", value))[0]
                    if kind in ("f32", "vec3", "vec4") else value)
            if word != expected[at]:
                return False
    return True


def gait_channel(pine, channel):
    # Every scalar clock/selector input has explicit address-labelled bookends.
    # Old captures used nine unlabelled words and did not bracket integer frame,
    # configured step, link, duration or row variable. They remain historical.
    guard_layout = [(name, offset, kind) for name, offset, kind in GAIT_CHANNEL
                    if kind in ("u32", "f32")]
    guard_offsets = [offset for _, offset, _ in guard_layout]
    guard_addresses = [channel + offset for offset in guard_offsets]
    before = pine.words(guard_addresses)
    result = fields(pine, channel, GAIT_CHANNEL)
    outgoing = result["outgoing_pose_pointer"]
    if pointer(outgoing):
        result["outgoing_root_quaternion"] = [float_word(word) for word in pine.words(
            range(outgoing, outgoing + 16, 4))]
    after = pine.words(guard_addresses)
    main = [struct.unpack("<I", struct.pack("<f", result[name]))[0]
            if kind == "f32" else result[name] for name, _, kind in guard_layout]
    result.update(guard_offsets=guard_offsets, guards_before=before, guards_after=after,
                  guards_stable=before == after,
                  packet_matches_guards=main == before == after)
    return result


def model_scale_packet(pine, actor):
    # Original cached producer3153d0, appearance156514 and head21 writer19d59c.
    base = [actor + 0x274, actor + 0x108, actor + 0x28c, 0x4b6fe0, 0x4b6fe4, 0x3d0700]
    initial = pine.words(base)
    skeleton, appearance = initial[1:3]
    packet = dict(player_record=initial[0], skeleton=skeleton, appearance=appearance,
                  body_option=initial[3], head_option=initial[4], options_initialized=initial[5])
    if not pointer(skeleton) or skeleton + 0x64 > 0x02000000:
        packet['status'] = 'invalid_skeleton_retained'
        return packet
    if not pointer(appearance) or appearance + 0x74 > 0x02000000:
        packet['status'] = 'invalid_appearance_retained'
        return packet
    addresses = base + [skeleton + off for off in (0, 4, 8, 12, 0x10, 0x40, 0x44, 0x48, 0x4c, 0x50, 0x60)]
    addresses += [appearance + off for off in (0x3c, 0x50, 0x70, 0x5c, 0x60, 0x64)]
    addresses += [skeleton + 0x58, 0x3d30bc]
    before = pine.words(addresses)
    packet.update(bone_count=before[6], cached_head_native_order=[float_word(w) for w in before[7:10]],
                  skeleton_dirty10=before[10] & 255, body_scale=[float_word(w) for w in before[11:15]],
                  evaluated_scale_pointer=before[15], skeleton_dirty60=before[16] & 255,
                  appearance_skeleton=before[17], appearance_build=before[18], appearance_player_record=before[19],
                  cached_body_height=float_word(before[20]), cached_body_width=float_word(before[21]),
                  cached_head_compensation=float_word(before[22]), skeleton_vtable=before[23],
                  option_enable_game_pointer=before[24])
    vt = before[23]
    if pointer(vt) and vt + 0x40 <= 0x02000000:
        descriptor = [vt + 0x38, vt + 0x3c]
        addresses += descriptor
        descriptor_words = pine.words(descriptor)
        before += descriptor_words
        packet['head_gate_adjustment'] = struct.unpack('<h', struct.pack('<H', descriptor_words[0] & 0xffff))[0]
        packet['head_gate_target'] = descriptor_words[1]
    scale_array = before[15]
    evaluated_words = None
    if 22 <= before[6] <= 256 and pointer(scale_array) and scale_array + 22 * 16 <= 0x02000000:
        evaluated = list(range(scale_array + 21 * 16, scale_array + 22 * 16, 4))
        addresses += evaluated
        before += pine.words(evaluated)
        evaluated_words = pine.words(evaluated)
        packet['evaluated_head21_scale'] = [float_word(w) for w in evaluated_words]
    after = pine.words(addresses)
    packet.update(status='read', guard_addresses=addresses, guards_before=before, guards_after=after,
                  guards_stable=before == after,
                  identity_matches_initial=before[:6] == initial and after[:6] == initial,
                  appearance_aliases_match=before[17] == skeleton and before[19] == initial[0],
                  evaluated_words_match_guards=(evaluated_words == before[-4:] == after[-4:])
                  if evaluated_words is not None else None)
    return packet


def actor_update_gate_packet(pine, game):
    # These are inputs of140558, not its unobserved virtual return value.
    packet = fields(pine, game, [('vtable20', 0x20, 'u32'), ('mode_index', 0xf4, 'u32'),
                                 ('blocked19c', 0x19c, 'u32'), ('state1a0', 0x1a0, 'u32'),
                                 ('ball', 0x130, 'u32')])
    if packet['mode_index'] < 16:
        packet['mode_table_word'] = pine.word(0x3d30d0 + 4 * packet['mode_index'])
    else:
        packet['mode_table_read_status'] = 'outside_probe_bound_retained'
    vt = packet['vtable20']
    if pointer(vt) and vt + 0x70 <= 0x02000000:
        adjustment, target = pine.words([vt + 0x68, vt + 0x6c])
        adjustment = struct.unpack('<h', struct.pack('<H', adjustment & 0xffff))[0]
        packet.update(virtual_adjustment=adjustment, virtual_target=target,
                      virtual_adjusted_object=game + adjustment)
    if pointer(packet['ball']) and packet['ball'] + 0x184 <= 0x02000000:
        packet['ball_state180'] = pine.word(packet['ball'] + 0x180)
    return packet


def staged_clock_packet(pine, manager, actors):
    # Original 336628/32b810/19a658 fields. Guards detect some tearing,
    # never an atomic manager/actor update or a completed scheduler tick.
    manager_before = pine.words([manager + 0x50, manager + 0x58])
    packet = fields(pine, manager, [
        ("state", 0x50, "u32"), ("pending", 0x54, "u32"),
        ("next_tick", 0x58, "u32"), ("current_script", 0x34, "u32"),
        ("header", 0x44, "u32"), ("record_array", 0x48, "u32"),
        ("proxy_count", 0x28, "u32"), ("proxy_array", 0x24, "u32"),
    ])
    if pointer(packet["header"]):
        packet["header_words"] = pine.words(range(packet["header"], packet["header"] + 20, 4))
    if packet["state"] in range(3, 8) and "header_words" in packet:
        count = packet["header_words"][4]
        packet["record_read_status"] = "bounded" if count <= 512 else "over_limit_retained"
        packet["actor_records"] = []
        if count <= 512 and pointer(packet["record_array"]):
            table = pine.words(range(packet["record_array"], packet["record_array"] + count * 20, 4))
            for index in range(count):
                category, frame, duration, handled, payload = table[index * 5:index * 5 + 5]
                if category == 0:
                    record = dict(index=index, frame=frame, duration=duration,
                                  handled=handled, payload_pointer=payload)
                    if pointer(payload):
                        record["payload_words"] = pine.words(range(payload, payload + 52, 4))
                    packet["actor_records"].append(record)
    packet["actors"] = []
    for actor in actors:
        address = int(actor["address"], 16)
        proxy = pine.word(address + 0x2a0)
        channel_address = int(actor.get("channel0", {}).get("address", "0"), 16)
        if not pointer(proxy) or not pointer(channel_address):
            continue
        skeleton = pine.word(address + 0x108)
        guards = [proxy + 0x14, channel_address + 0x64, channel_address + 0x70,
                  proxy + 0x20, proxy + 0x28, proxy + 0x90, proxy + 0x98,
                  address + 0x34, address + 0x108]
        if pointer(skeleton):
            guards.append(skeleton + 0x40)
        # Bracket physical x/z in the same packets as proxy tick and height.
        # The legacy clock guard keeps its original shape; position checks
        # use the complete packet and retain both bookends separately.
        position_addresses = [address + 0x30, address + 0x38, proxy + 0x24,
                              address + 0xe0, address + 0xe4, address + 0xe8,
                              address + 0x50, address + 0x54, address + 0x58, address + 0x5c,
                              proxy + 0x134]
        position_before = pine.words(guards + position_addresses)
        before = position_before[:-len(position_addresses)]
        item = fields(pine, proxy, [
            ("last_tick", 0x14, "u32"), ("manager", 0x18, "u32"),
            ("actor", 0xb8, "u32"), ("record_duration", 0xdc, "u32"),
            ("seek_countdown", 0xd8, "u32"), ("initial_pending", 0xc0, "u32"),
        ])
        # Proxy +20..+53 follows the Actor payload layout, including packed
        # shorts. Some fields are live/interpolated, not immutable source data.
        item["payload_words"] = pine.words(range(proxy + 0x20, proxy + 0x54, 4))
        item["previous_proxy_position"] = [float_word(word) for word in
                                            pine.words(range(proxy + 0x90, proxy + 0x9c, 4))]
        item["actor_height"] = float_word(pine.word(address + 0x34))
        if pointer(skeleton):
            item["skeleton_scale"] = fields(pine, skeleton, [("body_scale", 0x40, "vec4")])
        item["channel"] = fields(pine, channel_address, [
            ("skip_counter", 0x4c, "u32"), ("flags", 0x48, "u32"),
            ("animation", 0x50, "u32"), ("target_animation", 0x54, "u32"),
            ("key", 0x5c, "u32"), ("target_key", 0x60, "u32"),
            ("frame", 0x64, "f32"), ("step", 0x68, "f32"),
            ("duration", 0x6c, "u32"), ("counter", 0x70, "u32"),
            ("configured_step", 0x3c, "f32"), ("target_start", 0x24, "u32"),
        ])
        position_after = pine.words(guards + position_addresses)
        after = position_after[:-len(position_addresses)]
        pb, pa = (words[-len(position_addresses):] for words in (position_before, position_after))
        item.update(physical_position_before=[pb[0], before[7], pb[1]],
                    physical_position_after=[pa[0], after[7], pa[1]],
                    reference_position_before=[before[3], pb[2], before[4]],
                    reference_position_after=[after[3], pa[2], after[4]],
                    root_compensation_before=pb[3:6], root_compensation_after=pa[3:6],
                    orientation_before=pb[6:10], orientation_after=pa[6:10],
                    actor_launch_before=pb[10], actor_launch_after=pa[10],
                    physical_position_guards_stable=position_before == position_after)
        item.update(actor_id=actor["id"], guards_before=before, guards_after=after,
                    guards_stable=before == after,
                    proxy_matches_actor=item["actor"] == address,
                    proxy_matches_manager=item["manager"] == manager)
        packet["actors"].append(item)
    manager_after = pine.words([manager + 0x50, manager + 0x58])
    packet.update(guards_before=manager_before, guards_after=manager_after,
                  guards_stable=manager_before == manager_after)
    return packet


def snapshot(pine, include_rig=False, include_gait=False, include_flight_clock=False,
             include_trick_awards=False, include_staged=False, include_model_scales=False):
    roots = [0x3d30bc, 0x400058, 0x402020]
    before = pine.words(roots)
    game, context, active = before
    result = {"wall_time_ns": time.time_ns(),
              "roots": dict(zip(("game", "context", "active_camera"), before))}
    if include_gait:
        result["native_rng_words"] = pine.words(range(0x4309f0, 0x430a08, 4))
    values = pine.words([0x4d8cfc, 0x4da23c, 0x400b58, 0x400b5c, 0x4bbf88, 0x4bc03c,
                         0x400b40, 0x400b44])
    result["globals"] = {"camera_scale": float_word(values[0]), "fps": values[1],
                         "street_height_offset": float_word(values[2]),
                         "street_lens_offset": float_word(values[3]),
                         "native_game_type": values[4], "pause_camera_word": values[5],
                         "lens_damping_enabled": values[6],
                         "lens_damping": float_word(values[7])}
    if pointer(active):
        result["active_camera"] = fields(pine, active,
                                          [field for field in CAMERA if field[1] <= 0x1b8])
        if result["active_camera"]["vtable"] == 0x498950:
            result["active_camera"].update(fields(pine, active, [
                ("focus_e0", 0xe0, "vec4"), ("distance_scale", 0x1c0, "f32"),
                ("target_lens", 0x1c4, "f32"),
            ]))
    if pointer(context):
        manager = pine.word(context + 0x10)
        if pointer(manager):
            result["camera_manager"] = fields(pine, manager, [
                ("helper24", 0x24, "u32"), ("helper28", 0x28, "u32"),
                ("helper2c", 0x2c, "u32"), ("helper30", 0x30, "u32"),
                ("cinematic", 0x40, "u32"), ("tracking_player", 0x44, "u32"),
                ("derived_type_zero", 0x4c, "u32"), ("delay", 0x50, "u32"),
                ("state", 0x64, "u32"),
            ])
            row = pine.word(manager + 0x28)
            if pointer(row):
                result["row_camera"] = fields(pine, row, CAMERA)
                result["row_camera_active"] = row == active
                descriptor = result["row_camera"]["descriptor"]
                if pointer(descriptor):
                    result["camera_descriptor_state"] = pine.word(descriptor + 0x88)
    if pointer(game):
        combo = pine.word(game + 0xd0)
        if pointer(combo) and pine.word(combo) == 0x43c208:
            result["combo_manager"] = fields(pine, combo, [
                ("handoff_kind", 0x154, "u32"), ("owner", 0x15c, "u32"),
                ("previous_action_flag", 0x164, "u32"),
                ("handoff_seen", 0x168, "u32"), ("enabled", 0x16c, "u32"),
            ])
            result["combo_ring"] = [fields(pine, combo + 4 + index * 0x18, [
                ("class", 0, "u32"), ("extra", 4, "u32"), ("count", 8, "u32"),
                ("value", 12, "u32"), ("id", 16, "u32"), ("folded", 20, "u32"),
            ]) for index in range(6)]
        result["actors"] = []
        if include_gait:
            result["actor_update_gate_inputs"] = actor_update_gate_packet(pine, game)
        for team in pine.words([game + 0x184, game + 0x188]):
            if not pointer(team):
                continue
            array = pine.word(team + 0x44)
            if not pointer(array):
                continue
            for actor in pine.words([array, array + 4, array + 8]):
                if pointer(actor):
                    if include_gait:
                        actor_guard_addresses = [actor + offset for offset in
                                                 (0x30, 0x34, 0x38, 0x40, 0x44, 0x48, 0x4c,
                                                  0x50, 0x54, 0x58, 0x5c, 0x60, 0x64,
                                                  0x70, 0xa4, 0xa8, 0xac, 0xb0, 0xb4, 0xc8,
                                                  0x198, 0x19c, 0x1a0, 0x1a4, 0x25c, 0x274, 0x2a4)]
                        actor_guard_before = pine.words(actor_guard_addresses)
                        guard_offsets = [address - actor for address in actor_guard_addresses]
                        guarded_channel_set = actor_guard_before[guard_offsets.index(0xc8)]
                        slot_before = pine.word(guarded_channel_set) if pointer(guarded_channel_set) else None
                    actor_layout = [
                        ("position", 0x30, "vec3"), ("animation_state", 0xa4, "u32"),
                        ("pass_kind", 0x268, "u32"), ("id", 0x27c, "u32"),
                    ]
                    actor_fields = fields(pine, actor, actor_layout)
                    if include_model_scales:
                        actor_fields["model_scales"] = model_scale_packet(pine, actor)
                    if include_trick_awards:
                        actor_fields["trick_award"] = fields(pine, actor, [
                            ("base_value_flag", 0x218, "u32"),
                            ("value_class", 0x248, "u32"),
                            ("script_index", 0x264, "u32"),
                        ])
                    if include_gait:
                        actor_fields["gait"] = fields(pine, actor, GAIT_PLAYER)
                    channels = pine.word(actor + 0xc8)
                    if pointer(channels):
                        channel = pine.word(channels)
                        if pointer(channel):
                            actor_fields["channel0"] = fields(pine, channel, [
                                ("script_pc", 0x10, "u32"), ("state", 0x14, "u32"),
                                ("animation", 0x50, "u32"), ("frame_word", 0x5c, "u32"),
                            ])
                            if include_gait:
                                actor_fields["channel0"]["gait"] = gait_channel(pine, channel)
                    if include_gait:
                        slot_after = pine.word(guarded_channel_set) if pointer(guarded_channel_set) else None
                        actor_guard_after = pine.words(actor_guard_addresses)
                        expected = dict(zip(guard_offsets, actor_guard_before))
                        actor_fields["ordinary_vm_guard"] = dict(
                            offsets=guard_offsets, before=actor_guard_before, after=actor_guard_after,
                            stable=actor_guard_before == actor_guard_after,
                            packet_matches_guards=(actor_guard_before == actor_guard_after
                                and guarded_fields_match(actor_fields, actor_layout, expected)
                                and guarded_fields_match(actor_fields["gait"], GAIT_PLAYER, expected)
                                and channels == guarded_channel_set),
                            channel_set=guarded_channel_set, channel_slot_before=slot_before,
                            channel_slot_after=slot_after,
                            channel_identity_stable=(channels == guarded_channel_set
                                and slot_before == slot_after
                                and (slot_before == int(actor_fields["channel0"]["address"], 16)
                                     if "channel0" in actor_fields else not pointer(slot_before or 0))))
                    result["actors"].append(actor_fields)
        ball, queue, planner = pine.words([game + 0x130, game + 0x134, game + 0x160])
        result["planner"] = struct.unpack("<i", struct.pack("<I", planner))[0]
        if pointer(queue):
            result["queue_state"] = pine.word(queue + 8)
        if pointer(ball):
            if include_flight_clock:
                clock_before = flight_clock_guard(pine, game, ball)
            result["ball"] = fields(pine, ball, BALL + FLIGHT_CLOCK if include_flight_clock else BALL)
            if include_flight_clock:
                clock_after = flight_clock_guard(pine, game, ball)
                packet_matches = all(
                    guard["game"] == game and guard["ball"] == ball
                    and all(result["ball"][name] == guard[name]
                            for name, _, _ in FLIGHT_CLOCK_GUARD)
                    for guard in (clock_before, clock_after))
                result["flight_clock"] = {
                    "guard_before": clock_before, "guard_after": clock_after,
                    "guard_words_stable": clock_before == clock_after,
                    "packet_matches_guards": packet_matches,
                }
            b = result["ball"]
            player = b["target_player"] if b["state"] == 4 else b["holder"]
            if pointer(player):
                result["player"] = fields(pine, player, PLAYER)
                skeleton = result["player"]["skeleton"]
                if pointer(skeleton):
                    result["skeleton"] = fields(pine, skeleton, [
                        ("bone_count", 0, "u32"), ("root_translation", 0x20, "vec4"),
                        ("evaluated_root_translation", 0x30, "vec4"),
                        ("body_scale", 0x40, "vec4"), ("bones_pointer", 0x54, "u32"),
                    ])
                    sk = result["skeleton"]
                    if pointer(sk["bones_pointer"]) and 20 <= sk["bone_count"] <= 256:
                        result["hand_bones"] = {
                            str(index): fields(pine, sk["bones_pointer"] + index * 0xb0, [
                                ("translation", 0, "vec4"), ("quaternion", 0x30, "vec4"),
                            ]) for index in (15, 19)
                        }
                        if include_rig:
                            # One batch retains the hierarchy inputs beside its
                            # evaluated outputs; field names include bone index.
                            layout = [
                                (f"{index}:{name}", index * 0xb0 + offset, kind)
                                for index in range(sk["bone_count"])
                                for name, offset, kind in [
                                    ("translation", 0, "vec4"),
                                    ("local_translation", 0x10, "vec4"),
                                    ("local_quaternion", 0x20, "vec4"),
                                    ("quaternion", 0x30, "vec4"),
                                    ("parent", 0x40, "u32"), ("index", 0x44, "u32"),
                                ]
                            ]
                            result["rig"] = fields(pine, sk["bones_pointer"], layout)
                array = pine.word(player + 0xc8)
                if pointer(array):
                    channel = pine.word(array)
                    if pointer(channel):
                        result["channel0"] = fields(pine, channel, CHANNEL)
                        if include_gait:
                            result["channel0"]["gait"] = gait_channel(pine, channel)
        script_manager = pine.word(game + 0x424)
        if pointer(script_manager):
            result["script_manager_state"] = pine.word(script_manager + 0x50)
            if include_staged:
                result["staged_clock"] = staged_clock_packet(pine, script_manager, result["actors"])
    result["self_toss_target"] = [float_word(w) for w in pine.words(
        range(0x3d6590, 0x3d65a0, 4))]
    result["self_toss_duration"], result["self_toss_bounce"] = pine.words([0x3d65a0, 0x3d65a4])
    if include_gait and pointer(game):
        result['actor_update_gate_inputs_after'] = actor_update_gate_packet(pine, game)
        result['actor_update_gate_inputs_stable'] = (
            result.get('actor_update_gate_inputs') == result['actor_update_gate_inputs_after'])
    result["root_pointers_stable"] = before == pine.words(roots)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=28011)
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--interval", type=float, default=1.0)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--rig", action="store_true", help="include full native local/evaluated rig")
    parser.add_argument("--gait", action="store_true", help="include native gait/channel inputs and RNG words")
    parser.add_argument("--model-scales", action="store_true", help="include independently guarded cached body/head and evaluated head21 scale producer words")
    parser.add_argument("--flight-clock", action="store_true",
                        help="include native ball progress/bypass and bracketed clock/identity guards")
    parser.add_argument("--trick-awards", action="store_true",
                        help="include native actor base-value flag, class and repeat script index")
    parser.add_argument("--staged", action="store_true",
                        help="include original manager/proxy/channel clocks with non-atomic guards")
    args = parser.parse_args()
    if not 1 <= args.samples <= 1000 or not 0 <= args.interval <= 60:
        parser.error("samples must be 1..1000; interval must be 0..60 seconds")
    pine = Pine(args.port)
    try:
        metadata = {"emulator": pine.string(8), "title": pine.string(11),
                    "serial": pine.string(12), "uuid": pine.string(13),
                    "limitation": "Live reads are not atomic or paused; emulator fixes may apply."}
        if metadata["serial"] != "SLUS-20651":
            raise RuntimeError(f"Wrong executable for this field map: {metadata['serial']}")
        if args.flight_clock:
            metadata["flight_clock_capture"] = True
            metadata["flight_clock_limitation"] = (
                "Clock/identity packets bracket the ball field packet; stable matching words "
                "do not prove atomic reads or a completed update. Integer progress saturates "
                "at flight_updates. Endpoint guards cannot prove interior branch continuity.")
        if args.staged:
            metadata["staged_clock_capture"] = True
            metadata["staged_motion_table_before"] = {
                "initialization_flag": pine.word(0x4132f8),
                "words": pine.words(range(0x4dcd20, 0x4dcd20 + 108 * 40, 4)),
                "limitation": "Lazy table read once before samples; init flag and words not atomic."}
            metadata["staged_clock_limitation"] = (
                "Manager tick is next tick; proxy tick is last applied tick. Guarded reads "
                "are non-atomic and cannot prove scheduler completion or prior pose blending.")
        if args.trick_awards:
            metadata["trick_award_capture"] = True
            metadata["trick_award_limitation"] = (
                "Actor fields are read separately from channels/awards; no atomic event pairing.")
        if args.model_scales:
            metadata["model_scale_limitation"] = ("Source cache/evaluated scale words are separately bracketed, not atomic; "
                                                   "equal bookends do not prove application timing or normal options.")
        if args.gait:
            metadata["gait_records"] = [fields(pine, 0x4b8790 + i * 0x28, GAIT_RECORD)
                                        for i in range(108)]
            metadata["gait_table_limitation"] = "Runtime table read once before samples; not atomic."
            metadata["ordinary_vm_fields"] = True
            metadata["ordinary_vm_guard_schema"] = 3
            metadata["ordinary_vm_guard_limitation"] = (
                "Actor and channel bookends bracket separate reads; equal words do not prove atomicity, "
                "completed updates, uninterrupted history or evaluated outgoing/linked pose buffers.")
        samples = []
        for i in range(args.samples):
            if i:
                time.sleep(args.interval)
            item = snapshot(pine, args.rig, args.gait, args.flight_clock, args.trick_awards, args.staged, args.model_scales)
            samples.append(item)
            print(json.dumps(item, allow_nan=False))
        if args.staged:
            metadata["staged_motion_table_after"] = {
                "initialization_flag": pine.word(0x4132f8),
                "words": pine.words(range(0x4dcd20, 0x4dcd20 + 108 * 40, 4)),
                "limitation": "Second table read detects endpoint changes, not interior changes."}
        if args.out:
            coverage = {
                "samples": len(samples),
                "active_game": sum(pointer(item["roots"]["game"]) for item in samples),
                "six_actors": sum(len(item.get("actors", [])) == 6 for item in samples),
                "combo_ring": sum("combo_ring" in item for item in samples),
                "complete_rig": sum("rig" in item for item in samples),
            }
            if args.staged:
                packets = [item["staged_clock"] for item in samples if "staged_clock" in item]
                coverage["staged_clock_packets"] = len(packets)
                coverage["staged_active_packets"] = sum(packet["state"] in range(3, 8) for packet in packets)
                coverage["staged_actor_packets"] = sum(len(packet["actors"]) for packet in packets)
                coverage["staged_manager_guarded_packets"] = sum(packet["guards_stable"] for packet in packets)
            if args.trick_awards:
                coverage["trick_award_packets"] = sum(
                    "trick_award" in actor
                    for item in samples for actor in item.get("actors", []))
            if args.gait:
                coverage["gait_actor_packets"] = sum(
                    "gait" in actor and "gait" in actor.get("channel0", {})
                    for item in samples for actor in item.get("actors", []))
                coverage["native_rng_packets"] = sum("native_rng_words" in item for item in samples)
            if args.flight_clock:
                clocks = [item["flight_clock"] for item in samples if "flight_clock" in item]
                coverage["flight_clock_packets"] = len(clocks)
                coverage["flight_clock_guarded_packets"] = sum(
                    clock["guard_words_stable"] and clock["packet_matches_guards"] for clock in clocks)
                coverage["flight_clock_unstable_packets"] = sum(
                    not clock["guard_words_stable"] or not clock["packet_matches_guards"] for clock in clocks)
            report = json.dumps({"metadata": metadata, "coverage": coverage, "samples": samples},
                                indent=2, allow_nan=False)
            temporary = args.out.with_suffix(args.out.suffix + ".tmp")
            temporary.write_text(report + "\n", encoding="utf-8")
            temporary.replace(args.out)
    finally:
        pine.socket.close()


if __name__ == "__main__":
    main()
