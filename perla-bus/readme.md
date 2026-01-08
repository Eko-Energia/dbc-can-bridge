# Perla Bus Simulator

Symulator szyny CAN wysyłający losowe ramki zgodne z definicjami z pliku DBC.

**Dostępne wersje:**
- `symulator.py` - Symulator dla socketcan (vcan0, can0)
- `symulator_serial.py` - Symulator urządzenia szeregowego (emuluje /dev/ttyUSB0)

## Przygotowanie

### 1. Instalacja zależności

```bash
# Utwórz środowisko wirtualne Python
python3 -m venv venv

# Aktywuj środowisko
source venv/bin/activate

# Zainstaluj zależności
pip install -r requirements.txt
```

**Uwaga:** Pamiętaj aby zawsze aktywować środowisko przed uruchomieniem symulatorów:
```bash
source venv/bin/activate
```

### 2. Konfiguracja wirtualnego interfejsu CAN (Linux)

```bash
# Załaduj moduł vcan
sudo modprobe vcan

# Utwórz wirtualny interfejs
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0

# Sprawdź status
ip link show vcan0
```

## Użycie

### Symulator Socketcan (symulator.py)

#### Uruchomienie ciągłej symulacji

```bash
python symulator.py
```

### Opcje

```bash
# Użyj własnego pliku DBC
python symulator.py --dbc moj_plik.dbc

# Użyj innego kanału CAN
python symulator.py --channel can0

# Symuluj przez określony czas (30 sekund)
python symulator.py --duration 30

# Wyślij każdą wiadomość raz i zakończ
python symulator.py --single-cycle

# Pomoc
python symulator.py --help
``# Symulator Szeregowy (symulator_serial.py)

Ten symulator emuluje urządzenie **Waveshare USB-CAN-A** na wirtualnym porcie szeregowym (PTY). Idealny do testowania `can-receiver` bez fizycznego sprzętu.

#### Plik konfiguracyjny

Symulator może używać pliku konfiguracyjnego `simulator_config.py`:

```python
# Simulation settings
DBC_FILE = "perla_bus.dbc"
MODE = "continuous"  # "continuous", "single", or "duration"
DURATION = 30

# Timing
CYCLE_MULTIPLIER = 1.0  # 0.5 = 2x faster, 2.0 = 2x slower
CYCLE_VARIATION = 10  # Random variation 0-100%

# Value generation
REALISTIC_MODE = False  # Gradual changes instead of pure random
SMOOTHING_FACTOR = 0.8

# Logging
LOG_LEVEL = "info"  # "debug", "info", "warning", "error"
LOG_FRAMES = True
LOG_SIGNALS = True
```

#### Szybkie uruchomienie

```bash
# Użyj skryptu pomocniczego (automatycznie aktywuje venv)
./run_simulator.sh

# Lub z konfiguracją
./run_simulator.sh --config simulator_config.py
```

#### Podstawowe użycie

```bash
# Aktywuj środowisko wirtualne
source venv/bin/activate

# Utworzy wirtualny port szeregowy (np. /dev/pts/4)
python symulator_serial.py

# Użyj pliku konfiguracyjnego
python symulator_serial.py --config simulator_config.py
```

Po uruchomieniu symulator wyświetli ścieżkę do wirtualnego portu, np.:
```
✓ Virtual serial port created: /dev/pts/4
  Use this port in can-receiver config: device_port=/dev/pts/4
```

#### Konfiguracja can-receiver

Skopiuj ścieżkę wirtualnego portu i edytuj `can-receiver/config.txt`:
```
device_port=/dev/pts/4
can_baud_rate=500k
```

Następnie uruchom can-receiver:
```bash
cd ../can-receiver
cargo run
```

#### Opcje symulatora szeregowego

```bash
# Użyj własnego pliku DBC
./run_simulator.sh --dbc moj_plik.dbc
# lub
python symulator_serial.py --dbc moj_plik.dbc

# Symuluj przez określony czas (30 sekund)
python symulator_serial.py --duration 30

# Wyślij każdą wiadomość raz i zakończ
python symulator_serial.py --single-cycle

# Użyj konkretnego portu szeregowego zamiast PTY
python symulator_serial.py --port /dev/ttyUSB1

# Przyspiesz symulację 2x (cykle 2x krótsze)
python symulator_serial.py --cycle-multiplier 0.5

# Zwolnij symulację 2x (cykle 2x dłuższe)
python symulator_serial.py --cycle-multiplier 2.0

# Tryb realistyczny - stopniowe zmiany wartości
python symulator_serial.py --realistic

# Użyj konfiguracji z pliku
python symulator_serial.py --config simulator_config.py

# Pomoc
python symulator_serial.py --help
```

#### Format protokołu Waveshare

Symulator implementuje protokół Waveshare USB-CAN-A:
```
[0xAA] [ID3] [ID2] [ID1] [ID0] [DLC] [D0] ... [D7] [Checksum]
```
- Standard ID (11-bit): przechowywany w niższych 11 bitach
- Extended ID (29-bit): używa wszystkich 4 bajtów
- Checksum: suma wszystkich bajtów & 0xFF

##`

## Plik DBC

Plik `perla_bus.dbc` zawiera definicje następujących wiadomości:

- **MotorStatus (0x80)**: RPM i temperatura silnika
- **BatteryInfo (0x82)**: Napięcie, prąd i stan naładowania baterii
- **SensorData (0x8B)**: Prędkość, temperatura, ciśnienie, wilgotność
- **ControlCommands (0x100)**: Komendy sterujące
- **DiagnosticInfo (0x200)**: Informacje diagnostyczne i błędy

Każda wiadomość ma zdefiniowane:
- Sygnały z ich pozycją bitową, długością i kolejnością bajtów
- Zakresy wartości (minimum/maximum)
- Jednostki i współczynniki skalowania
- Cykle czasowe wysyłania

## Testowanie

### Odbieranie ramek

W innym terminalu możesz odbierać ramki używając:

```bash
# Wyświetl wszystkie ramki
candump vcan0

# Wyświetl z timestampem
candump -ta vcan0

# Filtruj konkretne ID
candump vcan0,080:7FF
```

### Integracja z can-receiver

Symulator może być użyty do testowania aplikacji `can-receiver`:

```bash
# Terminal 1: Uruchom symulator
python symulator.py

# Terminal 2: Uruchom can-receiver (po skompilowaniu)
cd ../can-receiver
cargo run
```

## Struktura wiadomości

### MotorStatus (ID: 128 / 0x80)
- Długość: 3 bajty
- Cykl: 100ms
- Sygnały:
  - `MotorRPM`: 16-bit, 0-8000 rpm
  - `MotorTemp`: 8-bit, -40 do 215°C

### BatteryInfo (ID: 130 / 0x82)
- Długość: 6 bajtów
- Cykl: 200ms
- Sygnały:
  - `Voltage`: 16-bit, 0-655.35V (skala 0.01)
  - `Current`: 16-bit signed, -3276.8 do 3276.7A (skala 0.1)
  - `StateOfCharge`: 8-bit, 0-100% (skala 0.5)

### SensorData (ID: 139 / 0x8B)
- Długość: 7 bajtów
- Cykl: 50ms
- Sygnały:
  - `Speed`: 16-bit, 0-300 km/h (skala 0.01)
  - `AmbientTemp`: 8-bit, -40 do 100°C
  - `Pressure`: 16-bit, 0-6553.5 kPa (skala 0.1)
  - `Humidity`: 8-bit, 0-100% (skala 0.5)

## Rozwiązywanie problemów

### Błąd: "Network is down"
```bash
# Upewnij się, że interfejs jest aktywny
sudo ip link set up vcan0
```

### Błąd: "No such device"
```bash
# Załaduj moduł vcan
sudo modprobe vcan
```

### Testowanie bez uprawnień root
Wirtualne interfejsy CAN (vcan) mogą wymagać sudo do utworzenia, ale po utworzeniu można ich używać bez uprawnień root.
