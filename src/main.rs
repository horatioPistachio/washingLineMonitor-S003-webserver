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
async fn get_device(mut db: Connection<Db>, device_id: String) -> Result<Json<serde_json::Value>, Status> {
    let row = sqlx::query("SELECT device_id, configuration FROM devices WHERE device_id = $1").bind(&device_id).fetch_optional(&mut **db).await.map_err(|_| Status::InternalServerError)?;
    match row {
        Some(row) => {
            let device = serde_json::json!({
                "device_id": row.get::<String, _>("device_id"),
                "configuration": row.get::<serde_json::Value, _>("configuration"),
            });
            Ok(Json(device))
        }
        None => {
            println!("Device with ID {} not found", device_id);
            Err(Status::NotFound)
        }
    }
} 

#[delete("/devices/<device_id>")]
async fn delete_device(mut db: Connection<Db>, device_id: String) -> Result<Status, Status> {
    let row = sqlx::query("DELETE FROM devices WHERE device_id = $1 RETURNING device_id")
        .bind(&device_id)
        .fetch_optional(&mut **db)
        .await
        .map_err(|_| Status::InternalServerError)?;

    match row {
        Some(_) => {
            Ok(Status::NoContent)
        }
        None => {
            println!("Device with ID {} not found for deletion", device_id);
            Err(Status::NotFound)
        }
    }
}

#[patch("/devices/<device_id>", format = "json", data = "<message>")]
async fn update_device_configuration(mut db: Connection<Db>, device_id: String, message : Json<NewDeviceMessage<'_>>) -> Result<Status, Status> {
    let row = sqlx::query("UPDATE devices SET configuration = $1 WHERE device_id = $2 RETURNING device_id")
        .bind(&message.configuration)
        .bind(&device_id)
        .fetch_optional(&mut **db)
        .await
        .map_err(|_| Status::InternalServerError)?;

    match row {
        Some(_) => {
            Ok(Status::Ok)
        }
        None => {
            println!("Device with ID {} not found for update", device_id);
            Err(Status::NotFound)
        }
    }

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
