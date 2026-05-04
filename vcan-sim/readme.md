# Perla Bus Simulator

Symulator szyny CAN wysyłający losowe ramki zgodne z definicjami z pliku DBC.

Działa tylko na Linuksie!

**Dostępne wersje:**
- `simulator_serial.py` - Symulator urządzenia szeregowego (emuluje /dev/ttyUSB0)

## Przygotowanie

### 1. Instalacja zależności

```bash
# Utwórz środowisko wirtualne Python
python3 -m venv .venv

# Aktywuj środowisko
source .venv/bin/activate

# Zainstaluj zależności
pip install -r requirements.txt
```

**Uwaga:** Pamiętaj aby zawsze aktywować środowisko przed uruchomieniem symulatorów:
```bash
source .venv/bin/activate
```

## Użycie

#### Podstawowe użycie

```bash

# Utworzy wirtualny port szeregowy (np. /dev/pts/4)
python simulator_serial.py

# Użyj pliku konfiguracyjnego
python simulator_serial.py --config simulator_config.py
```

Po uruchomieniu symulator wyświetli ścieżkę do wirtualnego portu oraz stałą ścieżkę (symlink), np.:
```
✓ Virtual serial port created: /dev/pts/4
✓ Stable device path: /tmp/perla-bus-tty
  Use this port in can-receiver config: device_port=/tmp/perla-bus-tty
```

#### Konfiguracja can-receiver

Edytuj `can-receiver/config.txt` i ustaw stałą ścieżkę:
```
device_port=/tmp/perla-bus-tty
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
python simulator_serial.py --dbc moj_plik.dbc

# Symuluj przez określony czas (30 sekund)
python simulator_serial.py --duration 30

# Wyślij każdą wiadomość raz i zakończ
python simulator_serial.py --single-cycle

# Użyj konkretnego portu szeregowego zamiast PTY
python simulator_serial.py --port /dev/ttyUSB1

# Przyspiesz symulację 2x (cykle 2x krótsze)
python simulator_serial.py --cycle-multiplier 0.5

# Zwolnij symulację 2x (cykle 2x dłuższe)
python simulator_serial.py --cycle-multiplier 2.0

# Tryb realistyczny - stopniowe zmiany wartości
python simulator_serial.py --realistic

# Użyj konfiguracji z pliku
python simulator_serial.py --config simulator_config.py

# Pomoc
python simulator_serial.py --help
```

#### Format protokołu Waveshare

Symulator implementuje protokół Waveshare USB-CAN-A:
```
[0xAA] [0x55] [0x01] [ID_TYPE] [FRAME_TYPE] [ID0] [ID1] [ID2] [ID3] [DLC] [D0] ... [D7] [0x00] [Checksum]
```
- `ID_TYPE`: `0x01` = standard (11-bit), `0x02` = extended (29-bit)
- `FRAME_TYPE`: `0x01` = data frame
- `ID0..ID3`: CAN ID jako `u32` little-endian (dla standard ID używane niższe 11 bitów)
- `D0..D7`: dane (z paddingiem `0x00` do 8 bajtów)
- Byte `0x00` przed checksum to pole zarezerwowane
- `Checksum`: suma bajtów od `[0x01]` do pola zarezerwowanego (włącznie) modulo 256

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

### Integracja z can-receiver

Symulator może być użyty do testowania aplikacji `can-receiver`:

```bash
# Terminal 1: Uruchom symulator
python simulator_serial.py

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