CREATE TABLE IF NOT EXISTS race_sessions (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP,
    total_distance FLOAT NOT NULL,
    status VARCHAR(50) NOT NULL,
    config JSONB DEFAULT '{}'::JSONB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS vehicles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model VARCHAR(255) NOT NULL,
    battery_capacity FLOAT NOT NULL,
    max_speed FLOAT NOT NULL,
    weight FLOAT NOT NULL,
    config JSONB DEFAULT '{}'::JSONB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS laps (
    id SERIAL PRIMARY KEY,
    race_session_id INTEGER REFERENCES race_sessions(id),
    lap_number INTEGER NOT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP,
    duration FLOAT,
    avg_speed FLOAT,
    energy_consumed FLOAT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS system_events (
    id SERIAL PRIMARY KEY,
    race_session_id INTEGER REFERENCES race_sessions(id),
    event_time TIMESTAMP NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    event_source VARCHAR(50) NOT NULL,
    message TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert sample vehicle
INSERT INTO vehicles (name, model, battery_capacity, max_speed, weight, config)
VALUES 
('EV Racer 1', 'Performance Model', 60.0, 180.0, 1200.0, '{"motor_type": "PMSM", "cooling_type": "liquid"}'),
('EV Racer 2', 'Endurance Model', 75.0, 160.0, 1300.0, '{"motor_type": "PMSM", "cooling_type": "liquid", "energy_recovery": "high"}');