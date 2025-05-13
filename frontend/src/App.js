import React, { useState, useEffect, useRef } from 'react';
import axios from 'axios';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import './style.css';

const API_URL = 'http://localhost:8000';

console.log("Using API URL:", API_URL);

function App() {
  const [sessionId, setSessionId] = useState(null);
  const [sessions, setSessions] = useState([]);
  const [dashboardData, setDashboardData] = useState(null);
  const [events, setEvents] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [speedHistory, setSpeedHistory] = useState([]);
  const [energyConsumptionHistory, setEnergyConsumptionHistory] = useState([]);
  // Flaga do śledzenia, czy komponent jest zamontowany
  const isMounted = useRef(true);
  // Referencja do ostatniego dashboardData, aby zachować dane nawet w przypadku błędu
  const lastDashboardData = useRef(null);
  
  // Funkcja do pobierania sesji
  useEffect(() => {
    const fetchSessions = async () => {
      try {
        console.log("Fetching sessions...");
        const response = await fetch(`${API_URL}/race-sessions/`);
        
        if (!response.ok) {
          throw new Error(`HTTP error! Status: ${response.status}`);
        }
        
        const data = await response.json();
        console.log("Received sessions:", data);
        setSessions(data);
        
        // Ustaw pierwszą dostępną sesję jako aktywną, jeśli nie wybrano żadnej
        if (data.length > 0 && !sessionId) {
          setSessionId(data[0].id);
        }
      } catch (error) {
        console.error('Error fetching sessions:', error);
        // Nie ustawiaj błędu jako stanu - tylko zaloguj
      } finally {
        setLoading(false);
      }
    };

    fetchSessions();
    // Nie ustawiaj interwału dla fetchSessions - to pobieramy tylko raz
  }, []);

  // Funkcja do pobierania danych dashboardu
  useEffect(() => {
    if (!sessionId) return;
    
    let isActive = true; // Flaga kontrolująca, czy komponent jest aktywny
    
    // Funkcja do pobierania danych
    const fetchDashboardData = async () => {
      if (!isActive) return; // Nie wykonuj, jeśli komponent został odmontowany
      
      try {
        console.log("Pobieranie danych dla sesji:", sessionId, "czas:", new Date().toISOString());
        
        // Pobierz dane dashboardu
        const dashboardResponse = await axios.get(`${API_URL}/dashboard/${sessionId}`);
        
        if (isActive) {
          setDashboardData(dashboardResponse.data);
          lastDashboardData.current = dashboardResponse.data;
          if (error) setError(null);
        }
        
        // Pozyskaj dane historii prędkości
        try {
          const speedHistoryResponse = await axios.get(`${API_URL}/telemetry/history/${sessionId}?minutes=10&metric=speed`);
          if (isActive && speedHistoryResponse.data && speedHistoryResponse.data.length > 0) {
            setSpeedHistory(speedHistoryResponse.data.map(item => ({
              time: new Date(item.timestamp).toLocaleTimeString(),
              value: item.speed
            })));
          }
        } catch (historyError) {
          console.warn('Nie udało się pobrać historii prędkości:', historyError);
        }
        
        // Pozyskaj dane historii zużycia energii
        try {
          const energyHistoryResponse = await axios.get(`${API_URL}/telemetry/history/${sessionId}?minutes=10&metric=energy_consumption`);
          if (isActive && energyHistoryResponse.data && energyHistoryResponse.data.length > 0) {
            setEnergyConsumptionHistory(energyHistoryResponse.data.map(item => ({
              time: new Date(item.timestamp).toLocaleTimeString(),
              value: item.energy_consumption
            })));
          }
        } catch (energyHistoryError) {
          console.warn('Nie udało się pobrać historii zużycia energii:', energyHistoryError);
        }
        
        // Pobierz wydarzenia
        try {
          const eventsResponse = await axios.get(`${API_URL}/events/${sessionId}?limit=5`);
          if (isActive) {
            setEvents(eventsResponse.data);
          }
        } catch (eventError) {
          console.warn('Nie udało się pobrać wydarzeń:', eventError);
        }
        
      } catch (error) {
        console.error('Błąd pobierania danych dashboardu:', error);
        
        // Zachowaj poprzednie dane, jeśli istnieją
        if (isActive && lastDashboardData.current) {
          // Nie aktualizuj stanu błędu za każdym razem - tylko w konsoli
          console.warn("Używam poprzednich danych z powodu błędu połączenia");
        }
      }
    };
    
    // Natychmiast pobierz dane
    fetchDashboardData();
    
    // Ustaw częstsze odświeżanie - co 3 sekundy zamiast 8
    const interval = setInterval(fetchDashboardData, 1000);
    
    // Funkcja czyszcząca
    return () => {
      isActive = false; // Oznacz komponent jako nieaktywny
      clearInterval(interval); // Zatrzymaj interwał
    };
  }, [sessionId, API_URL, error]);

  // Ustaw flagę czyszczącą przy odmontowaniu komponentu
  useEffect(() => {
    return () => {
      isMounted.current = false;
    };
  }, []);

  // Obsługa zmiany sesji
  const handleSessionChange = (e) => {
    const newSessionId = e.target.value ? parseInt(e.target.value) : null;
    setSessionId(newSessionId);
  };

  // Ręczne odświeżanie danych
  const refreshData = () => {
    if (sessionId) {
      setLoading(true);
      
      Promise.all([
        axios.get(`${API_URL}/dashboard/${sessionId}`),
        axios.get(`${API_URL}/events/${sessionId}?limit=5`),
        axios.get(`${API_URL}/telemetry/history/${sessionId}?minutes=10&metric=speed`),
        axios.get(`${API_URL}/telemetry/history/${sessionId}?minutes=10&metric=energy_consumption`) // Dodaj to zapytanie
      ])
      .then(([dashboardResponse, eventsResponse, speedHistoryResponse, energyHistoryResponse]) => {
        setDashboardData(dashboardResponse.data);
        setEvents(eventsResponse.data);
        
        if (speedHistoryResponse.data && speedHistoryResponse.data.length > 0) {
          setSpeedHistory(speedHistoryResponse.data.map(item => ({
            time: new Date(item.timestamp).toLocaleTimeString(),
            value: item.speed
          })));
        }
        
        // Dodaj przetwarzanie danych zużycia energii
        if (energyHistoryResponse.data && energyHistoryResponse.data.length > 0) {
          setEnergyConsumptionHistory(energyHistoryResponse.data.map(item => ({
            time: new Date(item.timestamp).toLocaleTimeString(),
            value: item.energy_consumption
          })));
        }
        
        setError(null);
      })
      .catch(error => {
        console.error('Error during manual refresh:', error);
        setError(`Nie można odświeżyć danych: ${error.message}`);
      })
      .finally(() => {
        setLoading(false);
      });
    }
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
        <div className="controls">
          <select 
            value={sessionId || ''} 
            onChange={handleSessionChange}
            className="session-select"
          >
            <option value="">Wybierz sesję wyścigową</option>
            {sessions.map(session => (
              <option key={session.id} value={session.id}>
                {session.name} ({session.status})
              </option>
            ))}
          </select>
          <button onClick={refreshData} className="refresh-btn">
            🔄 Odśwież dane
          </button>
        </div>
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
                  <LineChart data={speedHistory.length > 0 ? speedHistory : 
                    // Dane zastępcze jeśli nie ma historii
                    [...Array(10)].map((_, i) => ({
                      time: i.toString(),
                      value: Math.random() * 20 + (dashboardData?.avg_speed || 0)
                    }))
                  }>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis domain={['auto', 'auto']} hide />
                    <Tooltip />
                    <Line type="monotone" dataKey="value" stroke="#10B981" strokeWidth={2} dot={false} />
                  </LineChart>
                </ResponsiveContainer>
                <div className="chart-label">Prędkość (ostatnie 10 min)</div>
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
                <div className="data-value">{dashboardData.battery_voltage.current.toFixed(2)}V</div>
                <div className="data-subtext">
                  min: {dashboardData.battery_voltage.min.toFixed(2)}V | max: {dashboardData.battery_voltage.max.toFixed(2)}V
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
                  <LineChart data={energyConsumptionHistory.length > 0 ? energyConsumptionHistory : 
                    // Dane zastępcze jeśli nie ma historii
                    [...Array(10)].map((_, i) => ({
                      time: i.toString(),
                      value: Math.random() * 30 + dashboardData.energy_consumption - 15
                    }))
                  }>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis domain={['auto', 'auto']} hide />
                    <Tooltip />
                    <Line type="monotone" dataKey="value" stroke="#F59E0B" strokeWidth={2} dot={false} />
                  </LineChart>
                </ResponsiveContainer>
                <div className="chart-label">Zużycie energii (ostatnie 10 min)</div>
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