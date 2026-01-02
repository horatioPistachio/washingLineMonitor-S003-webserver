# High level Design Document
This project is a web server the that runs a basic api to take data from the washing line monitors, stores and processes the data, and then sends notifications to a phone when the washing is done. The projects two main purposes are to have a cleaner, purpose built server the washing line monitor (rather than using the hydro server) and to teach me about Rust (in preparation to make changes to chirpstack) and Docker (in preparation to containerise applications at work).

## Architecture
The web server is built in Rust using the Rocket framework. 
It uses a Postgres database to store data about the washing line monitors. 
The server exposes a REST API to receive data from the washing line monitors (esp32 over wifi).
The phone notifications are sent using the ntfy.sh service.
The server is containerised using Docker, making it easy to deploy and manage.

## Database schema
The database schema consists of the following tables:
- devices
    - id (int, primary key)
    - device_id (char(16), unique)
    - created_at (timestampz)
    - last_notification_at (timestampz, nullable)
    - configuration (jsonb) 
device is index by device_id for fast lookup when data is received from the washing line monitor.

- telemetry
    - id (bigint, primary key)
    - timestamp (timestampz)
    - device_id (char(16), foreign key to devices.device_id)
    - payload (jsonb)
The telemetry table stores the data received from the washing line monitors, including a timestamp and the device_id to link it to the corresponding device. device id is a foreign key to ensure data integrity. cascade delete is used to remove telemetry data when a device is deleted.

## API Endpoints
The web server exposes the following REST API endpoints:

### Device Endpoints
- POST /api/v1/devices
    - Registers a new washing line monitor device.
    - Request body: JSON object containing device_id and configuration.
    - Response: 201 Created on success, 400 Bad Request on failure.
- GET /api/v1/devices
    - Retrieves a list of all registered devices.
    - Response: 200 OK with JSON array of devices.
- GET /api/v1/devices/{device_id}
    - Retrieves the configuration of a specific device.
    - Response: 200 OK with JSON object containing device configuration, 404 Not Found if device does not exist.
- DELETE /api/v1/devices/{device_id}
    - Deletes a specific device and its associated telemetry data.
    - Response: 200 OK on success, 404 Not Found if device does not exist.
- PATCH /api/v1/devices/{device_id}
    - Updates the configuration of a specific device.
    - Request body: JSON object containing updated configuration.
    - Response: 200 OK on success, 400 Bad Request on failure, 404 Not Found if device does not exist.

### Telemetry Endpoints
- POST /api/v1/telemetry
    - Receives telemetry data from the washing line monitors.
    - Request body: JSON object containing device_id and payload.
    - Response: 201 OK on success, 400 Bad Request on failure.

- GET /api/v1/telemetry/{device_id}
    - Retrieves telemetry data for a specific device.
    - Response: 200 OK with JSON array of telemetry data, 404 Not Found if device does not exist.
    - Takes optional query parameters for start_time and end_time to filter results.

## Notification System
The notification system is responsible for sending notifications to the user's phone when the washing is done. Done is determined by analysing the telemetry data received from the washing line monitors. 
Every time new telemetry data is received, the server runs another tokio::spawn task to analyse the data in the background. If the analysis determines that the washing is done, a notification is sent using the ntfy.sh service(simple post request to the ntfy.sh api with the notification message).
Each device has a last_notification_at timestamp to prevent sending duplicate notifications within a configurable time window (e.g., 1 hour).

### Completion Detection Algorithm
First pass alrogrithm check for the textile resistacnce to be above a certain threshold for a configurable period of time (e.g., 5 minutes). If the resistance remains above the threshold for the entire period, the washing is considered done.   



## Frontend
No frontend is planned for this project. All interactions with the server will be done via the REST API. Future plans may include a simple web interface to view device statuses and telemetry data.
