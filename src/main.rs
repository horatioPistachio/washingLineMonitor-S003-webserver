#[macro_use]
extern crate rocket;

use rocket::{
    http::Status,
    serde::{
        Deserialize, Serialize,
        json::{Json, Value},
    },
    tokio,
};
use rocket_db_pools::{
    Connection, Database,
    sqlx::{self, FromRow, Row, types::chrono},
};
use std::error::Error;
use std::time::Duration;
// use rocket::response::status;
// use rocket_db_pools::{sqlx, Database};
mod trigger_algorithms;

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

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct NewTelemetryMessage<'r> {
    device_id: &'r str,
    payload: Value,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct TelemetryRecord {
    device_id: String,
    payload: serde_json::Value,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for TelemetryRecord {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(TelemetryRecord {
            // Use try_get to extract each column by name
            // The ? operator propagates errors if a column is missing or has wrong type
            device_id: row.try_get("device_id")?,
            payload: row.try_get("payload")?,
            timestamp: row.try_get("timestamp")?,
        })
    }
}

pub async fn process_telemetry(pool: sqlx::PgPool, device_id: String, _payload: Value) {
    println!("Processing telemetry for device {}", device_id);

    let  historical_telemetry_data = sqlx::query_as::<_, TelemetryRecord>(
        "SELECT device_id, payload, timestamp  FROM telemetry WHERE device_id = $1 AND timestamp >= $2 ORDER BY timestamp DESC")
        .bind(&device_id)
        .bind(chrono::Utc::now() - Duration::from_hours(1))
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Database error in process_telemetry for device '{}': {:?}", device_id, e);
            e
        });

    println!(
        "Got data: {}",
        historical_telemetry_data.as_ref().unwrap().len()
    );

    let data_point_list: Vec<_> = historical_telemetry_data
        .unwrap()
        .iter()
        .filter_map(|record| record.payload["temp"].as_f64())
        .collect();

    println!("Extracted data points: {:?}", data_point_list);

    if data_point_list.is_empty() {
        println!(
            "No temperature data points found for device {} in the last hour",
            device_id
        );
        return;
    }

    if trigger_algorithms::is_stable_resistance(&data_point_list, 0.01) {
        println!("Alert: Device {} reported stable resitance", device_id);
        // Here you could add code to send an alert (e.g., email, SMS, push notification)
        let ntfy_topic = std::env::var("NTFY_TOPIC").expect("NTFY_TOPCI must be set");
        let client = reqwest::Client::new();

        let response = client
            .post(format!("https://ntfy.sh/{}", ntfy_topic))
            .header("Title", "Washing Complete :)")
            .header("Priority", "default")
            .body(format!("Device {} reported stable resistance", device_id))
            .send()
            .await;

        if response.as_ref().unwrap().status().is_success() {
            println!("Alert sent successfully for device {}", device_id);
        } else {
            eprintln!(
                "Failed to send alert for device {}: {:?}",
                device_id, response
            );
        }
    }
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

// Device management routes
#[get("/devices")]
async fn get_devices(mut db: Connection<Db>) -> Result<Json<Vec<serde_json::Value>>, Status> {
    let rows = sqlx::query("SELECT device_id FROM devices")
        .fetch_all(&mut **db)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let devices: Vec<serde_json::Value> = rows
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
async fn create_device(
    mut db: Connection<Db>,
    message: Json<NewDeviceMessage<'_>>,
) -> Result<Status, Status> {
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
async fn get_device(
    mut db: Connection<Db>,
    device_id: String,
) -> Result<Json<serde_json::Value>, Status> {
    let row = sqlx::query("SELECT device_id, configuration FROM devices WHERE device_id = $1")
        .bind(&device_id)
        .fetch_optional(&mut **db)
        .await
        .map_err(|_| Status::InternalServerError)?;
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
        Some(_) => Ok(Status::NoContent),
        None => {
            println!("Device with ID {} not found for deletion", device_id);
            Err(Status::NotFound)
        }
    }
}

#[patch("/devices/<device_id>", format = "json", data = "<message>")]
async fn update_device_configuration(
    mut db: Connection<Db>,
    device_id: String,
    message: Json<NewDeviceMessage<'_>>,
) -> Result<Status, Status> {
    let row = sqlx::query(
        "UPDATE devices SET configuration = $1 WHERE device_id = $2 RETURNING device_id",
    )
    .bind(&message.configuration)
    .bind(&device_id)
    .fetch_optional(&mut **db)
    .await
    .map_err(|_| Status::InternalServerError)?;

    match row {
        Some(_) => Ok(Status::Ok),
        None => {
            println!("Device with ID {} not found for update", device_id);
            Err(Status::NotFound)
        }
    }
}

// Telemetry data routes
#[post("/telemetry", format = "json", data = "<message>")]
async fn post_telemetry(
    mut db: Connection<Db>,
    pool: &rocket::State<sqlx::PgPool>,
    message: Json<NewTelemetryMessage<'_>>,
) -> Result<Status, Status> {
    let _result = sqlx::query("INSERT INTO telemetry (device_id, payload) VALUES ($1, $2)")
        .bind(message.device_id)
        .bind(message.payload.clone())
        .execute(&mut **db)
        .await
        .map_err(|_| Status::InternalServerError)?;

    // start async processing of telemetry data here (e.g., spawn a task)

    let device_id = message.device_id.to_string(); // Convert &str to String for 'static lifetime
    let payload = message.payload.clone(); // Clone the JSON value
    let pool_clone = pool.inner().clone(); // Extract the underlying PgPool from the Connection wrapper

    tokio::spawn(async move {
        process_telemetry(pool_clone, device_id, payload).await;
    });

    Ok(Status::Created)
}

fn parse_timestamp(s: &str) -> Result<chrono::NaiveDateTime, Box<dyn Error>> {
    // Try multiple formats that clients might send
    // The ? operator propagates errors - if parsing fails, return Err immediately
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

#[get("/telemetry/<device_id>?<start_time>&<end_time>")]
async fn get_telemetry(
    mut db: Connection<Db>,
    device_id: &str,
    start_time: Option<String>,
    end_time: Option<String>,
) -> Result<Json<Vec<TelemetryRecord>>, Status> {
    let default_start = || {
        chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    };

    let default_end = || {
        chrono::NaiveDate::from_ymd_opt(2100, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    };

    let start = start_time
        .as_ref()
        .and_then(|s| parse_timestamp(s).ok())
        .unwrap_or_else(default_start);

    let end = end_time
        .as_ref()
        .and_then(|s| parse_timestamp(s).ok())
        .unwrap_or_else(default_end);

    let result = sqlx::query_as::<_, TelemetryRecord>(
        "SELECT device_id, payload, timestamp FROM telemetry
        WHERE device_id = $1
        AND timestamp >= $2
        AND timestamp <= $3
        ORDER BY timestamp DESC",
    )
    .bind(device_id)
    .bind(start)
    .bind(end)
    .fetch_all(&mut **db)
    .await
    .map_err(|e| {
        eprintln!(
            "Database error in get_telemetry for device '{}': {:?}",
            device_id, e
        );
        Status::InternalServerError
    })?;
    Ok(Json(result))
}

// Rocket launch
#[launch]
fn rocket() -> _ {
    dotenv::dotenv().ok();

    use rocket::fairing::AdHoc;
    use rocket_db_pools::Database;

    rocket::build()
        .attach(Db::init())
        .attach(AdHoc::on_ignite("Manage DB Pool", |rocket| async {
            if let Some(db) = Db::fetch(&rocket) {
                let pool = db.0.clone();
                rocket.manage(pool)
            } else {
                panic!("Failed to get database pool - make sure Db::init() is attached first");
            }
        }))
        .mount("/", routes![index])
        .mount(
            "/api/v1",
            routes![
                get_devices,
                create_device,
                get_device,
                delete_device,
                update_device_configuration,
                post_telemetry,
                get_telemetry
            ],
        )
}
