use crate::{
    binder::{Binder, BinderFlatObject, Transaction},
    parcel::Parcel,
};

use std::cell::RefCell;
use std::marker::PhantomData;

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
            parcel.write(data.to_slice());
        };

        let mut parcel = self
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

pub struct ServiceListener<'a> {
    service_manager: &'a ServiceManager<'a>,
    name: &'a str,
    interface_name: &'a str,
}

impl<'a> ServiceListener<'a> {
    pub fn new(service_manager: &'a ServiceManager<'a>, name: &'a str, interface_name: &'a str) -> Self {
        Self {
            service_manager,
            name,
            interface_name,
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
        let mut result = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::GetService as u32,
            0,
            &mut parcel,
        );
        println!("res: {:?}", result);
        result.read_u32();
        let flat_object: BinderFlatObject = result.read_object();

        self.binder.add_ref(flat_object.handle as i32);
        self.binder.acquire(flat_object.handle as i32);

        Service::new(self, service_name, interface_name, flat_object.handle as i32)
    }

    pub fn register_service(
        &'a mut self,
        name: &'a str,
        interface_name: &'a str,
        allow_isolated: bool,
        dump_priority: u32,
    ) -> ServiceListener<'a> {

        self.binder.enter_looper();

        let mut parcel = Parcel::empty();
        parcel.write_interface_token(SERVICE_MANAGER_INTERFACE_TOKEN);
        parcel.write_str16(name);
        parcel.write_object(BinderFlatObject::new(self as *const _ as usize, 0, 0));
        parcel.write_bool(allow_isolated);
        parcel.write_u32(dump_priority);

        println!("parcel: {:?}", parcel);
        let mut result = self.binder.transact(
            SERVICE_MANAGER_HANDLE,
            ServiceManagerFunctions::AddService as u32,
            0,
            &mut parcel,
        );
        println!("result: {:?}", result);

        ServiceListener::new(self , name, interface_name)
    }
}
