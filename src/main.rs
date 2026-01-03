#[macro_use] extern crate rocket;

use std::error::Error;
use rocket_db_pools::{Connection, Database, sqlx::{self, Row}};
use rocket::{http::Status, serde::{Deserialize, Serialize, json::{Json, Value}}, response::status};
// use rocket::response::status;
// use rocket_db_pools::{sqlx, Database};

// Define the database connection pool
#[derive(Database)]
#[database("postgres")]
struct Db(sqlx::PgPool);

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct NewDeviceMessage<'r> {
    device_id: &'r str,
    configuration: Value,
}


#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}


// Device management routes
#[get("/devices")]
async fn get_devices(mut db: Connection<Db>) -> Result<Json<Vec<serde_json::Value>>, Status> {
    let rows = sqlx::query("SELECT device_id FROM devices").fetch_all(&mut **db).await.map_err(|_| Status::InternalServerError)?;
    let devices : Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "device_id": row.get::<String, _>("device_id"),
            })
        })
        .collect();
    Ok(Json(devices))

}

#[post("/devices", format = "json", data = "<message>")]
async fn create_device(mut db: Connection<Db>, message : Json<NewDeviceMessage<'_>>) -> Result<Status, Status> {
    println!("Creating device with ID: {}", message.device_id);
    let result = sqlx::query("INSERT INTO devices (device_id, configuration) VALUES ($1, $2)")
        .bind(message.device_id)
        .bind(message.configuration.clone())
        .execute(&mut **db)
        .await;

    match result {
        Ok(_) => {
            println!("Creating device with ID: {}", message.device_id);
            Ok(Status::Created)
        }
        Err(e) => {
            use sqlx::Error;
            match e {
                Error::Database(db_err) => {
                    if db_err.message().contains("duplicate key value") {
                        println!("Device with ID {} already exists", message.device_id);
                        return Err(Status::Conflict);
                    }
            
                    println!("Failed to create device: {}", db_err.message());
                    Err(Status::BadRequest)
                }
                _ => {
                    println!("Failed to create device: {}", e);
                    Err(Status::BadRequest)
                }
            }
        }
    }
        
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
