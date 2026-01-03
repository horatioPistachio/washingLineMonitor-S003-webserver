-- Create devices table
CREATE TABLE devices (
    id SERIAL PRIMARY KEY,
    device_id CHAR(8) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_notification_at TIMESTAMPTZ,
    configuration JSONB NOT NULL DEFAULT '{}'::jsonb
);

-- Create index on device_id for fast lookups
CREATE INDEX idx_devices_device_id ON devices(device_id);

-- Create telemetry table
CREATE TABLE telemetry (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    device_id CHAR(8) NOT NULL,
    payload JSONB NOT NULL,
    CONSTRAINT fk_device
        FOREIGN KEY(device_id) 
        REFERENCES devices(device_id)
        ON DELETE CASCADE
);

-- Create index on device_id and timestamp for efficient queries
CREATE INDEX idx_telemetry_device_timestamp ON telemetry(device_id, timestamp DESC);