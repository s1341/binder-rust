use crate::{
    Error,
    binder::{Binder, BinderFlatObject, Transaction, TransactionFlags},
    parcel::Parcel,
    parcelable::Parcelable,
};

use std::ffi::c_void;
use std::marker::PhantomData;

use num_traits::FromPrimitive;

const SERVICE_MANAGER_HANDLE: i32 = 0;
const SERVICE_MANAGER_INTERFACE_TOKEN: &str = "android.os.IServiceManager";

enum ServiceManagerFunctions {
    GetService = 1,
    CheckService = 2,
    AddService = 3,
    ListServices = 4,
}

pub struct Service<'a> {
    service_manager: &'a mut ServiceManager<'a>,
    handle: i32,
    _name: &'a str,
    interface_name: &'a str,
}

impl<'a> Service<'a> {
    pub fn new(service_manager: &'a mut ServiceManager<'a>, _name: &'a str, interface_name: &'a str, handle: i32) -> Self {
        Self {
            service_manager,
            _name,
            interface_name,
            handle,
        }
    }
    pub fn call(&mut self, function_index: u32, data: &mut Parcel) -> Result<Parcel, Error> {
        let mut parcel = Parcel::empty();
        parcel.write_interface_token(self.interface_name)?;
        if !data.is_empty() {
            parcel.append_parcel(data)?;
        };

        let (_, mut parcel) = self
            .service_manager
            .binder
            .transact(self.handle, function_index, TransactionFlags::AcceptFds |TransactionFlags::CollectNotedAppOps, &mut parcel)?;

        let status = parcel.read_u32()?;
        if status != 0 {
            panic!(
                "service call failed with status: {:x}, {} - {}\n{}",
                status,
                parcel.read_str16()?,
                parcel.read_u32()?,
                parcel.read_str16()?
            );
        };

        Ok(parcel)
    }
}

pub trait BinderService {
    fn process_request(&self, code: u32, data: &mut Parcel) -> Parcel;
}

pub struct ServiceListener<'a, BS>
where
    BS: BinderService,
{
    service_delegate: &'a BS,
    service_manager: &'a mut ServiceManager<'a>,
    _name: &'a str,
    interface_name: &'a str,
}

impl<'a, BS> ServiceListener<'a, BS>
where
    BS: BinderService,
{
    pub fn new(service_delegate: &'a BS, service_manager: &'a mut ServiceManager<'a>, _name: &'a str, interface_name: &'a str) -> Self {
        Self {
            service_delegate,
            service_manager,
            _name,
            interface_name,
        }
    }

    pub fn run(&mut self) -> Result<(), Error>{
        loop {
            let (transaction, mut parcel) = self.service_manager.binder.do_write_read(&mut Parcel::empty())?;
            if let Some(transaction) = transaction {
                if transaction.code() >= Transaction::FirstCall as u32 && transaction.code() <= Transaction::LastCall as u32 {
                    assert!(parcel.read_interface_token()? == self.interface_name);
                    self.service_manager.binder.reply(&mut self.service_delegate.process_request(transaction.code(), &mut parcel), transaction.flags())?;
                } else if let Transaction::Interface =  Transaction::from_u32(transaction.code()).unwrap() {
                    let mut parcel = Parcel::empty();
                    parcel.write_u32(0)?;
                    parcel.write_str16(self.interface_name)?;
                    self.service_manager.binder.reply(&mut parcel, transaction.flags() | TransactionFlags::AcceptFds)?;
                }
            }
        }
    }
}

pub struct ServiceManager<'a> {
    binder: Binder,
    _phantom: &'a PhantomData<Binder>
}

impl<'a> ServiceManager<'a> {
    pub fn new() -> Result<Self, Error> {
        let mut service_manager = Self {
            binder: Binder::new(),
            _phantom: &PhantomData,
        };

        service_manager.ping()?;

        Ok(service_manager)
    }

    fn ping(&mut self) -> Result<(), Error>{
        let mut parcel = Parcel::empty();
        self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            Transaction::Ping as u32,
            TransactionFlags::empty(),
            &mut parcel,
        )?;
        Ok(())
    }

    pub fn get_service(&'a mut self, service_name: &'a str, interface_name: &'a str) -> Result<Service<'a>, Error> {
        let mut parcel = Parcel::empty();
        parcel.write_interface_token(SERVICE_MANAGER_INTERFACE_TOKEN)?;
        parcel.write_str16(service_name)?;
        let (_transaction, mut parcel) = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::GetService as u32,
            TransactionFlags::empty(),
            &mut parcel,
        )?;
        parcel.read_u32()?;
        let flat_object = BinderFlatObject::deserialize(&mut parcel)?;

        self.binder.add_ref(flat_object.handle as i32)?;
        self.binder.acquire(flat_object.handle as i32)?;

        Ok(Service::new(self, service_name, interface_name, flat_object.handle as i32))
    }

    pub fn register_service<BS: BinderService> (
        &'a mut self,
        service_delegate: &'a BS,
        name: &'a str,
        interface_name: &'a str,
        allow_isolated: bool,
        dump_priority: u32,
    ) -> Result<ServiceListener<'a, BS>, Error> {

        self.binder.enter_looper()?;

        let mut parcel = Parcel::empty();
        parcel.write_interface_token(SERVICE_MANAGER_INTERFACE_TOKEN)?;
        parcel.write_str16(name)?;
        parcel.write_binder(self as *const _ as *const c_void)?;
        parcel.write_bool(allow_isolated)?;
        parcel.write_u32(dump_priority)?;

        let (_transaction, _parcel) = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::AddService as u32,
            TransactionFlags::empty(),
            &mut parcel,
        )?;

        Ok(ServiceListener::new(service_delegate, self, name, interface_name))
    }
}
