use std::{collections::HashMap, fmt::Debug, hash::Hash};
use crate::{Error, Parcel};

pub trait Parcelable {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> where Self: Sized;
    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error>;
}

//impl Debug for dyn Parcelable {
    //fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //write!(f, "{:?}", (*self).fmt(f))
    //}
//}
#[derive(Debug)]
pub struct String16(String);

macro_rules! implement_primitve {
    ($ty:ty, $func:ident, $wty:ty, $wfunc:ident) => {
        impl Parcelable for $ty {
            fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> where Self: Sized {
                Ok(parcel.$func()? as $ty)
            }
            fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
                parcel.$wfunc(*self as $wty)?;
                Ok(())
            }
        }
    }
}

implement_primitve!(u8, read_u8, u8, write_u8);
implement_primitve!(i8, read_u8, u8, write_u8);
implement_primitve!(u16, read_u16, u16, write_u16);
implement_primitve!(i16, read_u16, u16, write_u16);
implement_primitve!(i32, read_i32, i32, write_i32);
implement_primitve!(u32, read_u32, u32, write_u32);
implement_primitve!(f32, read_u32, u32, write_u32);
implement_primitve!(f64, read_u64, u64, write_u64);
implement_primitve!(i64, read_u64, u64, write_u64);
implement_primitve!(u64, read_u64, u64, write_u64);
implement_primitve!(usize, read_usize, usize, write_usize);

impl Parcelable for bool {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(parcel.read_i32()? != 0)
    }

    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_i32(if *self { 1 } else { 0 })?;
        Ok(())
    }
}

impl Parcelable for String {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(parcel.read_str()?)
    }
    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_str(self)?;
        Ok(())
    }
}
impl Parcelable for String16 {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(String16(parcel.read_str16()?))
    }
    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_str16(&self.0)?;
        Ok(())
    }
}

impl<T: Parcelable> Parcelable for Option<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let prefix = parcel.read_i32()?;
        Ok(if prefix != 0 && prefix != -1 {
            Some(T::deserialize(parcel)?)
        } else {
            None
        })
    }
    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        if let Some(internal) = self {
            parcel.write_i32(1)?;
            internal.serialize(parcel)?;
        } else {
            parcel.write_i32(0)?;
        };
        Ok(())
    }
}

impl<T: Parcelable> Parcelable for Box<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(Box::new(T::deserialize(parcel)?))
    }

    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        self.as_ref().serialize(parcel)?;
        Ok(())
    }
}

impl<T: Parcelable> Parcelable for Vec<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let len = parcel.read_i32()? as usize;
        let mut res = Vec::with_capacity(len);
        for _ in 0..len {
            res.push(T::deserialize(parcel)?);
        }
        Ok(res)
    }
    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_i32(self.len() as i32)?;
        for val in self {
            val.serialize(parcel)?;
        }
        Ok(())
    }
}

impl<K: Parcelable + Eq + Hash, V: Parcelable> Parcelable for HashMap<K, V> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let len = parcel.read_i32()?;
        let mut res = HashMap::new();
        for _ in 0..len {
            res.insert(K::deserialize(parcel)?, V::deserialize(parcel)?);
        }
        Ok(res)
    }

    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_i32(self.len() as i32)?;
        for (k, v) in self {
            k.serialize(parcel)?;
            v.serialize(parcel)?;
        }
        Ok(())
    }
}
