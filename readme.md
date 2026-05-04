# Odbiornik i dekoder CAN

Ten program służy do odbierania i dekodowania ramek CAN na podstawie dostarczonego pliku DBC. Ma 2 tryby działania (zależne od architektury procesora):

1. x86-64
    - Obsługa dongla **Waveshare USB-CAN-A** - zawiera wbudowane sterowniki dla wszystkich systemów (Linux, Windows, MacOS)
2. arm64 (Linux)
    - Odbiór ramek poprzez moduł zgodny z `socketcan`

## Funkcje:

1. **Tworzenie pliku konfiguracyjnego**: Program tworzy plik konfiguracyjny i ładuje z niego konfigurację.
2. **Ładowanie pliku DBC**: Ładuje i parsuje plik `.dbc` umieszczony w tym samym folderze co plik wykonywalny programu.
3. **Odczyt ramek CAN**: Odczytuje ramki CAN, dekoduje je na wartość rzeczywistą przy pomocy pliku DBC.
4. **WebSocket API**: Udostępnia dane CAN przez WebSocket na porcie 8080 z obsługą snapshot + delta updates oraz filtrowaniem wiadomości.

## Pierwsze uruchomienie:

1. **Pobierz plik wykonywalny**: Z [Releases](https://github.com/Eko-Energia/Perla-Monitor/releases) pobierz plik zgodny z twoim systemem.
2. **Umieść go w nowym katalogu** i uruchom:
   - **Windows**: 
     ```bash
     .\can-receiver.exe
     ```
     (np. w cmd, PowerShell, Windows Terminal)
   - **Linux**: 
     ```bash
     # jednorazowo nadaj uprawnienia
     chmod u+r+w+x can-receiver
     
     ./can-receiver
     ```
     (w terminalu)

3. **Konfiguracja portu**: Po uruchomieniu zostanie utworzony plik `config.txt`. Należy w nim ustawić `device port` w formacie `device_port=`, po podłączeniu dongla USB CAN:
   - **Windows**: 
     - Wejdź w menedżer urządzeń, rozwiń pole "Porty (COM i LPT)", znajdź port, pod którym widnieje Waveshare USB CAN, a następnie wpisz go w config, np. 
     ```plaintext
     device_port=COM4
     ```
   - **Linux**: 
     - Wywołaj w terminalu 
     ```bash
     ls /dev/ttyUSB*
     ```
     - Następnie wypróbuj każdy z dostępnych, np. `/dev/ttyUSB0`, aż trafisz na dobry i zadziała.

4. **Umieść plik DBC**: Umieść plik DBC w katalogu z plikiem wykonywalnym.
5. **Uruchom program**: Użyj polecenia podobnego jak na początku.

## Uruchomienie na arm64 (z socketcan)

Uruchomienie socketcan wymaga podłączenia oraz konfiguracji odpowiedniego modułu. Poniżej znajduje się przykład dla `MCP2515` na Raspberry Pi 4B:

1. Podłącz moduł oraz uruchom SPI korzystając z [tego](https://github.com/tolgakarakurt/CANBus-MCP2515-Raspi?tab=readme-ov-file) poradnika.
2. Wywołaj poniżesze polecenie:
    ```
    sudo nano /boot/firmware/config.txt
    ```
    oraz wklej na koniec pliku następującą treść:
    ```
    dtoverlay=mcp2515-can0,oscillator=8000000,interrupt=25
    dtoverlay=spi-dma
    ```
3. Wykonaj reboot.
4. Wykonaj:
    ```
    sudo apt install autoconf autogen
    sudo apt install libtool
    sudo apt install can-utils
    ```
5. Sprawdź czy moduł CAN jest widoczny w systemie (np. jako `can0`):
    ```
    ls /sys/bus/spi/devices/spi0.0/net
    ```

    Wprowadź tę nazwę jako `device_port=` w pliku `config.txt`.
    
6. Skonfiguruj socketcan:
    ```
    sudo ip link set can0 up type can bitrate 500000
    ```

    Uwaga! To polecenie należy wywołać bo każdym ponownym uruchomieniu systemu.
7. Uruchom program w standardowy sposób.

## WebSocket API

Program automatycznie uruchamia serwer WebSocket na `ws://0.0.0.0:8080`, który umożliwia zdalny dostęp do danych CAN w czasie rzeczywistym.

### Szybki start

1. Otwórz plik `websocket-test-client.html` w przeglądarce
2. Kliknij "Connect" - automatycznie połączy się i pobierze wszystkie dane
3. Obserwuj aktualizacje w czasie rzeczywistym

### Możliwości

- **Snapshot + Delta**: Najpierw otrzymujesz pełny stan, potem tylko zmiany
- **Filtrowanie**: Możesz subskrybować tylko wybrane wiadomości CAN (np. tylko `BMS_Status, Motor_Speed`)
- **Wielokrotne połączenia**: Obsługa wielu klientów jednocześnie

Szczegóły API i przykłady w różnych językach: [WEBSOCKET_API.md](WEBSOCKET_API.md)

## Kompilacja

### Standardowa
```
cargo build --release
```

### Cross-compile na arm64 (np. Raspberry Pi):
```
sudo apt install zig
cargo install cargo-zigbuild
rustup target add aarch64-unknown-linux-gnu
```
```
cargo zigbuild --target aarch64-unknown-linux-gnu --release
```