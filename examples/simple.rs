extern crate binder_rust;
use binder_rust::{Parcel, ServiceManager};

fn main() {
    let mut service_manager = &mut ServiceManager::new();

    let mut package_manager = service_manager.get_service("myservice", "com.example.IMyService");

    let mut parcel = Parcel::empty();
    parcel.write_str16("Hello World");
    let mut res = package_manager.call(1, &mut parcel);
    println!("response: {:?}", res.read_str16());

    let mut parcel = Parcel::empty();
    parcel.write_str16("/data/local/tmp/testfile");
    let mut res = package_manager.call(2, &mut parcel);
    let fd = res.read_file_descriptor();
    unsafe {
        nix::libc::write(fd, "Hello world".as_ptr() as *const std::ffi::c_void, 11);
    }
}

