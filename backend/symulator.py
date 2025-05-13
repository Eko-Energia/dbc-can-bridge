import asyncio
import random
import time
from datetime import datetime, timedelta
import httpx
import math
import json
import os

# Configuration
API_URL = "http://backend:8000" if os.environ.get("IN_DOCKER") else "http://localhost:8000"
SESSION_ID = 1  # Replace with actual session ID
TOTAL_DISTANCE = 100  # km
RACE_DURATION = 3600  # seconds
UPDATE_INTERVAL = 1.0  # seconds

class EVSimulator:
    def __init__(self, session_id, total_distance):
        self.session_id = session_id
        self.total_distance = total_distance
        self.current_distance = 0
        self.current_speed = 0
        self.battery_soc = 100.0
        self.battery_temp = 25.0
        self.battery_voltage = 4.0
        self.motor_temp = 30.0
        self.energy_consumption = 200.0  # Wh/km
        self.start_time = datetime.utcnow()
        self.last_event_time = datetime.utcnow() - timedelta(minutes=10)
    
    def update(self):
        # Update speed (realistic variations)
        target_speed = 80 + 20 * math.sin(time.time() / 60)
        speed_change = (target_speed - self.current_speed) * 0.1
        self.current_speed = max(0, min(160, self.current_speed + speed_change))
        
        # Update distance
        elapsed = UPDATE_INTERVAL / 3600  # convert to hours
        distance_delta = self.current_speed * elapsed
        self.current_distance += distance_delta
        
        # Update battery
        energy_used = distance_delta * self.energy_consumption / 1000  # in kWh
        self.battery_soc = max(0, self.battery_soc - (energy_used / 50) * 100)  # assuming 50kWh battery
        
        # Update temperatures (gradual changes with external factors)
        ambient_factor = math.sin(time.time() / 900) * 2  # Simulating ambient temperature changes
        speed_factor = self.current_speed / 160 * 5  # Higher speeds = more heat
        
        self.battery_temp = min(60, max(20, self.battery_temp + 
                                       ambient_factor * 0.1 + 
                                       speed_factor * 0.1 - 
                                       (self.battery_temp - 25) * 0.02))  # Cooling effect
        
        self.motor_temp = min(100, max(30, self.motor_temp + 
                                      ambient_factor * 0.1 + 
                                      speed_factor * 0.2 - 
                                      (self.motor_temp - 30) * 0.01))  # Cooling effect
        
        # Update voltage (correlates with SOC)
        self.battery_voltage = 3.6 + (self.battery_soc / 100) * 0.6 + random.uniform(-0.05, 0.05)
        
        # Update energy consumption (varies with speed and conditions)
        base_consumption = 180  # Wh/km at optimal speed
        speed_efficiency_factor = 1.0 + 0.005 * (self.current_speed - 60) ** 2 / 100
        temperature_factor = 1.0 + abs(self.battery_temp - 25) * 0.01
        
        self.energy_consumption = base_consumption * speed_efficiency_factor * temperature_factor
        
        return {
            "race_session_id": self.session_id,
            "timestamp": datetime.utcnow().isoformat(),
            "speed": round(self.current_speed, 1),
            "battery_soc": round(self.battery_soc, 1),
            "battery_temperature": round(self.battery_temp, 1),
            "battery_voltage": round(self.battery_voltage, 2),
            "motor_temperature": round(self.motor_temp, 1),
            "energy_consumption": round(self.energy_consumption, 1),
            "distance_traveled": round(self.current_distance, 2)
        }
    
    def generate_system_event(self):
        # Random events with different probabilities
        now = datetime.utcnow()
        if (now - self.last_event_time).seconds < 60:
            return None
            
        event_types = [
            {"type": "info", "probability": 0.7},
            {"type": "warning", "probability": 0.25},
            {"type": "error", "probability": 0.05}
        ]
        
        sources = ["battery", "motor", "sensors", "communication", "cooling"]
        
        messages = {
            "info": [
                "Regular system check completed",
                "Sensor calibration successful",
                "Energy management system optimized",
                "Driver profile updated",
                "Regenerative braking active"
            ],
            "warning": [
                "Battery temperature increasing",
                "Energy consumption above optimal range",
                "Communication latency detected",
                "Sensor data fluctuation",
                "Motor temperature increasing"
            ],
            "error": [
                "Sensor communication lost",
                "Battery cell voltage imbalance",
                "Motor controller error",
                "Cooling system malfunction",
                "Power management system error"
            ]
        }
        
        # Determine if we generate an event
        if random.random() > 0.2:  # 20% chance of event
            return None
            
        # Choose event type weighted by probability
        r = random.random()
        cumulative = 0
        selected_type = "info"
        for event in event_types:
            cumulative += event["probability"]
            if r <= cumulative:
                selected_type = event["type"]
                break
                
        source = random.choice(sources)
        message = random.choice(messages[selected_type])
        
        # Additional details based on event type and source
        details = {}
        if source == "battery":
            details = {
                "voltage": round(self.battery_voltage, 2),
                "temperature": round(self.battery_temp, 1),
                "soc": round(self.battery_soc, 1)
            }
        elif source == "motor":
            details = {
                "temperature": round(self.motor_temp, 1),
                "rpm": int(self.current_speed * 100),
                "load": random.randint(50, 100)
            }
            
        self.last_event_time = now
        
        return {
            "race_session_id": self.session_id,
            "event_time": now.isoformat(),
            "event_type": selected_type,
            "event_source": source,
            "message": message,
            "details": details
        }

async def send_telemetry(client, data):
    try:
        response = await client.post(f"{API_URL}/telemetry/", json=data)
        if response.status_code == 200:
            print(f"Telemetry sent: Speed={data['speed']} km/h, Battery={data['battery_soc']}%")
        else:
            print(f"Failed to send telemetry: {response.status_code} {response.text}")
    except Exception as e:
        print(f"Error sending telemetry: {e}")

async def send_event(client, event_data):
    if not event_data:
        return
        
    try:
        response = await client.post(f"{API_URL}/events/", json=event_data)
        if response.status_code == 200:
            print(f"Event sent: {event_data['event_type']} - {event_data['message']}")
        else:
            print(f"Failed to send event: {response.status_code} {response.text}")
    except Exception as e:
        print(f"Error sending event: {e}")

async def main():
    # Create or get a race session
    async with httpx.AsyncClient() as client:
        try:
            # Check if we need to create a new session
            response = await client.get(f"{API_URL}/race-sessions/")
            sessions = response.json()
            
            if not sessions or all(s["status"] != "active" for s in sessions):
                # Create a new session
                new_session = {
                    "name": f"Test Race {datetime.utcnow().strftime('%Y-%m-%d %H:%M')}",
                    "vehicle_id": 1,  # Assuming a vehicle with ID 1 exists
                    "total_distance": TOTAL_DISTANCE,
                    "config": {}
                }
                response = await client.post(f"{API_URL}/race-sessions/", json=new_session)
                session = response.json()
                session_id = session["id"]
                print(f"Created new race session: {session_id}")
            else:
                # Use the first active session
                active_sessions = [s for s in sessions if s["status"] == "active"]
                session_id = active_sessions[0]["id"]
                print(f"Using existing race session: {session_id}")
                
            # Create the simulator
            simulator = EVSimulator(session_id, TOTAL_DISTANCE)
            
            # Simulation loop
            start_time = time.time()
            while time.time() - start_time < RACE_DURATION and simulator.battery_soc > 0:
                telemetry_data = simulator.update()
                await send_telemetry(client, telemetry_data)
                
                event_data = simulator.generate_system_event()
                if event_data:
                    await send_event(client, event_data)
                    
                # Sleep until next update
                await asyncio.sleep(UPDATE_INTERVAL)
                
                # Check if race is completed
                if simulator.current_distance >= TOTAL_DISTANCE:
                    print("Race completed!")
                    # Update race session status
                    update_data = {
                        "status": "completed",
                        "end_time": datetime.utcnow().isoformat()
                    }
                    await client.put(f"{API_URL}/race-sessions/{session_id}", json=update_data)
                    break
                    
            print("Simulation ended")
            
        except Exception as e:
            print(f"Error in simulation: {e}")

if __name__ == "__main__":
    asyncio.run(main())