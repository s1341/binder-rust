extern crate binder_rust;
use binder_rust::{Parcel, ServiceManager};

fn main() {
    let mut service_manager = &mut ServiceManager::new();

    let mut package_manager = service_manager.get_service("myservice", "com.example.IMyService");

    let mut parcel = Parcel::empty();
    parcel.write_str16("Hello World");
    let mut res = package_manager.call(1, &mut parcel);
    println!("response: {:?}", res.read_str16());
}

