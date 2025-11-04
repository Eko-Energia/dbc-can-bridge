import random
from datetime import datetime, timezone, timedelta
from pymongo import MongoClient
from pydantic import BaseModel, ValidationError
from typing import List, Dict
import os

class DataPoint(BaseModel):
    val: float
    unit: str
    time: int

# Connect to MongoDB using environment variable
mongo_host = os.getenv('MONGO_HOST', 'localhost')
mongo_port = int(os.getenv('MONGO_PORT', '27017'))

# Always use authentication credentials
client = MongoClient(
    mongo_host, 
    mongo_port, 
    username='admin', 
    password='password',
    authSource='admin'  # Specify the auth database
)
db = client['perla_monitor']
collection = db['jsons']

def generate_fake_data(timestamp):
    json_data = {}
    
    # Battery temperature
    json_data["battery_temperature"] = [{"val": round(random.uniform(20.0, 40.0), 2), "unit": "C", "time": timestamp}]
    
    # Voltage for each of 8 cells
    for i in range(1, 9):
        json_data[f"cell{i}_voltage"] = [{"val": round(random.uniform(3.0, 4.2), 2), "unit": "V", "time": timestamp}]
    
    # Battery current
    json_data["battery_current"] = [{"val": round(random.uniform(0.0, 10.0), 2), "unit": "A", "time": timestamp}]
    
    return json_data

def fill_fake_data_for_tests(num_entries=100, interval_seconds=60):
    """Function to artificially fill the database with fake data for testing."""
    current_time = datetime.now(timezone.utc)
    for i in range(num_entries):
        timestamp = int((current_time - timedelta(seconds=i * interval_seconds)).timestamp())
        json_data = generate_fake_data(timestamp)
        frames: Dict[str, List[DataPoint]] = {}
        for frame_name, points in json_data.items():
            frames[frame_name] = [DataPoint(**point) for point in points]
        
        # Insert each data point with frame name
        for frame_name, points in frames.items():
            for point in points:
                document = {
                    "frame": frame_name,
                    "val": point.val,
                    "unit": point.unit,
                    "time": point.time,
                    "receivedAt": datetime.now(timezone.utc)
                }
                collection.insert_one(document)
        print(f"Fake data batch {i+1} inserted")

if __name__ == "__main__":
    fill_fake_data_for_tests()