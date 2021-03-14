use crate::{
    binder::{Binder, BinderFlatObject, BinderTransactionData, Transaction},
    parcel::Parcel,
};

use std::cell::RefCell;
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
    name: &'a str,
    interface_name: &'a str,
}

impl<'a> Service<'a> {
    pub fn new(service_manager: &'a mut ServiceManager<'a>, name: &'a str, interface_name: &'a str, handle: i32) -> Self {
        Self {
            service_manager,
            name,
            interface_name,
            handle,
        }
    }
    pub fn call(&mut self, function_index: u32, data: &mut Parcel) -> Parcel {
        let mut parcel = Parcel::empty();
        parcel.write_interface_token(self.interface_name);
        if data.len() > 0 {
            parcel.append_parcel(data);
        };

        let (_, mut parcel) = self
            .service_manager
            .binder
            .transact(self.handle, function_index, 0, &mut parcel);

        let status = parcel.read_u32();
        if status != 0 {
            panic!(
                "service call failed with status: {:x}, {} - {}\n{}",
                status,
                parcel.read_str16(),
                parcel.read_u32(),
                parcel.read_str16()
            );
        };

        parcel
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
    name: &'a str,
    interface_name: &'a str,
}

impl<'a, BS> ServiceListener<'a, BS>
where
    BS: BinderService,
{
    pub fn new(service_delegate: &'a BS, service_manager: &'a mut ServiceManager<'a>, name: &'a str, interface_name: &'a str) -> Self {
        Self {
            service_delegate,
            service_manager,
            name,
            interface_name,
        }
    }

    pub fn run(&mut self){
        loop {

            let (transaction, mut parcel) = self.service_manager.binder.do_write_read(&mut Parcel::empty());
            match transaction {
                Some(transaction) => {
                    if transaction.code() >= Transaction::FirstCall as u32 && transaction.code() <= Transaction::LastCall as u32 {
                        assert!(&parcel.read_interface_token() == self.interface_name);
                        self.service_manager.binder.reply(&mut self.service_delegate.process_request(transaction.code(), &mut parcel), transaction.flags());
                    } else {
                        match Transaction::from_u32(transaction.code()) {
                            Interface => {
                                let mut parcel = Parcel::empty();
                                parcel.write_str16(self.interface_name);
                                self.service_manager.binder.reply(&mut parcel, transaction.flags());
                            }
                            _ => {}
                        }
                    }
                },
                None => {}
            }
        }
        }
}

pub struct ServiceManager<'a> {
    binder: Binder,
    phantom: &'a PhantomData<Binder>
}

impl<'a> ServiceManager<'a> {
    pub fn new() -> Self {
        let mut service_manager = Self {
            binder: Binder::new(),
            phantom: &PhantomData,
        };

        service_manager.ping();

        service_manager
    }

    fn ping(&mut self) {
        let mut parcel = Parcel::empty();
        self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            Transaction::Ping as u32,
            0,
            &mut parcel,
        );
    }

    pub fn get_service(&'a mut self, service_name: &'a str, interface_name: &'a str) -> Service<'a> {
        let mut parcel = Parcel::empty();
        parcel.write_interface_token(SERVICE_MANAGER_INTERFACE_TOKEN);
        parcel.write_str16(service_name);
        let (transaction, mut parcel) = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::GetService as u32,
            0,
            &mut parcel,
        );
        println!("res: {:?}\n{:?}", transaction, parcel);
        parcel.read_u32();
        let flat_object: BinderFlatObject = parcel.read_object();

        self.binder.add_ref(flat_object.handle as i32);
        self.binder.acquire(flat_object.handle as i32);

        Service::new(self, service_name, interface_name, flat_object.handle as i32)
    }

    pub fn register_service<BS: BinderService>(
        &'a mut self,
        service_delegate: &'a BS,
        name: &'a str,
        interface_name: &'a str,
        allow_isolated: bool,
        dump_priority: u32,
    ) -> ServiceListener<'a, BS> {

        self.binder.enter_looper();

        let mut parcel = Parcel::empty();
        parcel.write_interface_token(SERVICE_MANAGER_INTERFACE_TOKEN);
        parcel.write_str16(name);
        parcel.write_object(BinderFlatObject::new(self as *const _ as usize, 0, 0));
        parcel.write_u32(0xc); // stability  == SYSTEM
        parcel.write_bool(allow_isolated);
        parcel.write_u32(dump_priority);

        println!("parcel: {:?}", parcel);
        let (transaction, mut parcel) = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::AddService as u32,
            0,
            &mut parcel,
        );
        println!("result: {:?}\n{:?}", transaction, parcel);

        ServiceListener::new(service_delegate, self, name, interface_name)
    }
}
