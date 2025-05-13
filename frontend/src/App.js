import React, { useState, useEffect } from 'react';
import axios from 'axios';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import './style.css';

const API_URL = process.env.NODE_ENV === 'production' 
  ? process.env.REACT_APP_API_URL || '/api' // In production, use relative path
  : 'http://localhost:8000';  // In development, use localhost

console.log("Using API URL:", API_URL);

function App() {
  const [sessionId, setSessionId] = useState(null);
  const [sessions, setSessions] = useState([]);
  const [dashboardData, setDashboardData] = useState(null);
  const [events, setEvents] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  // Fetch available race sessions
  useEffect(() => {
    const fetchSessions = async () => {
      try {
        setLoading(true);
        console.log("Fetching from:", `${API_URL}/race-sessions/`);
        const response = await fetch(`${API_URL}/race-sessions/`);
        
        if (!response.ok) {
          throw new Error(`HTTP error! Status: ${response.status}`);
        }
        
        const data = await response.json();
        console.log("Received sessions:", data);
        setSessions(data);
        
        // Set first session as active if available
        if (data.length > 0 && !sessionId) {
          setSessionId(data[0].id);
        }
      } catch (error) {
        console.error('Error fetching sessions:', error);
        setError('Failed to load race sessions. Check API connection.');
      } finally {
        setLoading(false);
      }
    };

    fetchSessions();
  }, []);

  // Fetch dashboard data and events
  useEffect(() => {
    if (!sessionId) return;
    
    const fetchDashboardData = async () => {
      try {
        const [dashboardResponse, eventsResponse] = await Promise.all([
          axios.get(`${API_URL}/dashboard/${sessionId}`),
          axios.get(`${API_URL}/events/${sessionId}?limit=5`)
        ]);
        
        setDashboardData(dashboardResponse.data);
        setEvents(eventsResponse.data);
      } catch (err) {
        console.error('Failed to fetch dashboard data:', err);
      }
    };
    
    fetchDashboardData();
    
    // Poll for updates
    const interval = setInterval(fetchDashboardData, 1000);
    return () => clearInterval(interval);
  }, [sessionId]);

  const handleSessionChange = (e) => {
    setSessionId(Number(e.target.value));
  };

  if (loading) {
    return <div className="loading">Loading...</div>;
  }

  if (error) {
    return <div className="error">{error}</div>;
  }

  return (
    <div className="app">
      <header className="header">
        <h1>EV Racing Dashboard</h1>
        <select value={sessionId || ''} onChange={handleSessionChange}>
          <option value="">Select Race Session</option>
          {sessions.map(session => (
            <option key={session.id} value={session.id}>
              {session.name} ({session.status})
            </option>
          ))}
        </select>
        <button className="settings-btn">⚙️</button>
      </header>

      {dashboardData ? (
        <main className="dashboard">
          {/* Speed Panel */}
          <section className="panel speed-panel">
            <h2>PRĘDKOŚĆ</h2>
            <div className="speed-gauge">
              <div className="gauge">
                <div className="gauge-value">{Math.round(dashboardData.current_speed)}</div>
                <div className="gauge-unit">km/h</div>
              </div>
            </div>
            <div className="speed-info">
              <div className="data-group">
                <div className="data-label">Średnia z okrążeń</div>
                <div className="data-value">{Math.round(dashboardData.avg_speed)} km/h</div>
              </div>
              <div className="speed-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={[...Array(10)].map((_, i) => ({
                    time: i,
                    value: Math.random() * 20 + dashboardData.avg_speed - 10
                  }))}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis domain={['auto', 'auto']} hide />
                    <Tooltip />
                    <Line type="monotone" dataKey="value" stroke="#10B981" strokeWidth={2} dot={false} />
                  </LineChart>
                </ResponsiveContainer>
                <div className="chart-label">Czas okrążenia</div>
              </div>
            </div>
          </section>

          {/* Battery Panel */}
          <section className="panel battery-panel">
            <h2>BATERIA</h2>
            <div className="battery-gauge">
              <div className="battery-progress">
                <div 
                  className="battery-level" 
                  style={{width: `${dashboardData.battery_soc}%`}}
                ></div>
                <span className="battery-percentage">{Math.round(dashboardData.battery_soc)}%</span>
              </div>
            </div>
            <div className="battery-info">
              <div className="data-group">
                <div className="data-label">Temperatura</div>
                <div className="data-value">{dashboardData.battery_temp.current}°C</div>
                <div className="data-subtext">
                  min: {dashboardData.battery_temp.min}°C | max: {dashboardData.battery_temp.max}°C
                </div>
              </div>
              <div className="data-group">
                <div className="data-label">Napięcie ogniw</div>
                <div className="data-value">{dashboardData.battery_voltage.current}V</div>
                <div className="data-subtext">
                  min: {dashboardData.battery_voltage.min}V | max: {dashboardData.battery_voltage.max}V
                </div>
              </div>
            </div>
          </section>

          {/* Strategy Panel */}
          <section className="panel strategy-panel">
            <h2>WYŚCIG STRATEGIA</h2>
            <div className="strategy-grid">
              <div className="data-group">
                <div className="data-label">Sugerowana prędkość</div>
                <div className="data-value">{Math.round(dashboardData.suggested_speed)} km/h</div>
              </div>
              <div className="data-group">
                <div className="data-label">Pozostały czas</div>
                <div className="data-value">{dashboardData.remaining_time}</div>
              </div>
              <div className="data-group">
                <div className="data-label">Przebyta droga</div>
                <div className="data-value">{dashboardData.distance_covered.toFixed(1)} km</div>
              </div>
              <div className="progress-container">
                <div className="progress-bar">
                  <div 
                    className="progress-fill" 
                    style={{width: `${dashboardData.progress_percent}%`}}
                  ></div>
                </div>
                <div className="progress-label">{Math.round(dashboardData.progress_percent)}% ukończono</div>
              </div>
            </div>
          </section>

          {/* Range Panel */}
          <section className="panel range-panel">
            <h2>ZASIĘG</h2>
            <div className="range-grid">
              <div className="data-group">
                <div className="data-label">Szacowany zasięg</div>
                <div className="data-value">{Math.round(dashboardData.estimated_range)} km</div>
              </div>
              <div className="data-group">
                <div className="data-label">Szacowany czas jazdy</div>
                <div className="data-value">{dashboardData.estimated_time}</div>
              </div>
              <div className="data-group">
                <div className="data-label">Aktualne zużycie energii</div>
                <div className="data-value">{Math.round(dashboardData.energy_consumption)} Wh/km</div>
              </div>
              <div className="energy-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={[...Array(10)].map((_, i) => ({
                    time: i,
                    value: Math.random() * 30 + dashboardData.energy_consumption - 15
                  }))}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis domain={['auto', 'auto']} hide />
                    <Tooltip />
                    <Line type="monotone" dataKey="value" stroke="#F59E0B" strokeWidth={2} dot={false} />
                  </LineChart>
                </ResponsiveContainer>
                <div className="chart-label">Czas</div>
              </div>
            </div>
          </section>
        </main>
      ) : (
        <div className="no-data">Select a race session to view dashboard</div>
      )}

      {/* Diagnostics Status Bar */}
      {dashboardData && (
        <footer className="status-bar">
          <div className="status-title">STATUS SYSTEMU</div>
          <div className={`system-status ${dashboardData.system_status === 'OK' ? 'status-ok' : 'status-error'}`}>
            {dashboardData.system_status}
          </div>
          <div className="status-indicators">
            <div className={`indicator ${dashboardData.sensors_status === 'OK' ? 'status-ok' : 'status-error'}`}>
              <span className="indicator-dot"></span>
              <span className="indicator-label">Czujniki</span>
            </div>
            <div className={`indicator ${dashboardData.communication_status === 'OK' ? 'status-ok' : 'status-error'}`}>
              <span className="indicator-dot"></span>
              <span className="indicator-label">Komunikacja</span>
            </div>
            <div className={`indicator ${dashboardData.warnings > 0 ? 'status-warning' : 'status-ok'}`}>
              <span className="indicator-dot"></span>
              <span className="indicator-label">Ostrzeżenia ({dashboardData.warnings})</span>
            </div>
            <div className={`indicator ${dashboardData.errors > 0 ? 'status-error' : 'status-ok'}`}>
              <span className="indicator-dot"></span>
              <span className="indicator-label">Błędy ({dashboardData.errors})</span>
            </div>
          </div>
        </footer>
      )}
    </div>
  );
}

export default App;