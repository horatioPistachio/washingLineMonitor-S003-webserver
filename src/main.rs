#[macro_use] extern crate rocket;

use rocket_db_pools::{Connection, Database, sqlx::{self, Row, postgres::PgRow}};
// use rocket_db_pools::{sqlx, Database};

// Define the database connection pool
#[derive(Database)]
#[database("postgres")]
struct Db(sqlx::PgPool);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}


// Device management routes
#[get("/devices")]
async fn get_devices(mut db: Connection<Db>) -> &'static str {
    let rows = sqlx::query("SELECT device_id FROM devices").fetch_all(&mut **db).await.unwrap();
    for row in rows {
        let device_id: &str = row.get("device_id");
        println!("Device ID: {}", device_id);
    }
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
    dotenv::dotenv().ok();
    rocket::build()
        .attach(Db::init())
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
