extern crate binder_rust;
use binder_rust::{Parcel, ServiceManager};

fn main() {
    let mut service_manager = ServiceManager::new().unwrap();

    let mut package_manager = service_manager.get_service("myservice", "com.example.IMyService").unwrap();

    let mut parcel = Parcel::empty();
    let _ = parcel.write_str16("Hello World");
    let res = package_manager.call(1, &mut parcel);
    println!("response: {:?}", res.unwrap().read_str16());

    let mut parcel = Parcel::empty();
    let _ = parcel.write_str16("/data/local/tmp/testfile");
    let mut res = package_manager.call(2, &mut parcel).unwrap();
    let fd = res.read_file_descriptor().unwrap();
    unsafe {
        nix::libc::write(fd, "Hello world".as_ptr() as *const std::ffi::c_void, 11);
    }
}

