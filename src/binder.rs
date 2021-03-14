use crate::{
    parcel::Parcel
};

use nix::{
    fcntl::{
        OFlag,
        open,
    },
    ioctl_readwrite,
    ioctl_write_int,
    ioctl_write_ptr,
    sys::{
        mman::{
            MapFlags,
            mmap,
            ProtFlags,
        },
        stat::Mode,
    },
    unistd::close,
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use std::{
    convert::TryFrom,
    ffi::c_void,
    mem::size_of,
    os::unix::io::RawFd,
    ptr,
    slice,
};


/// The binder device name
const DEVICE: &str = "/dev/binder";

/// The default maximum number of threads to support
const DEFAULT_MAX_BINDER_THREADS: u32 = 15;

const PAGE_SIZE: usize = 0x1000;
const BINDER_VM_SIZE: usize =  (1 * 1024 * 1024) - PAGE_SIZE * 2;


macro_rules! pack_chars {
    ($c1:expr, $c2:expr, $c3:expr, $c4:expr) => {
        (((($c1 as u32) << 24)) | ((($c2 as u32) << 16)) | ((($c3 as u32) << 8)) | ($c4 as u32))
    };
}

const BINDER_TYPE_LARGE: u8 = 0x85;

#[repr(u32)]
#[derive(Debug, Hash, Clone, Copy)]
pub enum BinderType {
    Binder = pack_chars!(b's', b'b', b'*', BINDER_TYPE_LARGE),
    WeakBinder = pack_chars!(b'w', b'b', b'*', BINDER_TYPE_LARGE),
    Handle = pack_chars!(b's', b'h', b'*', BINDER_TYPE_LARGE),
    WeakHandle = pack_chars!(b'w', b'h', b'*', BINDER_TYPE_LARGE),
    Fd = pack_chars!(b'f', b'd', b'*', BINDER_TYPE_LARGE),
    Fda = pack_chars!(b'f', b'd', b'a', BINDER_TYPE_LARGE),
    Ptr = pack_chars!(b'p', b't', b'*', BINDER_TYPE_LARGE),
}

impl From<u32> for BinderType {
    fn from(v: u32) -> Self {
        unsafe { ::std::mem::transmute(v) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct BinderFlatObject {
    binder_type: BinderType,
    flags: u32,
    pub(crate) handle: *const c_void,
    cookie: *const c_void,
}

impl BinderFlatObject {
    pub fn new(handle: usize, cookie: usize, flags: u32) -> Self {
        Self {
            binder_type: BinderType::Binder,
            flags,
            handle: handle as *const c_void,
            cookie: cookie as *const c_void,
        }

    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Transaction {
    FirstCall = 1,
    LastCall = 16777215,
    Ping = pack_chars!(b'_', b'P',b'N',b'G'),
    Dump = 1598311760,
    Interface = 1598968902,
    Sysprops = 1599295570,
}

/// A structure representing the binder version
#[repr(C)]
pub struct BinderVersion {
    protocol_version: i32,
}


#[repr(C)]
pub struct BinderWriteRead {
    write_size: usize,
    write_consumed: usize,
    write_buffer: *const c_void,
    read_size: usize,
    read_consumed: usize,
    read_buffer: *mut c_void,
}

#[repr(C)]
pub(crate) struct BinderTransactionDataData {
}
#[repr(C)]
#[derive(Debug)]
pub struct BinderTransactionData {
    target: u32,
    cookie: u64,
    code: u32,
    flags: u32,
    sender_pid: u32,
    sender_euid: u32,
    data_size: u64,
    offset_size: u64,
    data: *mut u8,
    offsets: *mut usize,
}

enum Result {
    InvalidOperation,
    NoError,
}

ioctl_readwrite!(binder_write_read, b'b', 1, BinderWriteRead);
ioctl_write_ptr!(binder_set_max_threads, b'b', 5, u32);
ioctl_readwrite!(binder_read_version, b'b', 9, BinderVersion);

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TransactionFlags {
    ONE_WAY = 1,
    ROOT_OBJECT = 4,
    STATUS_CODE = 8,
    ACCEPT_FDS = 16,
}

macro_rules! _iow {
    ($c1:expr, $c2:expr, $c3:expr) => {
        (((0x40 << 24)) | ((($c3 as u32) << 16)) | ((($c1 as u32) << 8)) | ($c2 as u32))
    };
}

#[repr(u32)]
#[derive(Debug)]
pub enum BinderDriverCommandProtocol {
    BC_TRANSACTION = _iow!(b'c', 0, 0x40),
    BC_REPLY = _iow!(b'c', 1, 0x40),
    BC_ACQUIRE_RESULT = _iow!(b'c', 2, 0x4),
    BC_FREE_BUFFER = _iow!(b'c', 3, 0x8),
    BC_INCREFS = _iow!(b'c', 4, 0x4),
    BC_ACQUIRE = _iow!(b'c', 5, 0x4),
    BC_RELEASE = _iow!(b'c', 6, 0x4),
    BC_DECREFS = _iow!(b'c', 7, 0x4),
    BC_INCREFS_DONE = _iow!(b'c', 8, 0x8),
    BC_ACQUIRE_DONE = _iow!(b'c', 9, 0x8),
    BC_ATTEMPT_ACQUIRE = _iow!(b'c', 10, 0x10),
    BC_REGISTER_LOOPER = 25355,
    BC_ENTER_LOOPER = 25356,
    BC_EXIT_LOOPER = 25357,
    BC_REQUEST_DEATH_NOTIFICATION = _iow!(b'c', 14, 0x10),
    BC_CLEAR_DEATH_NOTIFICATION = _iow!(b'c', 15, 0x10),
    BC_DEAD_BINDER_DONE = _iow!(b'c', 16, 0x8),
}

#[repr(u32)]
#[derive(Debug, IntoPrimitive, TryFromPrimitive, Hash, Clone, Copy)]
pub enum BinderDriverReturnProtocol {
    BR_ERROR = 2147774976,
    BR_OK = 0x7201,
    BR_TRANSACTION = 0x80407202,
    BR_REPLY = 0x80407203,
    BR_ACQUIRE_RESULT = 2147774980,
    BR_DEAD_REPLY = 29189,
    BR_TRANSACTION_COMPLETE = 29190,
    BR_INCREFS = 0x80107207,
    BR_ACQUIRE = 2148037128,
    BR_RELEASE = 2148037129,
    BR_DECREFS = 2148037130,
    BR_ATTEMPT_ACQUIRE = 2148299275,
    BR_NOOP = 29196,
    BR_SPAWN_LOOPER = 29197,
    BR_FINISHED = 29198,
    BR_DEAD_BINDER = 2147774991,
    BR_CLEAR_DEATH_NOTIFICATION_DONE = 2147774992,
    BR_FAILED_REPLY = 29201,
}


/// Structure representing an open Binder interface.
pub struct Binder {
    fd: RawFd,
    mem: *const c_void,
    pending_out_data: Parcel,
}

impl Binder {
    pub fn new() -> Self {
        let mut flags = OFlag::empty();
        flags.set(OFlag::O_RDWR, true);
        flags.set(OFlag::O_CLOEXEC, true);

        let fd = open(DEVICE, flags, Mode::empty()).expect("Failed to open binder device");

        let mut binder_version = BinderVersion { protocol_version: 0 };
        unsafe {
            binder_read_version(fd, &mut binder_version).expect("Failed to read binder version");
        }

        println!("Binder version is {}", binder_version.protocol_version);

        let mut flags = MapFlags::empty();
        flags.set(MapFlags::MAP_PRIVATE, true);
        flags.set(MapFlags::MAP_NORESERVE, true);
        let mapping_address = unsafe { mmap(ptr::null_mut(), BINDER_VM_SIZE, ProtFlags::PROT_READ, flags, fd, 0) }.expect("Failed to map the binder file");

        let binder = Self {
            fd,
            mem: mapping_address as *const _,
            pending_out_data: Parcel::empty(),
        };

        unsafe {
            binder_set_max_threads(fd, &DEFAULT_MAX_BINDER_THREADS).expect("Failed to set max threads");
        }


        binder
    }

    /// Tell binder that we are entering the looper
    pub fn enter_looper(&self) {
        let mut parcel_out = Parcel::empty();

        parcel_out.write_i32(BinderDriverCommandProtocol::BC_ENTER_LOOPER as i32);

        self.write_read(&parcel_out, false);
    }

    /// Tell binder that we are exiting the looper
    fn exit_looper(&self) {
        let mut parcel_out = Parcel::empty();

        parcel_out.write_i32(BinderDriverCommandProtocol::BC_EXIT_LOOPER as i32);

        self.write_read(&parcel_out, false);
    }

    /// Increment the server side reference count of the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn add_ref(&mut self, handle: i32) {
        self.pending_out_data.write_u32(BinderDriverCommandProtocol::BC_INCREFS as u32);
        self.pending_out_data.write_i32(handle);
    }

    /// Decrement the server side reference count of the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn dec_ref(&mut self, handle: i32) {
        self.pending_out_data.write_u32(BinderDriverCommandProtocol::BC_DECREFS as u32);
        self.pending_out_data.write_i32(handle);
    }

    /// Acquire the server side resource for the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn acquire(&mut self, handle: i32) {
        self.pending_out_data.write_u32(BinderDriverCommandProtocol::BC_ACQUIRE as u32);
        self.pending_out_data.write_i32(handle);
    }

    /// Release the server side resource for the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn release(&mut self, handle: i32) {
        self.pending_out_data.write_u32(BinderDriverCommandProtocol::BC_RELEASE as u32);
        self.pending_out_data.write_i32(handle);
    }

    pub fn transact(&mut self, handle: i32, code: u32, flags: u32, data: &mut Parcel) -> Parcel {

        self.pending_out_data.write_i32(BinderDriverCommandProtocol::BC_TRANSACTION as i32);

        let transaction_data_out = BinderTransactionData {
            target: handle as u32,
            code,
            flags: flags as u32,
            cookie: 0,
            sender_pid: 0,
            sender_euid: 0,
            data_size: data.len() as u64,
            offset_size: (data.offsets_len() * size_of::<usize>()) as u64,
            data: if data.len() != 0 { data.as_mut_ptr() } else { 0 as *mut u8 },
            offsets: if data.offsets_len() != 0 { data.offsets().as_mut_ptr() } else { 0 as *mut usize },
        };
        self.pending_out_data.write_transaction_data(&transaction_data_out);
        println!("outgoing data: {:?}", self.pending_out_data);

        let mut parcel_in = self.write_read(&self.pending_out_data, true);
        println!("parcel_in: {:?}", parcel_in);
        self.pending_out_data.reset();

        let mut acquire_result = Result::NoError;

        while parcel_in.has_unread_data() {
            let cmd  = parcel_in.read_u32();
            println!("cmd is {}", cmd);
            match BinderDriverReturnProtocol::try_from(cmd).unwrap() {
                BinderDriverReturnProtocol::BR_TRANSACTION_COMPLETE => {},
                BinderDriverReturnProtocol::BR_DEAD_REPLY => {
                    panic!("Got a DEAD_REPLY");
                },
                BinderDriverReturnProtocol::BR_FAILED_REPLY => {
                    panic!("Transaction failed");
                },
                BinderDriverReturnProtocol::BR_INCREFS => {
                    panic!("IncRefs {:?}", parcel_in.read(0x10));
                },
                BinderDriverReturnProtocol::BR_ACQUIRE_RESULT => {
                    let result = parcel_in.read_i32();
                    acquire_result = if result == 0 {
                        Result::InvalidOperation
                    } else {
                        Result::NoError
                    };
                },
                BinderDriverReturnProtocol::BR_REPLY => {
                    println!("Got a response!");
                    let transaction_data_in = parcel_in.read_transaction_data();
                    println!("data: {:?}", transaction_data_in);
                    return
                        Parcel::from_data_and_offsets(
                            transaction_data_in.data,
                            transaction_data_in.data_size as usize,
                            transaction_data_in.offsets,
                            transaction_data_in.offset_size as usize / size_of::<usize>()
                        );
                },
                BinderDriverReturnProtocol::BR_ERROR => {
                    println!("Got an error {}", parcel_in.read_i32());
                },
                BinderDriverReturnProtocol::BR_NOOP => {
                    println!("Got a NOOP");
                },
                BinderDriverReturnProtocol::BR_SPAWN_LOOPER => {
                    println!("Need to spawn a looper");
                },
                _  => {}

            }
        }

        Parcel::empty()
    }

    /// Perform a low-level binder write/read operation
    fn write_read(&self, data_out: &Parcel, with_read: bool) -> Parcel {
        let mut data_in = [0u8; 32 * 8];

        let mut write_read_struct = BinderWriteRead {
            write_size: data_out.len(),
            write_buffer: data_out.as_ptr() as *const c_void,
            write_consumed: 0,
            read_size: if with_read { data_in.len() } else { 0 },
            read_buffer: data_in.as_mut_ptr() as *mut c_void,
            read_consumed: 0,
        };

        println!("before write_read {}, {}", write_read_struct.write_size, write_read_struct.write_consumed);
        unsafe {
            binder_write_read(self.fd, &mut write_read_struct).expect("Failed to perform write_read");
        }
        println!("after write_read {}, {}", write_read_struct.write_consumed, write_read_struct.read_consumed);
        println!("response: {:?}", &data_in[..write_read_struct.read_consumed]);
        Parcel::from_slice(&data_in[..write_read_struct.read_consumed])

    }

}

/// Implement Drop for Binder, so that we can clean up resources
impl Drop for Binder {
    fn drop(&mut self) {
        //TODO: do we need to unmap?

        self.exit_looper();

        close(self.fd);
    }
}
