//! PS2 DMA chains and VIF1 command streams, as EAGL models store them.
//!
//! A chain is a sequence of 16-byte DMA tags. Each tag's upper 64 bits are two
//! VIF codes, and CNT/RET/END tags are followed by `qwc` quadwords of VIF
//! stream. The stream UNPACKs vertex data into VU1 memory and then starts the
//! microprogram (MSCAL/MSCNT), which turns one batch into GS primitives.

use std::collections::HashMap;

use crate::{Error, Result, bytes};

/// Longest chain accepted before assuming the data is corrupt.
const MAX_TAGS: usize = 4096;

/// One UNPACK: `count` vectors of `components` x `bits` written to VU memory.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Unpack<'a> {
    /// VU1 address in quadwords (relative to the double-buffer base when FLG is set).
    pub addr: u16,
    pub components: u8,
    /// 32, 16 or 8; 5 for the packed V4-5 format.
    pub bits: u8,
    pub count: usize,
    pub unsigned: bool,
    /// FLG: `addr` is relative to the double-buffer base (TOPS). Without it the
    /// address is absolute, which microprograms use for data kept across batches.
    pub relative: bool,
    /// STROW contents when STMOD selects offset/difference mode, which the
    /// hardware adds to the unpacked values.
    pub offset_row: Option<[u32; 4]>,
    pub data: &'a [u8],
}

pub(crate) enum Event<'a> {
    Unpack(Unpack<'a>),
    /// MSCAL/MSCALF/MSCNT: the batch unpacked so far is drawn.
    Execute,
}

#[derive(Clone, Copy)]
struct State {
    cl: u8,
    wl: u8,
    mode: u8,
    row: [u32; 4],
}

impl Default for State {
    fn default() -> Self {
        Self {
            cl: 1,
            wl: 1,
            mode: 0,
            row: [0; 4],
        }
    }
}

/// Walk the DMA chain at `start`, calling `f` for every UNPACK and every
/// microprogram start. `pointers` maps relocated words (offset -> target),
/// which REF tags use for their data address.
pub(crate) fn walk_chain<'a>(
    data: &'a [u8],
    start: usize,
    pointers: &HashMap<u32, u32>,
    f: &mut impl FnMut(Event<'a>),
) -> Result<()> {
    let mut state = State::default();
    let mut pos = start;
    // Return addresses of CALL tags; the DMA controller keeps two.
    let mut returns: Vec<usize> = Vec::new();
    let target = |pos: usize| -> Result<usize> {
        pointers
            .get(&(pos as u32 + 4))
            .map(|&a| a as usize)
            .ok_or_else(|| Error::format(format!("DMA tag at {pos:#x} has no relocated address")))
    };
    for _ in 0..MAX_TAGS {
        let lo = bytes::u32_le(data, pos)?;
        let qwc = (lo & 0xffff) as usize;
        let id = (lo >> 28) & 7;
        run(&mut state, data, pos + 8, pos + 16, f)?;
        match id {
            // cnt, ret, end: the data follows the tag
            1 | 6 | 7 => {
                let end = pos + 16 + qwc * 16;
                run(&mut state, data, pos + 16, end, f)?;
                pos = match (id, returns.pop()) {
                    (1, _) => end,
                    (6, Some(back)) => back,
                    _ => return Ok(()),
                };
            }
            // next, call: the data follows the tag, then the chain continues at the address
            2 | 5 => {
                let end = pos + 16 + qwc * 16;
                run(&mut state, data, pos + 16, end, f)?;
                if id == 5 {
                    if returns.len() == 2 {
                        return Err(Error::format(format!(
                            "DMA CALL at {pos:#x} nests deeper than two"
                        )));
                    }
                    returns.push(end);
                }
                pos = target(pos)?;
            }
            // refe, ref, refs: the data is elsewhere, the next tag follows
            0 | 3 | 4 => {
                let addr = target(pos)?;
                run(&mut state, data, addr, addr + qwc * 16, f)?;
                if id == 0 {
                    return Ok(());
                }
                pos += 16;
            }
            _ => {
                return Err(Error::format(format!(
                    "unsupported DMA tag id {id} at {pos:#x}"
                )));
            }
        }
    }
    Err(Error::format(format!(
        "DMA chain at {start:#x} longer than {MAX_TAGS} tags"
    )))
}

fn run<'a>(
    s: &mut State,
    data: &'a [u8],
    mut pos: usize,
    end: usize,
    f: &mut impl FnMut(Event<'a>),
) -> Result<()> {
    bytes::range(data, pos, end.saturating_sub(pos))?;
    while pos < end {
        let code = bytes::u32_le(data, pos)?;
        pos += 4;
        let cmd = ((code >> 24) & 0x7f) as u8;
        let num = ((code >> 16) & 0xff) as usize;
        let imm = (code & 0xffff) as usize;

        if cmd & 0x60 == 0x60 {
            let (vn, vl) = ((cmd >> 2) & 3, cmd & 3);
            let written = if num == 0 { 256 } else { num };
            // With WL > CL ("filling") fewer source vectors than written ones are read.
            let count = if s.wl > s.cl && s.cl > 0 {
                let (cl, wl) = (s.cl as usize, s.wl as usize);
                cl * (written / wl) + (written % wl).min(cl)
            } else {
                written
            };
            let (components, bits) = if vn == 3 && vl == 3 {
                (4, 5)
            } else {
                (vn + 1, 32 >> vl)
            };
            let vector_bits = if bits == 5 {
                16
            } else {
                components as usize * bits as usize
            };
            let len = (count * vector_bits).div_ceil(32) * 4;
            if pos + len > end {
                return Err(Error::format(format!(
                    "UNPACK at {:#x} runs past its packet",
                    pos - 4
                )));
            }
            f(Event::Unpack(Unpack {
                addr: (imm & 0x3ff) as u16,
                components,
                bits,
                count,
                unsigned: imm & 0x4000 != 0,
                relative: imm & 0x8000 != 0,
                offset_row: (s.mode != 0).then_some(s.row),
                data: bytes::range(data, pos, len)?,
            }));
            pos += len;
            continue;
        }

        pos += match cmd {
            0x00 | 0x02..=0x04 | 0x06 | 0x07 | 0x10 | 0x11 | 0x13 => 0, // NOP, OFFSET, BASE, ITOP, MSKPATH3, MARK, FLUSH*
            0x01 => {
                s.cl = (imm & 0xff) as u8;
                s.wl = (imm >> 8) as u8;
                0
            }
            0x05 => {
                s.mode = (imm & 3) as u8;
                0
            }
            0x14 | 0x15 | 0x17 => {
                f(Event::Execute);
                0
            }
            0x20 => 4, // STMASK
            0x30 => {
                for (i, r) in s.row.iter_mut().enumerate() {
                    *r = bytes::u32_le(data, pos + 4 * i)?;
                }
                16
            }
            0x31 => 16,                                               // STCOL
            0x4a => (if num == 0 { 256 } else { num }) * 8,           // MPG
            0x50 | 0x51 => (if imm == 0 { 65536 } else { imm }) * 16, // DIRECT, DIRECTHL
            _ => {
                return Err(Error::format(format!(
                    "unknown VIF command {cmd:#04x} at {:#x}",
                    pos - 4
                )));
            }
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(ws: &[u32]) -> Vec<u8> {
        ws.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn walks_a_ret_chain_and_sizes_unpacks() {
        // RET tag with 2 quadwords; NOP NOP; UNPACK V3-32 x2 to 0x17 (24 bytes); MSCNT; NOP
        let mut d = words(&[0x6000_0002, 0, 0, 0]);
        d.extend(words(&[0x6802_c017]));
        d.extend(words(&[1, 2, 3, 4, 5, 6]));
        d.extend(words(&[0x1700_0000]));
        assert_eq!(d.len(), 16 + 32);
        let mut seen = Vec::new();
        walk_chain(&d, 0, &HashMap::new(), &mut |e| match e {
            Event::Unpack(u) => seen.push(format!(
                "unpack {:#x} {}x{}x{} {}",
                u.addr,
                u.count,
                u.components,
                u.bits,
                u.data.len()
            )),
            Event::Execute => seen.push("exec".into()),
        })
        .unwrap();
        assert_eq!(seen, ["unpack 0x17 2x3x32 24", "exec"]);
    }

    #[test]
    fn rejects_unknown_commands() {
        let d = words(&[0x6000_0001, 0, 0x3f00_0000, 0, 0, 0, 0, 0]);
        assert!(walk_chain(&d, 0, &HashMap::new(), &mut |_| {}).is_err());
    }
}
