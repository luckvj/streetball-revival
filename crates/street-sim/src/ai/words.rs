//! The AI's state as a run of 32-bit words, for the world snapshot: every
//! field is an integer, a flag or a float's exact bits, so a world read
//! back steps exactly as the original would.

pub(crate) struct Reader<'a> {
    words: &'a [u32],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(words: &'a [u32]) -> Self {
        Self { words, at: 0 }
    }

    pub fn word(&mut self) -> Result<u32, String> {
        let w = *self
            .words
            .get(self.at)
            .ok_or("AI state cut short".to_string())?;
        self.at += 1;
        Ok(w)
    }

    pub fn done(&self) -> bool {
        self.at == self.words.len()
    }
}

/// A value that is written as words.
pub(crate) trait Word: Sized {
    fn put(&self, out: &mut Vec<u32>);
    fn get(r: &mut Reader) -> Result<Self, String>;
}

impl Word for u32 {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(*self);
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        r.word()
    }
}

impl Word for i32 {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(*self as u32);
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        Ok(r.word()? as i32)
    }
}

impl Word for u8 {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(u32::from(*self));
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        u8::try_from(r.word()?).map_err(|_| "AI byte out of range".to_string())
    }
}

impl Word for i64 {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(*self as u32);
        out.push((*self >> 32) as u32);
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        let lo = u64::from(r.word()?);
        let hi = u64::from(r.word()?);
        Ok((lo | hi << 32) as i64)
    }
}

impl Word for bool {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(u32::from(*self));
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        match r.word()? {
            0 => Ok(false),
            1 => Ok(true),
            k => Err(format!("bad AI flag {k}")),
        }
    }
}

impl Word for f32 {
    fn put(&self, out: &mut Vec<u32>) {
        out.push(self.to_bits());
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        let v = f32::from_bits(r.word()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err("AI float not finite".into())
        }
    }
}

impl<T: Word + Copy + Default, const N: usize> Word for [T; N] {
    fn put(&self, out: &mut Vec<u32>) {
        self.iter().for_each(|v| v.put(out));
    }
    fn get(r: &mut Reader) -> Result<Self, String> {
        let mut a = [T::default(); N];
        for v in &mut a {
            *v = T::get(r)?;
        }
        Ok(a)
    }
}

/// A plain struct of [`Word`] fields, written in declaration order.
macro_rules! words {
    ($(#[$m:meta])* $v:vis struct $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        $v struct $name { $($(#[$fm])* pub $f: $t),* }
        impl $crate::ai::words::Word for $name {
            fn put(&self, out: &mut Vec<u32>) {
                $( $crate::ai::words::Word::put(&self.$f, out); )*
            }
            fn get(r: &mut $crate::ai::words::Reader) -> Result<Self, String> {
                Ok(Self { $($f: $crate::ai::words::Word::get(r)?),* })
            }
        }
    };
}
pub(crate) use words;
