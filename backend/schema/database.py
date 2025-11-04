import json
import time
import requests
from datetime import datetime, timezone
from pymongo import MongoClient
from pydantic import BaseModel, ValidationError
from typing import List, Dict
import os

class DataPoint(BaseModel):
    val: float
    unit: str
    time: int

FETCH_INTERVAL = 1
FETCH_URL = 'http://localhost:2137/get'

# Connect to MongoDB using environment variable
mongo_host = os.getenv('MONGO_HOST', 'localhost')
client = MongoClient(mongo_host, 27017, username='admin', password='password')
db = client['perla_monitor']
collection = db['jsons']

def fetch_and_insert():
    try:
        response = requests.get(FETCH_URL)
        if response.status_code == 200:
            json_data = response.json()
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
                    print(f"Data inserted for {frame_name} at {datetime.now(timezone.utc)}")
        else:
            print(f"Failed to fetch data: {response.status_code}")
    except ValidationError as e:
        print(f"Validation error: {e}")
    except Exception as e:
        print(f"Error: {e}")

# Run periodically
while True:
    fetch_and_insert()
    time.sleep(FETCH_INTERVAL)