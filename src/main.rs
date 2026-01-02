#[macro_use] extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}


// Device management routes
#[get("/devices")]
fn get_devices() -> &'static str {
    "List of devices"
}

#[post("/devices")]
fn create_device() -> &'static str {
    "Device created"
}

#[get("/devices/<device_id>")]
fn get_device(device_id: String) -> String {
    format!("Device ID: {}", device_id)
} 

#[delete("/devices/<device_id>")]
fn delete_device(device_id: String) -> String {
    format!("Deleted Device ID: {}", device_id)
}

#[patch("/devices/<device_id>")]
fn update_device_configuration(device_id: String) -> String {
    format!("Updated configuration for Device ID: {}", device_id)
}

// Telemetry data routes
#[post("/telemetry")]
fn post_telemetry() -> &'static str {
    "Telemetry data received"
}

#[get("/telemetry/<device_id>")]
fn get_telemetry(device_id: String) -> String {
    format!("Telemetry data for Device ID: {}", device_id)
}


// Rocket launch
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index])
        .mount("/api/v1", routes![
            get_devices,
            create_device, 
            get_device, 
            delete_device, 
            update_device_configuration,
            post_telemetry,
            get_telemetry
            ])

}
