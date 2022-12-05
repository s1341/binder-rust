use std::{
    ffi::c_void,
    fmt,
    io::{Cursor, Read, Write},
    mem::size_of,
    mem::transmute,
    os::unix::io::RawFd,
    slice,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{Binder, BinderFlatObject, BinderTransactionData, BinderType, Error, Parcelable};

const STRICT_MODE_PENALTY_GATHER: i32 = 1 << 31;
/// The header marker, packed["S", "Y", "S", "T"];
const HEADER: i32 = 0x53595354;

/// Represents a binder serializable parcel
pub struct Parcel {
    cursor: Cursor<Vec<u8>>,
    object_offsets: Vec<usize>,
    objects_position: usize,
}

impl fmt::Debug for Parcel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parcel")
            .field("data", &self.cursor.get_ref())
            .field("offsets", &self.object_offsets)
            .finish()
    }
}
impl Parcel {
    /// Create a new empty parcel.
    pub fn empty() -> Self {
        let data = vec![];
        Self {
            cursor: Cursor::new(data),
            object_offsets: vec![],
            objects_position: 0,
        }
    }

    /// Create a new empty parcel, with a reserved size
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            cursor: Cursor::new(data.to_vec()),
            object_offsets: vec![],
            objects_position: 0,
        }
    }

    pub unsafe fn from_data_and_offsets(
        data: *mut u8,
        data_size: usize,
        offsets: *mut usize,
        offsets_size: usize,
    ) -> Self {
        Self {
            cursor: Cursor::new(slice::from_raw_parts(data, data_size).to_vec()),
            object_offsets: slice::from_raw_parts(offsets, offsets_size).to_vec(),
            objects_position: 0,
        }
    }

    pub fn reset(&mut self) {
        self.cursor.set_position(0);
        self.cursor.get_mut().clear();
        self.objects_position = 0;
        self.object_offsets.clear();
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    pub fn set_position(&mut self, pos: u64) {
        self.cursor.set_position(pos)
    }

    /// Append the contents of another parcel to this parcel
    pub fn append_parcel(&mut self, other: &mut Parcel) -> Result<(), Error> {
        let current_position = self.cursor.position();
        self.cursor.write_all(other.to_slice())?;
        for offset in &other.object_offsets {
            self.object_offsets.push(offset + current_position as usize);
        }
        Ok(())
    }

    /// Retrieve the data of the parcel as a pointer
    pub fn as_ptr(&self) -> *const u8 {
        self.cursor.get_ref().as_ptr()
    }

    /// Retrieve the data of the parcel as a mutable pointer
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.cursor.get_mut().as_mut_ptr()
    }

    /// Retrieve the data of the parce as a slice
    pub fn to_slice(&self) -> &[u8] {
        self.cursor.get_ref()
    }
    /// Retrieve the size of the parcel's data
    pub fn len(&self) -> usize {
        self.cursor.get_ref().len()
    }

    /// Check if this Parcel is empty.
    pub fn is_empty(&self) -> bool {
        self.cursor.get_ref().is_empty()
    }

    /// Retrieve the number of object offsets
    pub fn offsets_len(&self) -> usize {
        self.object_offsets.len()
    }

    /// Retrieve the object offsets
    pub fn offsets(&mut self) -> &mut Vec<usize> {
        &mut self.object_offsets
    }

    pub fn push_object(&mut self) -> Result<(), Error> {
        self.object_offsets.push(self.cursor.position() as usize);
        Ok(())
    }

    /// Check if the parcel has unread data
    pub fn has_unread_data(&self) -> bool {
        self.cursor.position() != self.len() as u64
    }

    /// Write an i32 to the parcel
    pub fn write_i32(&mut self, data: i32) -> Result<(), Error> {
        self.cursor.write_i32::<LittleEndian>(data)?;
        Ok(())
    }
    /// Write an u32 to the parcel
    pub fn write_u32(&mut self, data: u32) -> Result<(), Error> {
        self.cursor.write_u32::<LittleEndian>(data)?;
        Ok(())
    }
    /// Write an u64 to the parcel
    pub fn write_u64(&mut self, data: u64) -> Result<(), Error> {
        self.cursor.write_u64::<LittleEndian>(data)?;
        Ok(())
    }
    /// Write an u16 to the parcel
    pub fn write_u16(&mut self, data: u16) -> Result<(), Error> {
        self.cursor.write_u16::<LittleEndian>(data)?;
        Ok(())
    }

    /// Write a bool to the parcel
    pub fn write_bool(&mut self, data: bool) -> Result<(), Error> {
        self.write_u32(data as u32)?;
        Ok(())
    }

    /// Write an u8 to the parcel
    pub fn write_u8(&mut self, data: u8) -> Result<(), Error>{
        self.cursor.write_u8(data as u8)?;
        Ok(())
    }

    /// Write an usize to the parcel
    pub fn write_usize(&mut self, data: usize) -> Result<(), Error> {
        self.cursor.write_u64::<LittleEndian>(data as u64)?;
        Ok(())
    }


    /// Write a slice of data to the parcel
    pub fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        let padded_len = (data.len() + 3) & !3;

        let mut data = data.to_vec();
        if padded_len > data.len() {
            data.resize(padded_len, 0);
        };

        self.cursor
            .write(data.as_slice())?;

        Ok(())
    }

    /// Write a BinderTransactionData struct into the parcel
    pub fn write_transaction_data(&mut self, data: &BinderTransactionData) -> Result<(), Error>{
        self.write(unsafe {
            slice::from_raw_parts(
                data as *const _ as *const u8,
                size_of::<BinderTransactionData>(),
            )
        })?;
        Ok(())
    }

    /// Read an u8 from the parcel
    pub fn read_u8(&mut self) -> Result<u8, Error> {
        Ok(self.cursor.read_u8()?)
    }

    /// Read an u16 from the parcel
    pub fn read_u16(&mut self) -> Result<u16, Error> {
        Ok(self.cursor.read_u16::<LittleEndian>()?)
    }

    /// Read an u32 from the parcel
    pub fn read_u32(&mut self) -> Result<u32, Error> {
        Ok(self.cursor.read_u32::<LittleEndian>()?)
    }

    /// Read an u64 from the parcel
    pub fn read_u64(&mut self) -> Result<u64, Error> {
        Ok(self.cursor.read_u64::<LittleEndian>()?)
    }

    /// Read an usize from the parcel
    pub fn read_usize(&mut self) -> Result<usize, Error> {
        if size_of::<usize>() == size_of::<u32>() {
            Ok(self.read_u32()? as usize)
        } else {
            Ok(self.read_u64()? as usize)
        }
    }

    /// Read an i32 from the parcel
    pub fn read_i32(&mut self) -> Result<i32, Error> {
        Ok(self.cursor.read_i32::<LittleEndian>()?)
    }

    /// Read a void pointer from the parcel
    pub fn read_pointer(&mut self) -> Result<*const c_void, Error> {
        Ok(self.read_usize()? as *const c_void)
    }

    /// Read a slice of size bytes from the parcel
    pub fn read(&mut self, size: usize) -> Result<Vec<u8>, Error> {
        let size = if (size % 4) != 0 {
            size + 4 - (size % 4)
        } else {
            size
        };
        let mut data = vec![0u8; size];
        self.cursor.read(&mut data)?;
        Ok(data)
    }

    /// Read a slice of size bytes from the parcel
    pub fn read_without_alignment(&mut self, size: usize) -> Result<Vec<u8>, Error> {
        let mut data = vec![0u8; size];
        self.cursor.read(&mut data)?;
        Ok(data)
    }

    /// Read a BinderTransactionData from the parcel
    pub fn read_transaction_data(&mut self) -> Result<BinderTransactionData, Error> {
        Ok(self.read_object()?)
    }

    /// Read an object of type T from the parcel
    pub fn read_object<T>(&mut self) -> Result<T, Error> {
        unsafe {
            let data = slice::from_raw_parts(
                self.cursor
                    .get_ref()
                    .as_ptr()
                    .offset(self.cursor.position() as isize),
                size_of::<T>(),
            );
            self.cursor.set_position(self.cursor.position() + size_of::<T>() as u64);
            Ok((data.as_ptr() as *const T).read())
        }
    }

    pub fn write_object<T>(&mut self, object: T) -> Result<(), Error>{
        self.object_offsets.push(self.cursor.position() as usize);
        self.cursor.write(unsafe {
            slice::from_raw_parts(&object as *const _ as *const u8, size_of::<T>())
        })?;
        Ok(())
    }

    /// Write a string to the parcel
    pub fn write_str16(&mut self, string: &str) -> Result<(), Error> {
        let mut s16: Vec<u8> = vec![];
        self.write_i32(string.len() as i32)?;
        for c in string.encode_utf16() {
            s16.write_u16::<LittleEndian>(c)?;
        }
        s16.write_u16::<LittleEndian>(0)?;

        if s16.len() % 4 != 0 {
            s16.resize(s16.len() + 4 - (s16.len() % 4), 0);
        }

        self.cursor.write_all(s16.as_slice())?;

        Ok(())
    }

    /// Write a string to the parcel
    pub fn write_str(&mut self, string: &str) -> Result<(), Error>{
        let mut s8: Vec<u8> = Vec::with_capacity(string.len() + 1);
        self.write_i32(string.len() as i32)?;
        for c in string.bytes() {
            s8.push(c);
        }
        s8.push(0);

        if s8.len() % 4 != 0 {
            s8.resize(s8.len() + 4 - (s8.len() % 4), 0);
        }

        self.cursor.write_all(s8.as_slice())?;

        Ok(())
    }

    /// Write a Binder object into the parcel
    pub fn write_binder(&mut self, object: *const c_void) -> Result<(), Error> {
        BinderFlatObject::new(BinderType::Binder, object as usize, 0, 0).serialize(self)?;
        Ok(())
    }

    /// Write a file descriptor into the parcel
    pub fn write_file_descriptor(&mut self, fd: RawFd, take_ownership: bool) -> Result<(), Error>{
        BinderFlatObject::new(BinderType::Fd, fd as usize, if take_ownership { 1 } else { 0 }, 0x17f).serialize(self)?;
        Ok(())
    }

    /// REad a file descriptor from the parcel
    pub fn read_file_descriptor(&mut self) -> Result<RawFd, Error> {
        let flat_object: BinderFlatObject = self.read_object()?;
        assert!(flat_object.binder_type == BinderType::Fd);
        Ok(flat_object.handle as RawFd)
    }

    /// Read a string from the parcel
    pub fn read_str16(&mut self) -> Result<String, Error> {
        let len = (self.read_i32()? + 1) as usize;
        if len == 0 {
            return Ok("".to_string())
        }
        unsafe {
            let u16_array: Vec<u16> = self.read(len * 2)?.chunks_exact(2).into_iter().map(|a| u16::from_ne_bytes([a[0], a[1]])).collect();
            let mut res = String::from_utf16(&u16_array)?;
            res.truncate(len - 1);
            Ok(res)
        }
    }

    /// Read a string from the parcel
    pub fn read_str(&mut self) -> Result<String, Error> {
        let len = (self.read_i32()? + 1) as usize;
        if len == 0 {
            return Ok("".to_string())
        }
        unsafe {
            let u8_array = self.read(len)?;
            let mut res = String::from_utf8(u8_array)?;
            res.truncate(len - 1);
            Ok(res)
        }
    }

    /// Read an interface token from the parcel
    pub fn read_interface_token(&mut self) -> Result<String, Error> {
        //assert!(self.read_i32() == STRICT_MODE_PENALTY_GATHER);
        self.read_i32()?;
        assert!(self.read_i32()? == -1);
        assert!(self.read_i32()? == HEADER);
        Ok(self.read_str16()?)
    }


    /// Write an interface token to the parcel
    pub fn write_interface_token(&mut self, name: &str) -> Result<(), Error>{
        // strict mode policy
        self.write_i32(STRICT_MODE_PENALTY_GATHER | 0x42000004)?;
        // work source uid, we use kUnsetWorkSource
        self.write_i32(-1)?;
        // header marker
        self.write_i32(HEADER)?;
        // the interface name
        self.write_str16(name)?;

        Ok(())
    }
}
