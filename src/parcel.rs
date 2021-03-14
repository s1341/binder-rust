use std::{
    ffi::c_void,
    fmt,
    io::{Cursor, Read, Write},
    mem::size_of,
    mem::transmute,
    slice,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{Binder, BinderDriverCommandProtocol, BinderTransactionData, BinderType};

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

    pub fn from_data_and_offsets(
        data: *mut u8,
        data_size: usize,
        offsets: *mut usize,
        offsets_size: usize,
    ) -> Self {
        unsafe {
            Self {
                cursor: Cursor::new(slice::from_raw_parts(data, data_size).to_vec()),
                object_offsets: slice::from_raw_parts(offsets, offsets_size).to_vec(),
                objects_position: 0,
            }
        }
    }

    pub fn reset(&mut self) {
        self.cursor.set_position(0);
        self.cursor.get_mut().resize(0, 0);
        self.objects_position = 0;
        self.object_offsets.resize(0, 0);
    }

    /// Append the contents of another parcel to this parcel
    pub fn append_parcel(&mut self, other: &mut Parcel) {
        let current_position = self.cursor.position();
        self.cursor.write(other.to_slice());
        for offset in &other.object_offsets {
            self.object_offsets.push(offset + current_position as usize);
        }
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

    /// Retrieve the number of object offsets
    pub fn offsets_len(&self) -> usize {
        self.object_offsets.len()
    }

    /// Retrieve the object offsets
    pub fn offsets(&mut self) -> &mut Vec<usize> {
        &mut self.object_offsets
    }

    /// Check if the parcel has unread data
    pub fn has_unread_data(&self) -> bool {
        self.cursor.position() != self.len() as u64
    }

    /// Write an i32 to the parcel
    pub fn write_i32(&mut self, data: i32) {
        self.cursor.write_i32::<LittleEndian>(data);
    }
    /// Write an u32 to the parcel
    pub fn write_u32(&mut self, data: u32) {
        self.cursor.write_u32::<LittleEndian>(data);
    }
    /// Write an u16 to the parcel
    pub fn write_u16(&mut self, data: u16) {
        self.cursor.write_u16::<LittleEndian>(data);
    }

    /// Write a bool to the parcel
    pub fn write_bool(&mut self, data: bool) {
        self.cursor.write_u32::<LittleEndian>(data as u32);
    }

    /// Read an i32 from the parcel
    pub fn read_i32(&mut self) -> i32 {
        self.cursor.read_i32::<LittleEndian>().unwrap()
    }

    /// Write a slice of data to the parcel
    pub fn write(&mut self, data: &[u8]) {
        let padded_len = (data.len() + 3) & !3;

        let mut data = data.to_vec();
        if padded_len > data.len() {
            data.resize(padded_len, 0);
        };

        self.cursor
            .write(data.as_slice())
            .expect("Coudn't write to parcel data");
    }

    /// Write a BinderTransactionData struct into the parcel
    pub fn write_transaction_data(&mut self, data: &BinderTransactionData) {
        self.write(unsafe {
            slice::from_raw_parts(
                data as *const _ as *const u8,
                size_of::<BinderTransactionData>(),
            )
        });
    }

    /// Read an u32 from the parcel
    pub fn read_u32(&mut self) -> u32 {
        self.cursor.read_u32::<LittleEndian>().unwrap()
    }

    /// Read an u64 from the parcel
    pub fn read_u64(&mut self) -> u64 {
        self.cursor.read_u64::<LittleEndian>().unwrap()
    }

    /// Read an usize from the parcel
    pub fn read_usize(&mut self) -> usize {
        if size_of::<usize>() == size_of::<u32>() {
            self.read_u32() as usize
        } else {
            self.read_u64() as usize
        }
    }

    /// Read a void pointer from the parcel
    pub fn read_pointer(&mut self) -> *const c_void {
        self.read_usize() as *const c_void
    }

    /// Read a slice of size bytes from the parcel
    pub fn read(&mut self, size: usize) -> Vec<u8> {
        let size = if (size % 4) != 0 {
            size + 4 - (size % 4)
        } else {
            size
        };
        let mut data = vec![0u8; size];
        self.cursor.read(&mut data);
        data
    }

    /// Read a BinderTransactionData from the parcel
    pub fn read_transaction_data(&mut self) -> BinderTransactionData {
        self.read_object()
    }

    /// Read an object of type T from the parcel
    pub fn read_object<T>(&mut self) -> T {
        unsafe {
            let data = slice::from_raw_parts(
                self.cursor
                    .get_ref()
                    .as_ptr()
                    .offset(self.cursor.position() as isize),
                size_of::<T>(),
            );
            self.cursor.set_position(self.cursor.position() + size_of::<T>() as u64);
            (data.as_ptr() as *const T).read()
        }
    }

    pub fn write_object<T>(&mut self, object: T) {
        self.object_offsets.push(self.cursor.position() as usize);
        self.cursor.write(unsafe {
            slice::from_raw_parts(&object as *const _ as *const u8, size_of::<T>())
        });

    }

    /// Write a string to the parcel
    pub fn write_str16(&mut self, string: &str) {
        let mut s16: Vec<u8> = vec![];
        self.write_i32(string.len() as i32);
        for c in string.encode_utf16() {
            s16.write_u16::<LittleEndian>(c);
        }
        s16.write_u16::<LittleEndian>(0);

        if s16.len() % 4 != 0 {
            s16.resize(s16.len() + 4 - (s16.len() % 4), 0)
        }

        self.cursor.write(s16.as_slice());
    }

    /// Read a string from the parcel
    pub fn read_str16(&mut self) -> String {
        let len = ((self.read_i32() + 1) * 2) as usize;
        unsafe {
            let u16_array = slice::from_raw_parts(self.read(len).as_mut_ptr() as *mut u16, len);
            let mut res = String::from_utf16(u16_array).unwrap();
            res.truncate(len / 2 - 1);
            res
        }
    }

    /// Read an interface token from the parcel
    pub fn read_interface_token(&mut self) -> String {
        assert!(self.read_i32() == STRICT_MODE_PENALTY_GATHER);
        assert!(self.read_i32() == -1);
        assert!(self.read_i32() == HEADER);
        self.read_str16()
    }


    /// Write an interface token to the parcel
    pub fn write_interface_token(&mut self, name: &str) {
        // strict mode policy
        self.write_i32(STRICT_MODE_PENALTY_GATHER);
        // work source uid, we use kUnsetWorkSource
        self.write_i32(-1);
        // header marker
        self.write_i32(HEADER);
        // the interface name
        self.write_str16(name);
    }
}
