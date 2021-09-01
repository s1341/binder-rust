use std::{collections::HashMap, fmt::Debug, hash::Hash};
use crate::{Error, Parcel};

pub trait Parcelable {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> where Self: Sized;
}

//impl Debug for dyn Parcelable {
    //fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //write!(f, "{:?}", (*self).fmt(f))
    //}
//}
#[derive(Debug)]
pub struct String16(String);

macro_rules! implement_primitve {
    ($ty:ty, $func:ident) => {
        impl Parcelable for $ty {
            fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> where Self: Sized {
                Ok(parcel.$func() as $ty)
            }
        }
    }
}

implement_primitve!(u8, read_u8);
implement_primitve!(i8, read_u8);
implement_primitve!(u16, read_u16);
implement_primitve!(i16, read_u16);
implement_primitve!(i32, read_i32);
implement_primitve!(u32, read_u32);
implement_primitve!(f32, read_u32);
implement_primitve!(f64, read_u64);
implement_primitve!(i64, read_u64);
implement_primitve!(u64, read_u64);
implement_primitve!(usize, read_usize);

impl Parcelable for bool {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(parcel.read_i32() != 0)
    }
}

impl Parcelable for String {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(parcel.read_str())
    }
}
impl Parcelable for String16 {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(String16(parcel.read_str16()))
    }
}

impl<T: Parcelable> Parcelable for Option<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let prefix = parcel.read_i32();
        Ok(if prefix != 0 && prefix != -1 {
            Some(T::deserialize(parcel)?)
        } else {
            None
        })
    }
}

impl<T: Parcelable> Parcelable for Box<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        Ok(Box::new(T::deserialize(parcel)?))
    }
}

impl<T: Parcelable> Parcelable for Vec<T> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let len = parcel.read_i32() as usize;
        let mut res = Vec::with_capacity(len);
        for _ in 0..len {
            res.push(T::deserialize(parcel)?);
        }
        Ok(res)
    }
}

impl<K: Parcelable + Eq + Hash, V: Parcelable> Parcelable for HashMap<K, V> {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> {
        let len = parcel.read_i32();
        let mut res = HashMap::new();
        for _ in 0..len {
            res.insert(K::deserialize(parcel)?, V::deserialize(parcel)?);
        }
        Ok(res)
    }
}
