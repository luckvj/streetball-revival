//! PlayStation pads read straight from their HID reports on Windows: the
//! DualSense (and its Edge) and the DualShock 4, over USB or Bluetooth.
//!
//! Bevy's gilrs backend reads pads through Windows.Gaming.Input, which
//! hands Xbox-style pads over as gamepads but may never list a Sony pad to
//! a desktop game at all (a DualSense on USB went unseen while other games
//! took it). Here each Sony pad Windows lists as a HID device gets a
//! reader thread, and its reports become the same gamepad events gilrs
//! sends, named and numbered as a Sony pad so the prompts use PlayStation
//! names ([`crate::pads::ButtonStyle`]). A Sony pad gilrs itself connects
//! takes over: this one steps aside, so no pad drives two seats. The
//! reports count only while the game's window has focus, as gilrs's do.
//! Vibration ([`crate::pads`]) goes out in the pad's own output report
//! over USB (the Bluetooth reports' checksums are not written).

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};

use bevy::{
    input::gamepad::{
        GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent,
        GamepadRumbleRequest, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        RawGamepadEvent,
    },
    prelude::*,
    window::Window,
};

/// Sony's USB vendor id.
const SONY: u16 = 0x054c;
/// The pads read here, by product id.
const PADS: [(u16, &str); 5] = [
    (0x0ce6, "DualSense Wireless Controller"),
    (0x0df2, "DualSense Edge Wireless Controller"),
    (0x05c4, "DUALSHOCK 4 Wireless Controller"),
    (0x09cc, "DUALSHOCK 4 Wireless Controller"),
    (0x0ba0, "DUALSHOCK 4 USB Wireless Adaptor"),
];
/// How often the HID devices are listed again for a pad plugged in.
const RESCAN: Duration = Duration::from_secs(2);

/// One report's worth of a pad: sticks -1..1 (up positive), triggers
/// 0..1, buttons in [`BUTTONS`]' order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub sticks: [f32; 4],
    pub triggers: [f32; 2],
    pub buttons: [bool; BUTTONS.len()],
}

/// The buttons a report carries, as Bevy names them.
pub const BUTTONS: [GamepadButton; 15] = [
    GamepadButton::West,         // Square
    GamepadButton::South,        // Cross
    GamepadButton::East,         // Circle
    GamepadButton::North,        // Triangle
    GamepadButton::LeftTrigger,  // L1
    GamepadButton::RightTrigger, // R1
    GamepadButton::Select,       // Create / Share
    GamepadButton::Start,        // Options
    GamepadButton::LeftThumb,    // L3
    GamepadButton::RightThumb,   // R3
    GamepadButton::Mode,         // PS
    GamepadButton::DPadUp,
    GamepadButton::DPadRight,
    GamepadButton::DPadDown,
    GamepadButton::DPadLeft,
];

/// A stick byte (0..255, 128 the centre) as -1..1.
fn stick(b: u8) -> f32 {
    ((f32::from(b) - 128.0) / 127.0).clamp(-1.0, 1.0)
}

/// The layout every Sony report shares from its sticks on: four stick
/// bytes, then (`dualsense`) the triggers, a counter and the button bytes,
/// or (the DualShock 4, and the DualSense's short Bluetooth report) the
/// button bytes, then the triggers.
fn read(r: &[u8], dualsense: bool) -> Option<State> {
    let (face, shoulder, ps, l2, r2) = if dualsense {
        (*r.get(7)?, *r.get(8)?, *r.get(9)?, *r.get(4)?, *r.get(5)?)
    } else {
        (*r.get(4)?, *r.get(5)?, *r.get(6)?, *r.get(7)?, *r.get(8)?)
    };
    // The hat: 0 up, clockwise in eighths, 8 none.
    let hat = face & 0x0f;
    let dir = |d: [u8; 3]| d.contains(&hat);
    Some(State {
        sticks: [stick(r[0]), -stick(r[1]), stick(r[2]), -stick(r[3])],
        triggers: [f32::from(l2) / 255.0, f32::from(r2) / 255.0],
        buttons: [
            face & 0x10 != 0,
            face & 0x20 != 0,
            face & 0x40 != 0,
            face & 0x80 != 0,
            shoulder & 0x01 != 0,
            shoulder & 0x02 != 0,
            shoulder & 0x10 != 0,
            shoulder & 0x20 != 0,
            shoulder & 0x40 != 0,
            shoulder & 0x80 != 0,
            ps & 0x01 != 0,
            dir([7, 0, 1]),
            dir([1, 2, 3]),
            dir([3, 4, 5]),
            dir([5, 6, 7]),
        ],
    })
}

/// One input report from pad `product`, its id byte first; `bluetooth`
/// when its reports are longer than USB's 64 bytes (Windows pads every
/// report to the longest).
pub fn parse(product: u16, bluetooth: bool, report: &[u8]) -> Option<State> {
    let dualsense = matches!(product, 0x0ce6 | 0x0df2);
    match (report.first()?, dualsense) {
        // USB, and the DualSense's full Bluetooth report one byte on.
        (0x01, true) if !bluetooth => read(&report[1..], true),
        (0x31, true) => read(report.get(2..)?, true),
        // The short Bluetooth report both send before they are woken, and
        // the DualShock 4's USB report.
        (0x01, _) => read(&report[1..], false),
        // The DualShock 4's full Bluetooth report.
        (0x11, false) => read(report.get(3..)?, false),
        _ => None,
    }
}

/// The motors' levels a reader writes out: strong (left), weak (right).
type Motors = Arc<Mutex<[u8; 2]>>;

/// What a reader thread tells the game.
enum Message {
    Opened(String, u16, Motors),
    Report(String, State),
    Gone(String),
}

/// A pad being read, connected as a gamepad.
struct Pad {
    entity: Entity,
    last: State,
    motors: Motors,
    /// Vibrations running: until when, strong, weak (0..1).
    rumbles: Vec<(Instant, f32, f32)>,
}

/// The readers' channel, the pads open and the ones connected.
#[derive(Resource)]
pub struct SonyPads {
    rx: Mutex<Receiver<Message>>,
    open: HashMap<String, (u16, Motors)>,
    pads: HashMap<String, Pad>,
}

pub struct SonyHidPlugin;

impl Plugin for SonyHidPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("sony-hid".into())
            .spawn(move || scan(tx))
            .ok();
        app.insert_resource(SonyPads {
            rx: Mutex::new(rx),
            open: HashMap::new(),
            pads: HashMap::new(),
        })
        .add_systems(PreUpdate, pump.before(bevy::input::InputSystems))
        .add_systems(PostUpdate, rumble);
    }
}

/// The reports since the last frame into gamepad events, for the pads
/// gilrs has not connected itself.
#[allow(clippy::too_many_arguments)]
fn pump(
    mut commands: Commands,
    mut sony: ResMut<SonyPads>,
    gamepads: Query<(Entity, &Gamepad)>,
    windows: Query<&Window>,
    mut raw: MessageWriter<RawGamepadEvent>,
    mut connections: MessageWriter<GamepadConnectionEvent>,
    mut buttons: MessageWriter<RawGamepadButtonChangedEvent>,
    mut axes: MessageWriter<RawGamepadAxisChangedEvent>,
) {
    let sony = &mut *sony;
    let ours: HashSet<Entity> = sony.pads.values().map(|p| p.entity).collect();
    // A Sony pad gilrs reads: this one steps aside.
    let gilrs_has_sony = gamepads
        .iter()
        .any(|(e, g)| !ours.contains(&e) && g.vendor_id() == Some(SONY));
    let focused = windows.iter().any(|w| w.focused);
    let messages: Vec<Message> = sony
        .rx
        .lock()
        .map(|rx| rx.try_iter().collect())
        .unwrap_or_default();
    let mut connect = |raw: &mut MessageWriter<RawGamepadEvent>,
                       entity: Entity,
                       connection: GamepadConnection| {
        let event = GamepadConnectionEvent::new(entity, connection);
        raw.write(event.clone().into());
        connections.write(event);
    };
    if gilrs_has_sony {
        for (_, pad) in sony.pads.drain() {
            connect(&mut raw, pad.entity, GamepadConnection::Disconnected);
        }
    }
    for message in messages {
        let (path, state) = match message {
            Message::Opened(path, product, motors) => {
                sony.open.insert(path, (product, motors));
                continue;
            }
            Message::Gone(path) => {
                sony.open.remove(&path);
                if let Some(pad) = sony.pads.remove(&path) {
                    connect(&mut raw, pad.entity, GamepadConnection::Disconnected);
                }
                continue;
            }
            Message::Report(path, state) => (path, state),
        };
        if gilrs_has_sony {
            continue;
        }
        let Some((product, motors)) = sony.open.get(&path).cloned() else {
            continue;
        };
        let pad = sony.pads.entry(path).or_insert_with(|| {
            let entity = commands.spawn_empty().id();
            let name = PADS
                .iter()
                .find(|p| p.0 == product)
                .map_or("PlayStation Controller", |p| p.1);
            info!("controller read over HID: {name}");
            connect(
                &mut raw,
                entity,
                GamepadConnection::Connected {
                    name: name.to_string(),
                    vendor_id: Some(SONY),
                    product_id: Some(product),
                },
            );
            Pad {
                entity,
                last: State::default(),
                motors,
                rumbles: Vec::new(),
            }
        });
        if !focused {
            continue;
        }
        let (entity, last) = (pad.entity, pad.last);
        let mut changed = Vec::new();
        for (k, &b) in BUTTONS.iter().enumerate() {
            if state.buttons[k] != last.buttons[k] {
                changed.push((b, if state.buttons[k] { 1.0 } else { 0.0 }));
            }
        }
        for (k, b) in [GamepadButton::LeftTrigger2, GamepadButton::RightTrigger2]
            .into_iter()
            .enumerate()
        {
            if state.triggers[k] != last.triggers[k] {
                changed.push((b, state.triggers[k]));
            }
        }
        for (b, v) in changed {
            let e = RawGamepadButtonChangedEvent::new(entity, b, v);
            raw.write(e.into());
            buttons.write(e);
        }
        let sticks = [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ];
        for (k, axis) in sticks.into_iter().enumerate() {
            if state.sticks[k] != last.sticks[k] {
                let e = RawGamepadAxisChangedEvent::new(entity, axis, state.sticks[k]);
                raw.write(e.into());
                axes.write(e);
            }
        }
        pad.last = state;
    }
}

/// The vibration asked of the pads read here, as the motors' levels their
/// readers write out.
fn rumble(mut sony: ResMut<SonyPads>, mut requests: MessageReader<GamepadRumbleRequest>) {
    let now = Instant::now();
    for request in requests.read() {
        let Some(pad) = sony
            .pads
            .values_mut()
            .find(|p| p.entity == request.gamepad())
        else {
            continue;
        };
        match request {
            GamepadRumbleRequest::Add {
                duration,
                intensity,
                ..
            } => pad.rumbles.push((
                now + *duration,
                intensity.strong_motor,
                intensity.weak_motor,
            )),
            GamepadRumbleRequest::Stop { .. } => pad.rumbles.clear(),
        }
    }
    for pad in sony.pads.values_mut() {
        pad.rumbles.retain(|r| r.0 > now);
        let level = |strong: bool| {
            let v = pad
                .rumbles
                .iter()
                .map(|r| if strong { r.1 } else { r.2 })
                .fold(0.0_f32, f32::max);
            (v.clamp(0.0, 1.0) * 255.0) as u8
        };
        let levels = [level(true), level(false)];
        if let Ok(mut m) = pad.motors.lock() {
            *m = levels;
        }
    }
}

/// Lists the Sony pads every [`RESCAN`] and starts a reader for each new
/// one.
fn scan(tx: Sender<Message>) {
    let reading = Arc::new(Mutex::new(HashSet::<String>::new()));
    loop {
        for (path, product) in hid::sony_pads() {
            let fresh = reading.lock().is_ok_and(|mut r| r.insert(path.clone()));
            if !fresh {
                continue;
            }
            let (tx, reading) = (tx.clone(), reading.clone());
            let motors: Motors = Arc::default();
            if tx
                .send(Message::Opened(path.clone(), product, motors.clone()))
                .is_err()
            {
                return;
            }
            std::thread::Builder::new()
                .name("sony-pad".into())
                .spawn(move || {
                    hid::read_pad(&path, product, &motors, |report, bluetooth| {
                        match parse(product, bluetooth, report) {
                            Some(state) => tx.send(Message::Report(path.clone(), state)).is_ok(),
                            None => true,
                        }
                    });
                    let _ = tx.send(Message::Gone(path.clone()));
                    if let Ok(mut r) = reading.lock() {
                        r.remove(&path);
                    }
                })
                .ok();
        }
        std::thread::sleep(RESCAN);
    }
}

/// The output report setting pad `product`'s motors over USB, `length`
/// long: the DualSense's report 2 (its compatible vibration and the
/// haptics' select, then the right and left motors), the DualShock 4's
/// report 5 (the motors' flag alone, so its light bar stays as it is).
fn motors_report(product: u16, length: usize, [strong, weak]: [u8; 2]) -> Vec<u8> {
    let mut out = vec![0u8; length.max(8)];
    if matches!(product, 0x0ce6 | 0x0df2) {
        out[0] = 0x02;
        out[1] = 0x03;
        out[3] = weak;
        out[4] = strong;
    } else {
        out[0] = 0x05;
        out[1] = 0x01;
        out[4] = weak;
        out[5] = strong;
    }
    out
}

/// The Win32 side: listing HID devices and reading one.
mod hid {
    use windows::{
        Win32::{
            Devices::{
                DeviceAndDriverInstallation::{
                    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
                    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
                    SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
                    SetupDiGetDeviceInterfaceDetailW,
                },
                HumanInterfaceDevice::{
                    HIDD_ATTRIBUTES, HIDP_CAPS, HidD_FreePreparsedData, HidD_GetAttributes,
                    HidD_GetFeature, HidD_GetHidGuid, HidD_GetPreparsedData, HidP_GetCaps,
                    PHIDP_PREPARSED_DATA,
                },
            },
            Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE},
            Storage::FileSystem::{
                CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
                OPEN_EXISTING, ReadFile, WriteFile,
            },
        },
        core::PCWSTR,
    };

    /// An open HID device, closed on drop.
    struct Device(HANDLE);

    impl Drop for Device {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateFileW and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn open(path: &str, access: u32) -> Option<Device> {
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated path that outlives the call.
        unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .ok()
        .map(Device)
    }

    /// Every present HID device's path with Sony's vendor id and a pad's
    /// product id.
    pub fn sony_pads() -> Vec<(String, u16)> {
        let mut out = Vec::new();
        // SAFETY: SetupAPI enumeration; every buffer is sized as the API
        // asks, and the list is destroyed at the end.
        unsafe {
            let guid = HidD_GetHidGuid();
            let Ok(set) = SetupDiGetClassDevsW(
                Some(&guid),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            ) else {
                return out;
            };
            let mut index = 0;
            loop {
                let mut data = SP_DEVICE_INTERFACE_DATA {
                    cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(set, None, &guid, index, &mut data).is_err() {
                    break;
                }
                index += 1;
                let mut size = 0u32;
                let _ =
                    SetupDiGetDeviceInterfaceDetailW(set, &data, None, 0, Some(&mut size), None);
                if size < 8 {
                    continue;
                }
                // u32-aligned storage for the variable-length detail.
                let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
                let detail = buffer
                    .as_mut_ptr()
                    .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
                (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &data, Some(detail), size, None, None)
                    .is_err()
                {
                    continue;
                }
                let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
                let chars = (size as usize - 4) / 2;
                let slice = std::slice::from_raw_parts(start, chars);
                let end = slice.iter().position(|&c| c == 0).unwrap_or(chars);
                let path = String::from_utf16_lossy(&slice[..end]);
                // No access asked: enough to read the attributes.
                let Some(device) = open(&path, 0) else {
                    continue;
                };
                let mut attributes = HIDD_ATTRIBUTES {
                    Size: size_of::<HIDD_ATTRIBUTES>() as u32,
                    ..Default::default()
                };
                if HidD_GetAttributes(device.0, &mut attributes)
                    && attributes.VendorID == super::SONY
                    && super::PADS.iter().any(|p| p.0 == attributes.ProductID)
                {
                    out.push((path, attributes.ProductID));
                }
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        out
    }

    /// Reads `path`'s input reports into `report` until the pad is gone or
    /// `report` says stop, writing `motors` out between them when they
    /// change (over USB).
    pub fn read_pad(
        path: &str,
        product: u16,
        motors: &super::Motors,
        mut report: impl FnMut(&[u8], bool) -> bool,
    ) {
        let Some(device) =
            open(path, GENERIC_READ.0 | GENERIC_WRITE.0).or_else(|| open(path, GENERIC_READ.0))
        else {
            return;
        };
        // SAFETY: the preparsed data is freed once read; every buffer is as
        // long as its call is told.
        unsafe {
            let mut length = 64usize;
            let mut out_length = 0usize;
            let mut preparsed = PHIDP_PREPARSED_DATA::default();
            if HidD_GetPreparsedData(device.0, &mut preparsed) {
                let mut caps = HIDP_CAPS::default();
                if HidP_GetCaps(preparsed, &mut caps).is_ok() {
                    length = usize::from(caps.InputReportByteLength).max(16);
                    out_length = usize::from(caps.OutputReportByteLength);
                }
                let _ = HidD_FreePreparsedData(preparsed);
            }
            // Over Bluetooth a pad sends its short report until its
            // calibration is read (feature report 5; 2 on a DualShock 4).
            for id in [0x05u8, 0x02] {
                let mut feature = [0u8; 64];
                feature[0] = id;
                let _ =
                    HidD_GetFeature(device.0, feature.as_mut_ptr().cast(), feature.len() as u32);
            }
            let bluetooth = length > 64;
            let mut buffer = vec![0u8; length];
            let mut written = [0u8; 2];
            loop {
                let mut read = 0u32;
                if ReadFile(device.0, Some(&mut buffer), Some(&mut read), None).is_err()
                    || read == 0
                {
                    return;
                }
                if !report(&buffer[..read as usize], bluetooth) {
                    return;
                }
                let want = motors.lock().map_or(written, |m| *m);
                if want != written && !bluetooth && out_length > 0 {
                    let out = super::motors_report(product, out_length, want);
                    let _ = WriteFile(device.0, Some(&out[..out_length]), None, None);
                    written = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressed(s: &State) -> Vec<GamepadButton> {
        BUTTONS
            .iter()
            .zip(s.buttons)
            .filter(|(_, b)| *b)
            .map(|(&b, _)| b)
            .collect()
    }

    #[test]
    fn a_dualsenses_usb_report_reads_as_the_pads_buttons_and_sticks() {
        // Report 1: the left stick full right, the right full up, L2 half,
        // the hat right, Cross and Triangle, R1 and Options, PS.
        let mut r = [0u8; 64];
        r[0] = 0x01;
        r[1..5].copy_from_slice(&[255, 128, 128, 0]);
        r[5] = 128;
        r[8] = 0x02 | 0x20 | 0x80;
        r[9] = 0x02 | 0x20;
        r[10] = 0x01;
        let s = parse(0x0ce6, false, &r).unwrap();
        assert_eq!(s.sticks, [1.0, 0.0, 0.0, 1.0]);
        assert!((s.triggers[0] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(
            pressed(&s),
            [
                GamepadButton::South,
                GamepadButton::North,
                GamepadButton::RightTrigger,
                GamepadButton::Start,
                GamepadButton::Mode,
                GamepadButton::DPadRight,
            ]
        );
    }

    #[test]
    fn the_bluetooth_reports_and_the_dualshock_4_read_alike() {
        // The DualShock 4's USB report: the button bytes after the sticks,
        // then the triggers.
        let mut ds4 = [0u8; 64];
        ds4[0] = 0x01;
        ds4[1..5].copy_from_slice(&[128, 0, 128, 128]);
        ds4[5] = 0x08 | 0x40; // hat none, Circle
        ds4[6] = 0x01; // L1
        ds4[9] = 255; // R2
        let s = parse(0x09cc, false, &ds4).unwrap();
        assert_eq!(s.sticks[1], 1.0);
        assert_eq!(s.triggers, [0.0, 1.0]);
        assert_eq!(
            pressed(&s),
            [GamepadButton::East, GamepadButton::LeftTrigger]
        );
        // The DualSense's short Bluetooth report, padded to the long one's
        // length, reads the same way.
        let mut short = [0u8; 78];
        short[..10].copy_from_slice(&ds4[..10]);
        assert_eq!(parse(0x0ce6, true, &short), Some(s));
        // Its long one is USB's, one byte on.
        let mut usb = [0u8; 64];
        usb[0] = 0x01;
        usb[1..5].copy_from_slice(&[10, 20, 30, 40]);
        usb[8] = 0x10 | 0x04;
        let mut bt = [0u8; 78];
        bt[0] = 0x31;
        bt[2..65].copy_from_slice(&usb[1..64]);
        assert_eq!(parse(0x0ce6, false, &usb), parse(0x0ce6, true, &bt));
        assert_eq!(
            pressed(&parse(0x0ce6, true, &bt).unwrap()),
            [GamepadButton::West, GamepadButton::DPadDown]
        );
    }

    /// The Sony pads plugged in, listed and read for a few reports
    /// (opt-in: `--ignored --nocapture`, with a pad on USB or Bluetooth).
    #[test]
    #[ignore]
    fn the_sony_pads_plugged_in_read() {
        let pads = hid::sony_pads();
        println!("pads: {pads:?}");
        for (path, product) in pads {
            let motors = Motors::default();
            let mut n = 0;
            hid::read_pad(&path, product, &motors, |report, bluetooth| {
                n += 1;
                println!(
                    "{} bytes, id {:#x}, bluetooth {bluetooth}: {:?}",
                    report.len(),
                    report[0],
                    parse(product, bluetooth, report)
                );
                n < 5
            });
            assert!(n > 0, "{path}: no report");
        }
    }

    #[test]
    fn the_motors_go_in_each_pads_own_output_report() {
        let ds = motors_report(0x0ce6, 48, [200, 100]);
        assert_eq!((ds.len(), ds[0], ds[3], ds[4]), (48, 0x02, 100, 200));
        let ds4 = motors_report(0x09cc, 32, [200, 100]);
        assert_eq!((ds4[0], ds4[1], ds4[4], ds4[5]), (0x05, 0x01, 100, 200));
    }
}
