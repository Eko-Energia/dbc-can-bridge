"""
EV Racing Dashboard Backend API
Built with FastAPI, PostgreSQL and InfluxDB

- PostgreSQL: Used for storing configuration, user data, race metadata, and other non-time-series data
- InfluxDB: Used for storing high-frequency time-series data (speed, battery status, temperature, etc.)
"""

from fastapi import FastAPI, HTTPException, Depends, BackgroundTasks, Query
from fastapi.middleware.cors import CORSMiddleware
from typing import List, Dict, Any, Optional
from pydantic import BaseModel
from datetime import datetime, timedelta
import databases
import sqlalchemy
import influxdb_client
from influxdb_client import InfluxDBClient, Point
from influxdb_client.client.write_api import SYNCHRONOUS
import os
import json
import asyncio
import random

# Configuration
DATABASE_URL = os.getenv("DATABASE_URL", "postgresql://postgres:postgres@db:5432/racing")
INFLUXDB_URL = os.getenv("INFLUXDB_URL", "http://influxdb:8086")
INFLUXDB_TOKEN = os.getenv("INFLUXDB_TOKEN", "my-token")
INFLUXDB_ORG = os.getenv("INFLUXDB_ORG", "racing-org")
INFLUXDB_BUCKET = os.getenv("INFLUXDB_BUCKET", "racing-data")

# Initialize FastAPI
app = FastAPI(title="EV Racing Dashboard API")

# Update CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# PostgreSQL Database setup
database = databases.Database(DATABASE_URL)
metadata = sqlalchemy.MetaData()

# Define PostgreSQL tables
race_sessions = sqlalchemy.Table(
    "race_sessions",
    metadata,
    sqlalchemy.Column("id", sqlalchemy.Integer, primary_key=True),
    sqlalchemy.Column("name", sqlalchemy.String),
    sqlalchemy.Column("start_time", sqlalchemy.DateTime),
    sqlalchemy.Column("end_time", sqlalchemy.DateTime, nullable=True),
    sqlalchemy.Column("total_distance", sqlalchemy.Float),
    sqlalchemy.Column("status", sqlalchemy.String),
    sqlalchemy.Column("config", sqlalchemy.JSON),
    sqlalchemy.Column("created_at", sqlalchemy.DateTime, default=datetime.utcnow),
    sqlalchemy.Column("updated_at", sqlalchemy.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow),
)

vehicles = sqlalchemy.Table(
    "vehicles",
    metadata,
    sqlalchemy.Column("id", sqlalchemy.Integer, primary_key=True),
    sqlalchemy.Column("name", sqlalchemy.String),
    sqlalchemy.Column("model", sqlalchemy.String),
    sqlalchemy.Column("battery_capacity", sqlalchemy.Float),  # kWh
    sqlalchemy.Column("max_speed", sqlalchemy.Float),  # km/h
    sqlalchemy.Column("weight", sqlalchemy.Float),  # kg
    sqlalchemy.Column("config", sqlalchemy.JSON),
    sqlalchemy.Column("created_at", sqlalchemy.DateTime, default=datetime.utcnow),
    sqlalchemy.Column("updated_at", sqlalchemy.DateTime, default=datetime.utcnow, onupdate=datetime.utcnow),
)

laps = sqlalchemy.Table(
    "laps",
    metadata,
    sqlalchemy.Column("id", sqlalchemy.Integer, primary_key=True),
    sqlalchemy.Column("race_session_id", sqlalchemy.ForeignKey("race_sessions.id")),
    sqlalchemy.Column("lap_number", sqlalchemy.Integer),
    sqlalchemy.Column("start_time", sqlalchemy.DateTime),
    sqlalchemy.Column("end_time", sqlalchemy.DateTime, nullable=True),
    sqlalchemy.Column("duration", sqlalchemy.Float, nullable=True),  # seconds
    sqlalchemy.Column("avg_speed", sqlalchemy.Float, nullable=True),  # km/h
    sqlalchemy.Column("energy_consumed", sqlalchemy.Float, nullable=True),  # kWh
    sqlalchemy.Column("created_at", sqlalchemy.DateTime, default=datetime.utcnow),
)

system_events = sqlalchemy.Table(
    "system_events",
    metadata,
    sqlalchemy.Column("id", sqlalchemy.Integer, primary_key=True),
    sqlalchemy.Column("race_session_id", sqlalchemy.ForeignKey("race_sessions.id")),
    sqlalchemy.Column("event_time", sqlalchemy.DateTime),
    sqlalchemy.Column("event_type", sqlalchemy.String),  # warning, error, info
    sqlalchemy.Column("event_source", sqlalchemy.String),  # sensor, battery, motor, etc.
    sqlalchemy.Column("message", sqlalchemy.String),
    sqlalchemy.Column("details", sqlalchemy.JSON, nullable=True),
    sqlalchemy.Column("created_at", sqlalchemy.DateTime, default=datetime.utcnow),
)

# Pydantic models for request/response validation
class RaceSessionCreate(BaseModel):
    name: str
    vehicle_id: int
    total_distance: float
    config: Optional[Dict[str, Any]] = {}

class RaceSessionUpdate(BaseModel):
    status: Optional[str] = None
    end_time: Optional[datetime] = None
    config: Optional[Dict[str, Any]] = None

class RaceSessionResponse(BaseModel):
    id: int
    name: str
    start_time: datetime
    end_time: Optional[datetime] = None
    total_distance: float
    status: str
    config: Dict[str, Any]
    created_at: datetime
    updated_at: datetime

class VehicleCreate(BaseModel):
    name: str
    model: str
    battery_capacity: float
    max_speed: float
    weight: float
    config: Optional[Dict[str, Any]] = {}

class VehicleResponse(BaseModel):
    id: int
    name: str
    model: str
    battery_capacity: float
    max_speed: float
    weight: float
    config: Dict[str, Any]
    created_at: datetime
    updated_at: datetime

class LapCreate(BaseModel):
    race_session_id: int
    lap_number: int
    start_time: datetime

class LapUpdate(BaseModel):
    end_time: datetime
    duration: float
    avg_speed: float
    energy_consumed: float

class LapResponse(BaseModel):
    id: int
    race_session_id: int
    lap_number: int
    start_time: datetime
    end_time: Optional[datetime] = None
    duration: Optional[float] = None
    avg_speed: Optional[float] = None
    energy_consumed: Optional[float] = None
    created_at: datetime

class SystemEventCreate(BaseModel):
    race_session_id: int
    event_time: datetime
    event_type: str
    event_source: str
    message: str
    details: Optional[Dict[str, Any]] = None

class SystemEventResponse(BaseModel):
    id: int
    race_session_id: int
    event_time: datetime
    event_type: str
    event_source: str
    message: str
    details: Optional[Dict[str, Any]] = None
    created_at: datetime

class TelemetryData(BaseModel):
    race_session_id: int
    timestamp: datetime
    speed: float
    battery_soc: float
    battery_temperature: float
    battery_voltage: float
    motor_temperature: float
    energy_consumption: float
    distance_traveled: float

class DashboardData(BaseModel):
    current_speed: float
    avg_speed: float
    battery_soc: float
    battery_temp: Dict[str, float]
    battery_voltage: Dict[str, float]
    suggested_speed: float
    remaining_time: str
    distance_covered: float
    progress_percent: float
    estimated_range: float
    estimated_time: str
    energy_consumption: float
    system_status: str
    warnings: int
    errors: int
    sensors_status: str
    communication_status: str

# InfluxDB connection
def get_influxdb_client():
    client = InfluxDBClient(
        url=INFLUXDB_URL,
        token=INFLUXDB_TOKEN,
        org=INFLUXDB_ORG
    )
    return client

# Event handlers
@app.on_event("startup")
async def startup():
    # Add retry logic for database connection
    retry_count = 0
    max_retries = 5
    while retry_count < max_retries:
        try:
            await database.connect()
            print("Database connection established successfully")
            break
        except Exception as e:
            retry_count += 1
            print(f"Connection attempt {retry_count} failed: {e}")
            if retry_count >= max_retries:
                print("Maximum retry attempts reached. Could not connect to database.")
                raise
            await asyncio.sleep(5)  # Wait 5 seconds before retrying

@app.on_event("shutdown")
async def shutdown():
    await database.disconnect()

# API Routes

# Race Sessions
@app.post("/race-sessions/", response_model=RaceSessionResponse)
async def create_race_session(race_session: RaceSessionCreate):
    now = datetime.utcnow()
    query = race_sessions.insert().values(
        name=race_session.name,
        start_time=now,
        total_distance=race_session.total_distance,
        status="active",
        config=race_session.config,
        created_at=now,
        updated_at=now
    )
    session_id = await database.execute(query)
    
    return {**race_session.dict(), "id": session_id, "start_time": now, "end_time": None, 
            "status": "active", "created_at": now, "updated_at": now}

@app.get("/race-sessions/", response_model=List[RaceSessionResponse])
async def get_race_sessions():
    query = race_sessions.select()
    return await database.fetch_all(query)

@app.get("/race-sessions/{session_id}", response_model=RaceSessionResponse)
async def get_race_session(session_id: int):
    query = race_sessions.select().where(race_sessions.c.id == session_id)
    result = await database.fetch_one(query)
    if not result:
        raise HTTPException(status_code=404, detail="Race session not found")
    return result

@app.put("/race-sessions/{session_id}", response_model=RaceSessionResponse)
async def update_race_session(session_id: int, session_update: RaceSessionUpdate):
    query = race_sessions.select().where(race_sessions.c.id == session_id)
    existing = await database.fetch_one(query)
    if not existing:
        raise HTTPException(status_code=404, detail="Race session not found")
    
    update_data = {k: v for k, v in session_update.dict(exclude_unset=True).items() if v is not None}
    update_data["updated_at"] = datetime.utcnow()
    
    query = race_sessions.update().where(race_sessions.c.id == session_id).values(**update_data)
    await database.execute(query)
    
    return await database.fetch_one(race_sessions.select().where(race_sessions.c.id == session_id))

# Vehicle endpoints
@app.post("/vehicles/", response_model=VehicleResponse)
async def create_vehicle(vehicle: VehicleCreate):
    now = datetime.utcnow()
    query = vehicles.insert().values(
        name=vehicle.name,
        model=vehicle.model,
        battery_capacity=vehicle.battery_capacity,
        max_speed=vehicle.max_speed,
        weight=vehicle.weight,
        config=vehicle.config,
        created_at=now,
        updated_at=now
    )
    vehicle_id = await database.execute(query)
    
    return {**vehicle.dict(), "id": vehicle_id, "created_at": now, "updated_at": now}

@app.get("/vehicles/", response_model=List[VehicleResponse])
async def get_vehicles():
    query = vehicles.select()
    return await database.fetch_all(query)

@app.get("/vehicles/{vehicle_id}", response_model=VehicleResponse)
async def get_vehicle(vehicle_id: int):
    query = vehicles.select().where(vehicles.c.id == vehicle_id)
    result = await database.fetch_one(query)
    if not result:
        raise HTTPException(status_code=404, detail="Vehicle not found")
    return result

@app.put("/vehicles/{vehicle_id}", response_model=VehicleResponse)
async def update_vehicle(vehicle_id: int, vehicle: VehicleCreate):
    query = vehicles.select().where(vehicles.c.id == vehicle_id)
    existing = await database.fetch_one(query)
    if not existing:
        raise HTTPException(status_code=404, detail="Vehicle not found")
    
    update_data = {k: v for k, v in vehicle.dict().items()}
    update_data["updated_at"] = datetime.utcnow()
    
    query = vehicles.update().where(vehicles.c.id == vehicle_id).values(**update_data)
    await database.execute(query)
    
    return await database.fetch_one(vehicles.select().where(vehicles.c.id == vehicle_id))

# Lap endpoints
@app.post("/laps/", response_model=LapResponse)
async def create_lap(lap: LapCreate):
    query = laps.insert().values(
        race_session_id=lap.race_session_id,
        lap_number=lap.lap_number,
        start_time=lap.start_time,
        created_at=datetime.utcnow()
    )
    lap_id = await database.execute(query)
    
    return {**lap.dict(), "id": lap_id, "end_time": None, "duration": None, 
            "avg_speed": None, "energy_consumed": None, "created_at": datetime.utcnow()}

@app.put("/laps/{lap_id}", response_model=LapResponse)
async def update_lap(lap_id: int, lap_update: LapUpdate):
    query = laps.select().where(laps.c.id == lap_id)
    existing = await database.fetch_one(query)
    if not existing:
        raise HTTPException(status_code=404, detail="Lap not found")
    
    query = laps.update().where(laps.c.id == lap_id).values(
        end_time=lap_update.end_time,
        duration=lap_update.duration,
        avg_speed=lap_update.avg_speed,
        energy_consumed=lap_update.energy_consumed
    )
    await database.execute(query)
    
    return await database.fetch_one(laps.select().where(laps.c.id == lap_id))

@app.get("/laps/{session_id}", response_model=List[LapResponse])
async def get_session_laps(session_id: int):
    query = laps.select().where(laps.c.race_session_id == session_id).order_by(laps.c.lap_number)
    return await database.fetch_all(query)

# Telemetry Data
@app.post("/telemetry/")
async def record_telemetry(telemetry: TelemetryData):
    """Record telemetry data to InfluxDB"""
    client = get_influxdb_client()
    write_api = client.write_api(write_options=SYNCHRONOUS)
    
    point = Point("telemetry") \
        .tag("race_session_id", str(telemetry.race_session_id)) \
        .field("speed", telemetry.speed) \
        .field("battery_soc", telemetry.battery_soc) \
        .field("battery_temperature", telemetry.battery_temperature) \
        .field("battery_voltage", telemetry.battery_voltage) \
        .field("motor_temperature", telemetry.motor_temperature) \
        .field("energy_consumption", telemetry.energy_consumption) \
        .field("distance_traveled", telemetry.distance_traveled) \
        .time(telemetry.timestamp)
    
    write_api.write(bucket=INFLUXDB_BUCKET, org=INFLUXDB_ORG, record=point)
    return {"status": "recorded"}

@app.get("/dashboard/{session_id}", response_model=DashboardData)
async def get_dashboard_data(session_id: int):
    """Get all dashboard data for a racing session"""
    # Get session info from PostgreSQL
    session_query = race_sessions.select().where(race_sessions.c.id == session_id)
    session = await database.fetch_one(session_query)
    
    if not session:
        raise HTTPException(status_code=404, detail="Race session not found")
    
    # Get latest telemetry from InfluxDB
    client = get_influxdb_client()
    query_api = client.query_api()
    
    flux_query = f'''
    from(bucket: "{INFLUXDB_BUCKET}")
        |> range(start: -1h)
        |> filter(fn: (r) => r._measurement == "telemetry" and r.race_session_id == "{session_id}")
        |> last()
    '''
    
    result = query_api.query(org=INFLUXDB_ORG, query=flux_query)
    
    # Extract values from InfluxDB result
    telemetry_data = {}
    for table in result:
        for record in table.records:
            telemetry_data[record.get_field()] = record.get_value()
    
    # Get speed chart data (last 5 minutes)
    speed_query = f'''
    from(bucket: "{INFLUXDB_BUCKET}")
        |> range(start: -5m)
        |> filter(fn: (r) => r._measurement == "telemetry" and r.race_session_id == "{session_id}" and r._field == "speed")
    '''
    
    speed_result = query_api.query(org=INFLUXDB_ORG, query=speed_query)
    speed_chart_data = []
    for table in speed_result:
        for record in table.records:
            speed_chart_data.append({
                "time": record.get_time().strftime("%H:%M:%S"),
                "value": record.get_value()
            })
    
    # Get battery chart data
    battery_query = f'''
    from(bucket: "{INFLUXDB_BUCKET}")
        |> range(start: -30m)
        |> filter(fn: (r) => r._measurement == "telemetry" and r.race_session_id == "{session_id}" and r._field == "battery_soc")
    '''
    
    battery_result = query_api.query(org=INFLUXDB_ORG, query=battery_query)
    battery_chart_data = []
    for table in battery_result:
        for record in table.records:
            battery_chart_data.append({
                "time": record.get_time().strftime("%H:%M:%S"),
                "value": record.get_value()
            })
    
    # Get system events (warnings, errors)
    events_query = system_events.select().where(
        (system_events.c.race_session_id == session_id) & 
        (system_events.c.event_time >= datetime.utcnow() - timedelta(minutes=15))
    )
    events = await database.fetch_all(events_query)
    
    warnings = sum(1 for e in events if e.event_type == "warning")
    errors = sum(1 for e in events if e.event_type == "error")
    
    # Calculate progress
    total_distance = session["total_distance"]
    current_distance = telemetry_data.get("distance_traveled", 0)
    progress = (current_distance / total_distance) * 100 if total_distance > 0 else 0
    
    # Calculate remaining time based on average speed
    avg_speed_query = f'''
    from(bucket: "{INFLUXDB_BUCKET}")
        |> range(start: -15m)
        |> filter(fn: (r) => r._measurement == "telemetry" and r.race_session_id == "{session_id}" and r._field == "speed")
        |> mean()
    '''
    
    avg_speed_result = query_api.query(org=INFLUXDB_ORG, query=avg_speed_query)
    avg_speed = 0
    for table in avg_speed_result:
        for record in table.records:
            avg_speed = record.get_value()
    
    if avg_speed > 0:
        remaining_distance = total_distance - current_distance
        remaining_hours = remaining_distance / avg_speed
        remaining_minutes = int(remaining_hours * 60)
        remaining_seconds = int((remaining_hours * 60 * 60) % 60)
        remaining_time = f"{remaining_minutes}:{remaining_seconds:02d}"
    else:
        remaining_time = "N/A"
    
    # Calculate estimated range based on battery and consumption
    battery_soc = telemetry_data.get("battery_soc", 0)
    energy_consumption = telemetry_data.get("energy_consumption", 0)
    
    estimated_range = 0
    estimated_time = "N/A"
    
    if energy_consumption > 0:
        # This is simplified and would need a more complex model in a real application
        estimated_range = (battery_soc / 100) * 100  # Assuming 100km range at 100% battery
        estimated_hours = estimated_range / avg_speed if avg_speed > 0 else 0
        estimated_minutes = int(estimated_hours * 60)
        estimated_seconds = int((estimated_hours * 60 * 60) % 60)
        estimated_time = f"{estimated_minutes}:{estimated_seconds:02d}"
    
    # Compile full dashboard data
    dashboard_data = {
        "current_speed": telemetry_data.get("speed", 0),
        "avg_speed": avg_speed,
        "battery_soc": telemetry_data.get("battery_soc", 0),
        "battery_temp": {
            "current": telemetry_data.get("battery_temperature", 0),
            "min": telemetry_data.get("battery_temperature", 0) - 5,  # Example, would be from actual monitoring
            "max": telemetry_data.get("battery_temperature", 0) + 5,
        },
        "battery_voltage": {
            "current": telemetry_data.get("battery_voltage", 0),
            "min": telemetry_data.get("battery_voltage", 0) - 0.2,
            "max": telemetry_data.get("battery_voltage", 0) + 0.2,
        },
        "suggested_speed": avg_speed + 5,  # Example, would be from some strategy calculation
        "remaining_time": remaining_time,
        "distance_covered": current_distance,
        "progress_percent": progress,
        "estimated_range": estimated_range,
        "estimated_time": estimated_time,
        "energy_consumption": telemetry_data.get("energy_consumption", 0),
        "system_status": "OK" if errors == 0 else "FAIL",
        "warnings": warnings,
        "errors": errors,
        "sensors_status": "OK",  # Example, would be based on actual sensor diagnostics
        "communication_status": "OK",  # Example, would be based on actual communication checks
    }
    
    return dashboard_data

# System Events
@app.post("/events/", response_model=SystemEventResponse)
async def record_system_event(event: SystemEventCreate):
    query = system_events.insert().values(
        race_session_id=event.race_session_id,
        event_time=event.event_time,
        event_type=event.event_type,
        event_source=event.event_source,
        message=event.message,
        details=event.details,
        created_at=datetime.utcnow()
    )
    event_id = await database.execute(query)
    
    return {**event.dict(), "id": event_id, "created_at": datetime.utcnow()}

@app.get("/events/{session_id}", response_model=List[SystemEventResponse])
async def get_session_events(
    session_id: int,
    limit: int = 10,
    event_type: Optional[str] = None
):
    query = system_events.select().where(system_events.c.race_session_id == session_id)
    
    if event_type:
        query = query.where(system_events.c.event_type == event_type)
    
    query = query.order_by(system_events.c.event_time.desc()).limit(limit)
    return await database.fetch_all(query)

@app.get("/health")
async def health_check():
    """Health check endpoint for the API"""
    try:
        # Check database connection
        await database.fetch_one("SELECT 1")
        
        # Check InfluxDB connection
        client = get_influxdb_client()
        health = client.health()
        
        return {
            "status": "ok",
            "timestamp": datetime.utcnow().isoformat(),
            "database": "connected",
            "influxdb": health.status
        }
    except Exception as e:
        return {
            "status": "error",
            "timestamp": datetime.utcnow().isoformat(),
            "error": str(e)
        }

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=True)