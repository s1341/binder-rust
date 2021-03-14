/// Implements a simple service which echos any string it receives. Requires root to run.

use binder_rust::{Binder, BinderService, Parcel, ServiceManager};

#[macro_use]
extern crate num_derive;
use num_traits::FromPrimitive;

struct MyService {

}

#[repr(u32)]
#[derive(Debug, FromPrimitive)]
enum MyServiceCommands {
    Echo = 1,
}

impl BinderService for MyService {
    fn process_request(&self, code: u32, data: &mut Parcel) -> Parcel {
        match MyServiceCommands::from_u32(code) {
            Echo => {
                let mut parcel = Parcel::empty();
                parcel.write_u32(0); //status
                parcel.write_str16(&data.read_str16());
                parcel
            }
        }
    }
}
fn main() {
    let mut service_manager = &mut ServiceManager::new();

    let myservice = MyService {};

    let mut service = service_manager.register_service(&myservice, "myservice", "com.example.IMyService", true, 0);

    service.run();
}

