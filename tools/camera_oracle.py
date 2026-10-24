"""Discrete native camera arithmetic oracle, independent of the Rust renderer.

Run with a JSON snapshot path (or - for stdin). Supply eye, focus, motion,
velocity [x,z,pitch,yaw], mode, side and the native framing flags. The output
contains the 13-float command record and predicted post-update fields.

This deliberately does not infer framing flags, actor predicates, descriptor
transitions or source motion from Rust gameplay. Supply those native inputs.
For Street/Court supply depth_offset and transition_depth_step when +12c is
nonzero. For Street/Wide supply screen_motion_factor when +15c activates the
screen-space damping branch. Floating point uses rounded float32 operations,
but does not emulate EE exceptional values or VU rounding modes.
"""

import argparse
import json
import math
import struct
import sys


ROWS = [
    (4., 5., 12., 12., -30., .33),
    (15., 5., 16., 12., -40., .23),
    (18., 12., 22., 0., -65., .18),
    (0., 0., 28., 0., -75., .15),
    (0., -60., 19., 26.5, 0., .281),
    (4., 5., 12., 8., -40., .315),
]
COEFFICIENTS = (.16770227253437042, .06765850633382797,
                .05200503021478653, .05498472973704338)


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def add(a, b):
    return f32(a + b)


def sub(a, b):
    return f32(a - b)


def mul(a, b):
    return f32(a * b)


def div(a, b):
    return f32(a / b)


def native_atan(x):
    """FUN_002ff1c8/FUN_002face8 polynomial, finite scalar inputs."""
    x = f32(x)
    square = mul(x, x)
    t = div(1., add(square, 1.)) if abs(x) > 1. else div(square, add(square, 1.))
    p = COEFFICIENTS[3]
    for coefficient in reversed(COEFFICIENTS[:3]):
        p = add(coefficient, mul(t, p))
    result = mul(f32(math.sqrt(t)), add(1., mul(t, p)))
    if x < -1.:
        return add(result, -f32(math.pi / 2))
    if x > 1.:
        return sub(f32(math.pi / 2), result)
    return -result if x < 0. else result


def renderer_projection(scalar, aspect=4/3, zoom=1.3):
    """300990 degree emission -> 378328 full-viewport coefficients.

    Host tan stands in for the EE fdlibm tangent kernel; allow an ulp.
    """
    horizontal = mul(f32(scalar), f32(4/3))
    angle = native_atan(horizontal)
    degrees = mul(add(angle, angle), f32(57.295776))
    half_radians = mul(mul(degrees, .5), f32(.017453294))
    tangent = f32(math.tan(half_radians))
    m00 = div(1., mul(tangent, f32(zoom)))
    return {'horizontal_degrees': degrees, 'm00': m00,
            'm11': mul(-f32(aspect), m00),
            'vertical_fov_degrees': math.degrees(f32(2 * math.atan(
                div(mul(tangent, f32(zoom)), f32(aspect)))))}


def native_atan2(y, x):
    # Native zero/negative-axis choice gives -pi for y==0,x<0.
    if x == 0.:
        return 0. if y == 0. else math.copysign(f32(math.pi / 2), y)
    result = native_atan(div(y, x))
    if x < 0.:
        result = add(result, f32(math.pi) if y > 0. else -f32(math.pi))
    return result


def wrap(angle):
    if angle > f32(math.pi):
        return sub(angle, f32(2 * math.pi))
    if angle < -f32(math.pi):
        return add(angle, f32(2 * math.pi))
    return angle


def angles(vector):
    x, y, z = vector
    horizontal = f32(math.sqrt(add(mul(x, x), mul(z, z))))
    return -native_atan2(y, horizontal), native_atan2(x, z)


def approach_step(current, target, limit):
    difference = sub(target, current)
    return math.copysign(min(abs(difference), limit), difference) if difference else 0.


def velocity_step(current, command, correction, yaw=False):
    # 2fda08 does not snap to a nonzero command when correction overshoots.
    if correction == 0. or abs(sub(current, command)) < abs(correction):
        if command == 0. or (yaw and current * command < 0.):
            return 0.
        return current
    return add(current, correction)


def positioned_lens(current, distance_scale, eye, focus_e0, fps=60., damping=.8):
    """301fa8 target and raw 301d50 easing for the active +2c helper."""
    difference = [sub(a, b) for a, b in zip(eye[:3], focus_e0[:3])]
    square = add(add(mul(difference[0], difference[0]),
                     mul(difference[1], difference[1])),
                 mul(difference[2], difference[2]))
    distance = f32(math.sqrt(square))
    target = div(distance_scale, distance)
    delta = mul(sub(target, current), .25)
    limit = div(60., fps)
    correction = max(-limit, min(limit, mul(delta, damping)))
    return {'distance': distance, 'target': target,
            'scalar': add(current, sub(delta, correction))}


def project_basis(eye, point, scalar, basis, near=.5):
    """2ffac0 projection using matching native +10/+20/+30 basis snapshots.

    Basis must reflect the yaw-extra branch being checked. +2fa558's published
    basis excludes +164; reject or separately rotate a nonzero yaw-extra input.
    """
    relative = [sub(point[i], eye[i]) for i in range(3)]
    def dot(axis):
        return add(add(mul(relative[0], axis[0]), mul(relative[1], axis[1])),
                   mul(relative[2], axis[2]))
    depth = dot(basis['forward'])
    if depth <= near:
        return None
    tan_h = mul(scalar, f32(4 / 3))
    # Native VU path divides XYZ by depth, subtracts (tanH,tanV),
    # then multiplies by screen dimensions / tangent * -0.5.
    x = mul(mul(sub(div(dot(basis['right']), depth), tan_h), div(640., tan_h)), -.5)
    y = mul(mul(sub(div(dot(basis['up']), depth), scalar), div(480., scalar)), -.5)
    return [x, y]


def zone_mask(screen, thresholds, top_y=None):
    """FUN_002ff830's exact ordered pixel comparisons and bit mask."""
    if screen is None:
        return 0x4000
    x, y = screen
    top_y = y if top_y is None else top_y
    left, left_band, right, right_band, top, top_band, bottom, bottom_band = thresholds
    mask = 0x1000 if x < 0. else 0x2000 if x > 640. else 0
    if top_y < 0.:
        mask |= 0x400
    elif y > 480.:
        mask |= 0x800
    if x < left:
        mask |= 0x100
    elif x < left + left_band:
        mask |= 0x10
    elif x > 640. - right:
        mask |= 0x200
    elif x > 640. - right - right_band:
        mask |= 0x20
    else:
        mask |= 1
    if top_y < top:
        mask |= 0x40
    elif top_y < top + top_band:
        mask |= 4
    if y > 480. - bottom:
        mask |= 0x80
    elif y > 480. - bottom - bottom_band:
        mask |= 8
    return mask if mask & 0xcc else mask | 2


def normal_flags(focus_zone, reference_zone, alternate_zone, side,
                 reference_x, special_game_state=False):
    """2f8370 non-Behind framing flags after all three zone inputs are known.

    alternate_zone differs from reference_zone for the Street/Wide ball-state3
    and offscreen-reference branches. Caller must supply the observed/derived
    alternate zone rather than pretending every camera uses one focus point.
    """
    flags = 0
    if focus_zone & 0x100:
        flags = 1
    elif focus_zone & 0x200:
        flags = 2
    elif not special_game_state:
        if reference_zone & 0x2200 or (reference_zone & 0x4000 and reference_x < 0.):
            if not focus_zone & 0x10:
                flags = 2
        elif reference_zone & 0x1100 or (reference_zone & 0x4000 and reference_x > 0.):
            if not focus_zone & 0x20:
                flags = 1
        elif side < 0:
            flags = 0x10 if (reference_zone | focus_zone) & 0x20 else 0x11
        elif side > 0:
            flags = 0x10 if (reference_zone | focus_zone) & 0x10 else 0x12
    if focus_zone & 0x80:
        flags |= 8
    elif special_game_state:
        if not focus_zone & 0x48:
            flags |= 4 if alternate_zone & 0x40 else 8 if not alternate_zone & 4 else 0x20
    elif alternate_zone & 0x40:
        if not focus_zone & 8:
            flags |= 4
    elif focus_zone & 0x40:
        flags |= 4
    elif not (focus_zone | alternate_zone) & 4:
        flags |= 0x28
    else:
        flags |= 0x20
    return flags | 0x40


def inspect_capture(capture):
    """Check actual stored EE values without treating live reads as atomic ticks."""
    samples = capture['samples']
    basis_checks = []
    source_checks = []
    for sample in samples:
        camera = sample.get('active_camera', {})
        if camera.get('vtable') not in (0x4985b0, 0x498950, 0x498dd0):
            continue
        if camera.get('yaw_extra') or 'basis_forward' not in camera:
            continue
        pitch, yaw = camera['pitch'], camera['yaw']
        sy, cy, sp, cp = math.sin(yaw), math.cos(yaw), math.sin(pitch), math.cos(pitch)
        expected = {'basis_forward': [sy * cp, -sp, cy * cp],
                    'basis_right': [cy, 0., -sy],
                    'basis_up': [sy * sp, cp, cy * sp]}
        basis_checks.append(max(abs(expected[k][i] - camera[k][i])
                                for k in expected for i in range(3)))
    for before, after in zip(samples, samples[1:]):
        if not before.get('row_camera_active') or not after.get('row_camera_active'):
            continue
        old, new = before['row_camera'], after['row_camera']
        old_actor, actor = before.get('player'), after.get('player')
        if old['ownerless'] or new['ownerless'] or not old_actor or not actor:
            continue
        if old_actor['address'] != actor['address'] or old['address'] != new['address']:
            continue
        delta = [new['focus_current'][i] - old['focus_current'][i] for i in range(3)]
        if not any(delta) or max(abs(delta[i] - new['motion'][i]) for i in range(3)) > 1e-7:
            continue
        if max(abs(actor['position'][i] - new['focus_current'][i]) for i in range(3)) > 1e-7:
            continue
        physical = actor['motion'][:3]
        actor_delta = [actor['position'][i] - old_actor['position'][i] for i in range(3)]
        source_checks.append({'delta': delta, 'physical_motion': physical,
                              'difference': max(abs(actor_delta[i] - physical[i]) for i in range(3))})
    return {'sample_count': len(samples),
            'row_active_samples': sum(bool(s.get('row_camera_active')) for s in samples),
            'basis_checks': len(basis_checks), 'basis_max_error': max(basis_checks, default=None),
            'adjacent_held_row_delta_checks': len(source_checks),
            'physical_motion_matches': sum(s['difference'] <= 5e-6 for s in source_checks),
            'root_motion_with_zero_physical': sum(not any(s['physical_motion']) for s in source_checks),
            'limitation': 'Filtering is value consistency, not a guarantee of adjacent or atomic EE ticks.'}


def predict(snapshot):
    """State-0x12 ordinary finite arithmetic with explicit native inputs."""
    if 'record' in snapshot:
        return integrate(snapshot, list(map(f32, snapshot['record'])))
    mode = snapshot['mode']
    if mode == 4:
        raise ValueError('Behind coordinate rotation and framing are not implemented')
    bound, offset, height, scale, depth, scalar = ROWS[mode]
    eye = list(map(f32, snapshot['eye']))
    focus = list(map(f32, snapshot['focus']))
    motion = list(map(f32, snapshot['motion']))
    velocity = list(map(f32, snapshot['velocity']))
    pitch, yaw = map(f32, snapshot['angles'])
    side = snapshot['side']
    flags = snapshot['flags']
    transition = snapshot.get('transition', 0)
    count = snapshot.get('count', 1)
    scalar = f32(snapshot.get('scalar', scalar))
    record = [0.] * 13
    record[5] = mul(div(42., snapshot.get('scale', 60.)), .5)
    record[6] = mul(div(26.5, snapshot.get('scale', 60.)), .5)
    record[7] = mul(div(native_atan(scalar), snapshot.get('scale', 60.)), .5)
    record[8] = mul(record[7], f32(4 / 3))
    if mode < 2 and 'depth_offset' not in snapshot:
        raise ValueError('Street/Court require observed depth_offset (+1f4 dereference)')
    depth = f32(snapshot.get('depth_offset', depth))
    if transition and mode < 2:
        depth_step = snapshot['transition_depth_step']
    else:
        depth_step = max(record[6], abs(motion[2]))
    if not snapshot.get('inhibited', False):
        record[1] = approach_step(eye[2], add(mul(focus[2], div(scale, 26.5)), depth), depth_step)
        record[0] = approach_step(eye[0], add(focus[0], mul(side, offset)), max(record[5], abs(motion[0])))
        next_x = add(eye[0], record[0])
        if next_x > bound or next_x < -bound:
            record[0] = sub(max(-bound, min(bound, next_x)), eye[0])
        if motion[0] == 0.:
            record[0] = 0.
        if transition == 0 and motion[2] == 0.:
            record[1] = 0.
        current_vector = [sub(focus[i], eye[i]) for i in range(3)]
        predicted_vector = [sub(add(current_vector[i], motion[i]), command)
                            for i, command in enumerate((record[0], snapshot.get('eye_y_command', 0.), record[1]))]
        old_pitch, old_yaw = angles(current_vector)
        new_pitch, new_yaw = angles(predicted_vector)
        record[2], record[3] = wrap(sub(new_pitch, old_pitch)), wrap(sub(new_yaw, old_yaw))
    # fe780 can replace pitch step and flag4/8 before 2fcd50.
    record[7] = f32(snapshot.get('pitch_step', record[7]))
    if flags & 0x10 and transition == 0:
        record[3] = 0.
    if flags & 0x20 and transition == 0:
        record[2] = 0.
    if flags & 1:
        record[3] = add(record[3], record[8])
    elif flags & 2:
        record[3] = sub(record[3], record[8])
    if flags & 4:
        record[2] = sub(record[2], record[7])
    elif flags & 8:
        record[2] = add(record[2], record[7])
    if snapshot.get('reference_zone', 0) & 0x7c00:
        raise ValueError('Offscreen nonlinear pitch scaling requires explicit record override')
    for index in range(4):
        record[9 + index] = mul(sub(record[index], velocity[index]), .25)
    if not transition:
        for index in (2, 3):
            correction = record[9 + index]
            if velocity[index] == 0. or (velocity[index] >= 0.) == (correction >= 0.):
                record[9 + index] = mul(correction, .5)
    elif mode < 2:
        raise ValueError('Street/Court transition correction scaling requires explicit record override')
    if snapshot.get('pitch_acceleration', False):
        record[11] = mul(sub(record[2], velocity[2]), .125)
    if mode in (0, 5) and snapshot.get('settled', False):
        factor = f32(snapshot['screen_motion_factor'])
        record[0], record[3] = mul(record[0], factor), mul(record[3], factor)
    return integrate(snapshot, record)


def integrate(snapshot, record):
    """FUN_002fda08 finite ordinary integration after optional damping."""
    mode = snapshot['mode']
    if mode == 4:
        raise ValueError('Behind integration is not implemented')
    bound = ROWS[mode][0]
    eye = list(map(f32, snapshot['eye']))
    pitch, yaw = map(f32, snapshot['angles'])
    velocity = list(map(f32, snapshot['velocity']))
    scalar = f32(snapshot.get('scalar', ROWS[mode][5]))
    before_integration = record.copy()
    for _ in range(snapshot.get('count', 1)):
        velocity = [velocity_step(velocity[i], record[i], record[9 + i], i == 3) for i in range(4)]
        eye[0], eye[2] = add(eye[0], velocity[0]), add(eye[2], velocity[1])
        pitch, yaw = add(pitch, velocity[2]), add(yaw, velocity[3])
        if snapshot.get('state', 0x12) == 0x12 and (eye[0] > bound or eye[0] < -bound):
            eye[0] = max(-bound, min(bound, eye[0]))
            record[0] = record[9] = velocity[0] = 0.
        scalar = add(scalar, record[4])
    return {'record': before_integration, 'eye': eye, 'angles': [pitch, yaw],
            'velocity': velocity, 'scalar': scalar, 'count': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshot', help='JSON path, or - for stdin')
    args = parser.parse_args()
    with (sys.stdin if args.snapshot == '-' else open(args.snapshot, encoding='utf-8')) as source:
        snapshot = json.load(source)
    if 'samples' in snapshot:
        result = inspect_capture(snapshot)
    elif snapshot.get('helper') == 'normal_flags':
        result = {'flags': normal_flags(snapshot['focus_zone'], snapshot['reference_zone'],
                                        snapshot['alternate_zone'], snapshot['side'],
                                        snapshot['reference_x'], snapshot.get('special_game_state', False))}
    elif snapshot.get('helper') == 'projection':
        if snapshot.get('yaw_extra', 0.) != 0.:
            raise ValueError('Supply separately rotated basis for nonzero yaw_extra')
        screen = project_basis(snapshot['eye'], snapshot['point'], snapshot['scalar'], snapshot['basis'])
        result = {'screen': screen}
        if 'thresholds' in snapshot:
            top_y = None
            if snapshot.get('margin', 0.):
                high_point = snapshot['point'].copy()
                high_point[1] = add(high_point[1], snapshot['margin'])
                high_screen = project_basis(snapshot['eye'], high_point, snapshot['scalar'], snapshot['basis'])
                if high_screen is not None:
                    top_y = high_screen[1]
            result['zone'] = zone_mask(screen, snapshot['thresholds'], top_y)
    elif snapshot.get('helper') == 'positioned_lens':
        result = positioned_lens(snapshot['scalar'], snapshot['distance_scale'],
                                 snapshot['eye'], snapshot['focus_e0'],
                                 snapshot.get('fps', 60.), snapshot.get('damping', .8))
    else:
        result = predict(snapshot)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
