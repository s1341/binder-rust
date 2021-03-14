/// Implements a simple service which echos any string it receives. Requires root to run.

use binder_rust::{Binder, BinderService, Parcel, ServiceManager};

#[macro_use]
extern crate num_derive;

use nix::libc::{open, O_CREAT, O_RDWR};
use num_traits::FromPrimitive;

struct MyService {

}

#[repr(u32)]
#[derive(Debug, FromPrimitive)]
enum MyServiceCommands {
    Echo = 1,
    GetFile = 2,
}

impl BinderService for MyService {
    fn process_request(&self, code: u32, data: &mut Parcel) -> Parcel {
        println!("Got command: {} -> {:?}", code, MyServiceCommands::from_u32(code));
        match MyServiceCommands::from_u32(code).unwrap() {
            MyServiceCommands::GetFile => {
                let filename = &std::ffi::CString::new(data.read_str16()).unwrap();
                let fd = unsafe { open(filename.as_ptr(), O_RDWR | O_CREAT) };
                println!("filename: {:?}, fd: {}", filename, fd);
                let mut parcel = Parcel::empty();
                parcel.write_u32(0);
                parcel.write_file_descriptor(fd, false);
                parcel
            },
            MyServiceCommands::Echo => {
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

