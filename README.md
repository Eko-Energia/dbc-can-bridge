# Perla Monitor - EV Racing Dashboard

A real-time monitoring dashboard for electric vehicle racing teams.

## Features

- Real-time telemetry data visualization
- Battery and energy management
- Race strategy insights
- System diagnostics and event monitoring

## Technology Stack

- **Backend**: FastAPI, PostgreSQL, InfluxDB
- **Frontend**: React, Recharts
- **Infrastructure**: Docker, Docker Compose

## Setup and Installation

### Prerequisites

- Docker and Docker Compose
- Node.js (for local frontend development)
- Python 3.10+ (for local backend development)

### Running with Docker

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/Perla-Monitor.git
   cd Perla-Monitor
   ```

2. Start the services:
   ```
   docker-compose up -d
   ```

3. Access the application:
   - Frontend: http://localhost:3000
   - Backend API: http://localhost:8000/docs

### Local Development

#### Backend

1. Navigate to the backend directory:
   ```
   cd backend
   ```

2. Install dependencies:
   ```
   pip install -r requirements.txt
   ```

3. Run the development server:
   ```
   uvicorn main:app --reload
   ```

#### Frontend

1. Navigate to the frontend directory:
   ```
   cd frontend
   ```

2. Install dependencies:
   ```
   npm install
   ```

3. Run the development server:
   ```
   npm start
   ```

## Running the Simulator

The project includes a simulator to generate test data for development and demonstration:

```
cd backend
python symulator.py
```

## Database Structure

- PostgreSQL: Stores configuration, race sessions, laps, and system events
- InfluxDB: Stores time-series telemetry data

## API Documentation

Once the backend is running, you can access the auto-generated API documentation at:
- Swagger UI: http://localhost:8000/docs
- ReDoc: http://localhost:8000/redoc
