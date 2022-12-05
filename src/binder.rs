use crate::{parcel::Parcel, Error, Parcelable};
use parcelable_derive::Parcelable;

use nix::{
    fcntl::{open, OFlag},
    ioctl_readwrite, ioctl_write_int, ioctl_write_ptr,
    sys::{
        mman::{mmap, MapFlags, ProtFlags},
        stat::Mode,
    },
    unistd::close,
};

use std::{
    convert::{TryFrom, TryInto},
    ffi::c_void,
    mem::size_of,
    ops::BitOr,
    os::unix::io::RawFd,
    ptr, slice,
};

use num_traits::FromPrimitive;

/// The binder device name
const DEVICE: &str = "/dev/binder";

/// The default maximum number of threads to support
const DEFAULT_MAX_BINDER_THREADS: u32 = 15;

const PAGE_SIZE: usize = 0x1000;
const BINDER_VM_SIZE: usize = (1 * 1024 * 1024) - PAGE_SIZE * 2;

macro_rules! pack_chars {
    ($c1:expr, $c2:expr, $c3:expr, $c4:expr) => {
        ((($c1 as u32) << 24) | (($c2 as u32) << 16) | (($c3 as u32) << 8) | ($c4 as u32))
    };
}

const BINDER_TYPE_LARGE: u8 = 0x85;

const TF_BINDER: u32 = pack_chars!(b's', b'b', b'*', BINDER_TYPE_LARGE);
const TF_WEAKBINDER: u32 = pack_chars!(b'w', b'b', b'*', BINDER_TYPE_LARGE);
const TF_HANDLE: u32 = pack_chars!(b's', b'h', b'*', BINDER_TYPE_LARGE);
const TF_WEAKHANDLE: u32 = pack_chars!(b'w', b'h', b'*', BINDER_TYPE_LARGE);
const TF_FD: u32 = pack_chars!(b'f', b'd', b'*', BINDER_TYPE_LARGE);
const TF_FDA: u32 = pack_chars!(b'f', b'd', b'a', BINDER_TYPE_LARGE);
const TF_PTR: u32 = pack_chars!(b'p', b't', b'*', BINDER_TYPE_LARGE);

#[derive(Debug, Hash, Clone, Copy, PartialEq, FromPrimitive)]
#[repr(u32)]
pub enum BinderType {
    Binder = TF_BINDER,
    WeakBinder = TF_WEAKBINDER,
    Handle = TF_HANDLE,
    WeakHandle = TF_WEAKHANDLE,
    Fd = TF_FD,
    Fda = TF_FDA,
    Ptr = TF_PTR,
}
impl Parcelable for BinderType {
    fn deserialize(parcel: &mut Parcel) -> Result<Self, crate::Error>
    where
        Self: Sized,
    {
        Ok(match parcel.read_u32()? {
            TF_BINDER => BinderType::Binder,
            TF_WEAKBINDER => BinderType::WeakBinder,
            TF_HANDLE => BinderType::Handle,
            TF_WEAKHANDLE => BinderType::WeakHandle,
            TF_FD => BinderType::Fd,
            TF_FDA => BinderType::Fda,
            TF_PTR => BinderType::Ptr,
            _ => {
                return Err(Error::BadEnumValue);
            }
        })
    }

    fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
        parcel.write_u32(*self as u32)?;
        Ok(())
    }
}

#[derive(Parcelable, Clone, Debug)]
#[parcelable(push_object = true)]
pub struct BinderFlatObject {
    pub(crate) binder_type: BinderType,
    flags: u32,
    pub(crate) handle: usize,
    cookie: usize,
    stability: u32, // stability  == SYSTEM
}

impl BinderFlatObject {
    pub fn new(binder_type: BinderType, handle: usize, cookie: usize, flags: u32) -> Self {
        Self {
            binder_type,
            flags,
            handle,
            cookie,
            stability: 0xc, // == SYSTEM
        }
    }

    pub fn handle(&self) -> usize {
        self.handle
    }

    pub fn cookie(&self) -> usize {
        self.cookie
    }
}
#[derive(Parcelable, Clone, Debug)]
#[parcelable(push_object = true)]
pub struct BinderFd {
    pub(crate) binder_type: BinderType,
    flags: u32,
    pub(crate) handle: usize,
    cookie: usize,
}

impl BinderFd {
    pub fn new(binder_type: BinderType, handle: usize, cookie: usize, flags: u32) -> Self {
        Self {
            binder_type,
            flags,
            handle,
            cookie,
        }
    }

    pub fn handle(&self) -> usize {
        self.handle
    }

    pub fn cookie(&self) -> usize {
        self.cookie
    }
}

const PING_TRANSCATION: u32 = pack_chars!(b'_', b'P', b'N', b'G');
const DUMP_TRANSACTION: u32 = pack_chars!(b'_', b'D', b'M', b'P');
const SHELL_COMMAND_TRANSACTION: u32 = pack_chars!(b'_', b'C', b'M', b'D');
const INTERFACE_TRANSACTION: u32 = pack_chars!(b'_', b'N', b'T', b'F');
const SYSPROPS_TRANSACTION: u32 = pack_chars!(b'_', b'S', b'P', b'R');
const EXTENSION_TRANSACTION: u32 = pack_chars!(b'_', b'E', b'X', b'T');
const DEBUG_PID_TRANSACTION: u32 = pack_chars!(b'_', b'P', b'I', b'D');
const TWEET_TRANSACTION: u32 = pack_chars!(b'_', b'T', b'W', b'T');
const LIKE_TRANSACTION: u32 = pack_chars!(b'_', b'L', b'I', b'K');

#[repr(u32)]
#[derive(Debug, FromPrimitive)]
pub enum Transaction {
    FirstCall = 1,
    LastCall = 0xffffff,
    Ping = PING_TRANSCATION,
    Dump = DUMP_TRANSACTION,
    ShellCommand = SHELL_COMMAND_TRANSACTION,
    Interface = INTERFACE_TRANSACTION,
    Sysprops = SYSPROPS_TRANSACTION,
    Extension = EXTENSION_TRANSACTION,
    DebugPid = DEBUG_PID_TRANSACTION,
    Tweet = TWEET_TRANSACTION,
    Like = LIKE_TRANSACTION,
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

impl BinderWriteRead {
    pub fn write_size(&self) -> usize {
        self.write_size
    }
    pub fn write_consumed(&self) -> usize {
        self.write_consumed
    }
    pub fn read_size(&self) -> usize {
        self.read_size
    }
    pub fn read_consumed(&self) -> usize {
        self.read_consumed
    }
    pub fn write_buffer(&self) -> *const c_void {
        self.write_buffer
    }
    pub fn read_buffer(&self) -> *mut c_void {
        self.read_buffer
    }
}
#[repr(C)]
pub(crate) struct BinderTransactionDataData {}
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

impl BinderTransactionData {
    pub fn code(&self) -> u32 {
        self.code
    }
    pub fn cookie(&self) -> u64 {
        self.cookie
    }

    pub fn target(&self) -> u32 {
        self.target
    }

    pub fn flags(&self) -> TransactionFlags {
        TransactionFlags::from_bits(self.flags).unwrap()
    }

    pub unsafe fn raw_data(&self) -> &[u8] {
        std::slice::from_raw_parts(self.data, self.data_size as usize)
    }

    pub fn parcel(&self) -> Parcel {
        unsafe { Parcel::from_slice(self.raw_data()) }
    }
}

enum BinderResult {
    InvalidOperation,
    NoError,
}

ioctl_readwrite!(binder_write_read, b'b', 1, BinderWriteRead);
ioctl_write_ptr!(binder_set_max_threads, b'b', 5, u32);
ioctl_readwrite!(binder_read_version, b'b', 9, BinderVersion);

bitflags! {
    pub struct TransactionFlags: u32 {
        const OneWay = 1;
        const CollectNotedAppOps = 2;
        const RootObject = 4;
        const StatusCode = 8;
        const AcceptFds = 0x10;
        const ClearBuf = 0x20;
    }
}

macro_rules! _iow {
    ($c1:expr, $c2:expr, $c3:expr) => {
        ((0x40 << 24) | (($c3 as u32) << 16) | (($c1 as u32) << 8) | ($c2 as u32))
    };
}

macro_rules! _ior {
    ($c1:expr, $c2:expr, $c3:expr) => {
        ((0x80 << 24) | (($c3 as u32) << 16) | (($c1 as u32) << 8) | ($c2 as u32))
    };
}

macro_rules! _io {
    ($c1:expr, $c2:expr) => {
        ((($c1 as u32) << 8) | ($c2 as u32))
    };
}

const BC_TRANSACTION: u32 = _iow!(b'c', 0, 0x40);
const BC_REPLY: u32 = _iow!(b'c', 1, 0x40);
const BC_ACQUIRE_RESULT: u32 = _iow!(b'c', 2, 0x4);
const BC_FREE_BUFFER: u32 = _iow!(b'c', 3, 0x8);
const BC_INCREFS: u32 = _iow!(b'c', 4, 0x4);
const BC_ACQUIRE: u32 = _iow!(b'c', 5, 0x4);
const BC_RELEASE: u32 = _iow!(b'c', 6, 0x4);
const BC_DECREFS: u32 = _iow!(b'c', 7, 0x4);
const BC_INCREFS_DONE: u32 = _iow!(b'c', 8, 0x10);
const BC_ACQUIRE_DONE: u32 = _iow!(b'c', 9, 0x10);
const BC_ATTEMPT_ACQUIRE: u32 = _iow!(b'c', 10, 0x10);
const BC_REGISTER_LOOPER: u32 = _io!(b'c', 11);
const BC_ENTER_LOOPER: u32 = _io!(b'c', 12);
const BC_EXIT_LOOPER: u32 = _io!(b'c', 13);
const BC_REQUEST_DEATH_NOTIFICATION: u32 = _iow!(b'c', 14, 0xc);
const BC_CLEAR_DEATH_NOTIFICATION: u32 = _iow!(b'c', 15, 0x0c);
const BC_DEAD_BINDER_DONE: u32 = _iow!(b'c', 16, 0x8);
const BC_TRANSACTION_SG: u32 = _iow!(b'c', 17, 0x48);
const BC_REPLY_SG: u32 = _iow!(b'c', 18, 0x48);

#[repr(u32)]
#[derive(Debug, FromPrimitive)]
pub enum BinderDriverCommandProtocol {
    Transaction = BC_TRANSACTION,
    Reply = BC_REPLY,
    AcquireResult = BC_ACQUIRE_RESULT,
    FreeBuffer = BC_FREE_BUFFER,
    IncRefs = BC_INCREFS,
    Acquire = BC_ACQUIRE,
    Release = BC_RELEASE,
    DecRefs = BC_DECREFS,
    IncRefsDone = BC_INCREFS_DONE,
    AcquireDone = BC_ACQUIRE_DONE,
    AttemptAcquire = BC_ATTEMPT_ACQUIRE,
    RegisterLooper = BC_REGISTER_LOOPER,
    EnterLooper = BC_ENTER_LOOPER,
    ExitLooper = BC_EXIT_LOOPER,
    RequestDeathNotification = BC_REQUEST_DEATH_NOTIFICATION,
    ClearDeathNotification = BC_CLEAR_DEATH_NOTIFICATION,
    DeadBinderDone = BC_DEAD_BINDER_DONE,
    TransactionSG = BC_TRANSACTION_SG,
    ReplySG = BC_REPLY_SG,
}

impl From<u32> for BinderDriverCommandProtocol {
    fn from(int: u32) -> Self {
        log::info!("BinderDriverCommandProtocol: {:x}", int);
        BinderDriverCommandProtocol::from_u32(int).unwrap()
    }
}

const BR_ERROR: u32 = _ior!(b'r', 0, 4);
const BR_OK: u32 = _ior!(b'r', 1, 0);
const BR_TRANSACTION: u32 = _ior!(b'r', 2, 0x40);
const BR_REPLY: u32 = _ior!(b'r', 3, 0x40);
const BR_ACQUIRE_RESULT: u32 = _ior!(b'r', 4, 0x4);
const BR_DEAD_REPLY: u32 = _io!(b'r', 5);
const BR_TRANSACTION_COMPLETE: u32 = _io!(b'r', 6);
const BR_INCREFS: u32 = _ior!(b'r', 7, 0x10);
const BR_ACQUIRE: u32 = _ior!(b'r', 8, 0x10);
const BR_RELEASE: u32 = _ior!(b'r', 9, 0x10);
const BR_DECREFS: u32 = _ior!(b'r', 10, 0x10);
const BR_ATTEMPT_ACQUIRE: u32 = _ior!(b'r', 11, 0xc);
const BR_NOOP: u32 = _io!(b'r', 12);
const BR_SPAWN_LOOPER: u32 = _io!(b'r', 13);
const BR_FINISHED: u32 = _io!(b'r', 14);
const BR_DEAD_BINDER: u32 = _ior!(b'r', 15, 0x8);
const BR_CLEAR_DEATH_NOTIFICATION_DONE: u32 = _ior!(b'r', 16, 0x8);
const BR_FAILED_REPLY: u32 = _io!(b'r', 17);
const BR_FROZEN_REPLY: u32 = _io!(b'r', 18);
const BR_ONEWAY_SPAM_SUSPECT: u32 = _io!(b'r', 19);

#[repr(u32)]
#[derive(Debug, FromPrimitive, ToPrimitive)]
pub enum BinderDriverReturnProtocol {
    Error = BR_ERROR,
    Ok = BR_OK,
    Transaction = BR_TRANSACTION,
    Reply = BR_REPLY,
    AcquireResult = BR_ACQUIRE_RESULT,
    DeadReply = BR_DEAD_REPLY,
    TransactionComplete = BR_TRANSACTION_COMPLETE,
    IncRefs = BR_INCREFS,
    Acquire = BR_ACQUIRE,
    Release = BR_RELEASE,
    DecRefs = BR_DECREFS,
    AttemptAcquire = BR_ATTEMPT_ACQUIRE,
    Noop = BR_NOOP,
    SpawnLooper = BR_SPAWN_LOOPER,
    Finished = BR_FINISHED,
    DeadBinder = BR_DEAD_BINDER,
    ClearDeathNotification = BR_CLEAR_DEATH_NOTIFICATION_DONE,
    FailedReply = BR_FAILED_REPLY,
    FrozenReply = BR_FROZEN_REPLY,
    OnwaySpamSuspect = BR_ONEWAY_SPAM_SUSPECT,
}

impl From<u32> for BinderDriverReturnProtocol {
    fn from(int: u32) -> Self {
        log::info!("BinderDriverReturnProtocol: {:x}", int);
        BinderDriverReturnProtocol::from_u32(int).unwrap()
    }
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

        let mut binder_version = BinderVersion {
            protocol_version: 0,
        };
        unsafe {
            binder_read_version(fd, &mut binder_version).expect("Failed to read binder version");
        }

        let mut flags = MapFlags::empty();
        flags.set(MapFlags::MAP_PRIVATE, true);
        flags.set(MapFlags::MAP_NORESERVE, true);
        let mapping_address = unsafe {
            mmap(
                ptr::null_mut(),
                BINDER_VM_SIZE,
                ProtFlags::PROT_READ,
                flags,
                fd,
                0,
            )
        }
        .expect("Failed to map the binder file");

        let binder = Self {
            fd,
            mem: mapping_address as *const _,
            pending_out_data: Parcel::empty(),
        };

        unsafe {
            binder_set_max_threads(fd, &DEFAULT_MAX_BINDER_THREADS)
                .expect("Failed to set max threads");
        }

        binder
    }

    /// Tell binder that we are entering the looper
    pub fn enter_looper(&self) -> Result<(), Error> {
        let mut parcel_out = Parcel::empty();

        parcel_out.write_i32(BinderDriverCommandProtocol::EnterLooper as i32)?;

        self.write_read(&parcel_out, false);
        Ok(())
    }

    /// Tell binder that we are exiting the looper
    fn exit_looper(&self) -> Result<(), Error> {
        let mut parcel_out = Parcel::empty();

        parcel_out.write_i32(BinderDriverCommandProtocol::ExitLooper as i32)?;

        self.write_read(&parcel_out, false);
        Ok(())
    }

    /// Increment the server side reference count of the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn add_ref(&mut self, handle: i32) -> Result<(), Error> {
        self.pending_out_data
            .write_u32(BinderDriverCommandProtocol::IncRefs as u32)?;
        self.pending_out_data.write_i32(handle)?;

        Ok(())
    }

    /// Decrement the server side reference count of the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn dec_ref(&mut self, handle: i32) -> Result<(), Error> {
        self.pending_out_data
            .write_u32(BinderDriverCommandProtocol::DecRefs as u32)?;
        self.pending_out_data.write_i32(handle)?;
        Ok(())
    }

    /// Acquire the server side resource for the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn acquire(&mut self, handle: i32) -> Result<(), Error> {
        self.pending_out_data
            .write_u32(BinderDriverCommandProtocol::Acquire as u32)?;
        self.pending_out_data.write_i32(handle)?;
        Ok(())
    }

    /// Release the server side resource for the given handle. Note that this request is
    /// queued and only actually perfomed with the next outgoing transaction.
    pub fn release(&mut self, handle: i32) -> Result<(), Error> {
        self.pending_out_data
            .write_u32(BinderDriverCommandProtocol::Release as u32)?;
        self.pending_out_data.write_i32(handle)?;
        Ok(())
    }

    pub fn transact(
        &mut self,
        handle: i32,
        code: u32,
        flags: TransactionFlags,
        data: &mut Parcel,
    ) -> Result<(Option<BinderTransactionData>, Parcel), Error> {
        self.pending_out_data
            .write_i32(BinderDriverCommandProtocol::Transaction as i32)?;

        let transaction_data_out = BinderTransactionData {
            target: handle as u32,
            code,
            flags: (TransactionFlags::AcceptFds | flags).bits,
            cookie: 0,
            sender_pid: 0,
            sender_euid: 0,
            data_size: data.len() as u64,
            offset_size: (data.offsets_len() * size_of::<usize>()) as u64,
            data: if !data.is_empty() {
                data.as_mut_ptr()
            } else {
                std::ptr::null_mut()
            },
            offsets: if data.offsets_len() != 0 {
                data.offsets().as_mut_ptr()
            } else {
                std::ptr::null_mut()
            },
        };
        self.pending_out_data
            .write_transaction_data(&transaction_data_out)?;

        self.do_write_read(&mut Parcel::empty())
    }

    pub fn reply(
        &mut self,
        data: &mut Parcel,
        flags: TransactionFlags,
    ) -> Result<(Option<BinderTransactionData>, Parcel), Error> {
        self.pending_out_data
            .write_i32(BinderDriverCommandProtocol::Reply as i32)?;

        let transaction_data_out = BinderTransactionData {
            target: 0xffffffff,
            code: 0,
            flags: flags.bits,
            cookie: 0,
            sender_pid: 0,
            sender_euid: 0,
            data_size: data.len() as u64,
            offset_size: (data.offsets_len() * size_of::<usize>()) as u64,
            data: if !data.is_empty() {
                data.as_mut_ptr()
            } else {
                std::ptr::null_mut()
            },
            offsets: if data.offsets_len() != 0 {
                data.offsets().as_mut_ptr()
            } else {
                std::ptr::null_mut()
            },
        };
        self.pending_out_data
            .write_transaction_data(&transaction_data_out)?;

        self.do_write_read(&mut Parcel::empty())
    }

    pub fn do_write_read(
        &mut self,
        parcel_out: &mut Parcel,
    ) -> Result<(Option<BinderTransactionData>, Parcel), Error> {
        self.pending_out_data.append_parcel(parcel_out)?;
        let mut parcel_in = self.write_read(&self.pending_out_data, true);
        self.pending_out_data.reset();

        self.proccess_incoming(&mut parcel_in)
    }

    fn proccess_incoming(
        &mut self,
        parcel_in: &mut Parcel,
    ) -> Result<(Option<BinderTransactionData>, Parcel), Error> {
        while parcel_in.has_unread_data() {
            let cmd_u32 = parcel_in.read_u32()?;
            let cmd_option = BinderDriverReturnProtocol::from_u32(cmd_u32);
            if let Some(cmd) = cmd_option {
                match cmd {
                    BinderDriverReturnProtocol::TransactionComplete => {}
                    BinderDriverReturnProtocol::DeadReply => {
                        panic!("Got a DEAD_REPLY");
                    }
                    BinderDriverReturnProtocol::FailedReply => {
                        panic!("Transaction failed");
                    }
                    BinderDriverReturnProtocol::IncRefs => {
                        log::info!("binder: IncRefs ******************");
                    }
                    BinderDriverReturnProtocol::Acquire => {
                        log::info!("binder: Acquire ******************");
                    }
                    BinderDriverReturnProtocol::AcquireResult => {
                        log::info!("binder: AcquireResult ****************");
                        parcel_in.read_i32()?;
                    }
                    BinderDriverReturnProtocol::Reply | BinderDriverReturnProtocol::Transaction => {
                        let transaction_data_in = parcel_in.read_transaction_data()?;
                        let parcel = unsafe {
                            Parcel::from_data_and_offsets(
                                transaction_data_in.data,
                                transaction_data_in.data_size as usize,
                                transaction_data_in.offsets,
                                transaction_data_in.offset_size as usize / size_of::<usize>(),
                            )
                        };
                        return Ok((Some(transaction_data_in), parcel));
                    }
                    BinderDriverReturnProtocol::Error => {
                        println!("Got an error {}", parcel_in.read_i32()?);
                    }
                    BinderDriverReturnProtocol::Noop => {}
                    BinderDriverReturnProtocol::SpawnLooper => {}
                    _ => {}
                }
            }
        }

        Ok((None, Parcel::empty()))
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

        unsafe {
            binder_write_read(self.fd, &mut write_read_struct)
                .expect("Failed to perform write_read");
        }
        Parcel::from_slice(&data_in[..write_read_struct.read_consumed])
    }
}

/// Implement Drop for Binder, so that we can clean up resources
impl Drop for Binder {
    fn drop(&mut self) {
        //TODO: do we need to unmap?

        self.exit_looper().unwrap();

        close(self.fd).unwrap();
    }
}
