# Obsługa dongla Waveshare USB-CAN-A

Ten program służy do obsługi dongla **Waveshare USB-CAN-A** i zawiera wbudowane sterowniki dla wszystkich systemów (Linux, Windows, MacOS).

## Jak to działa:

1. **Tworzenie pliku konfiguracyjnego**: Program tworzy plik konfiguracyjny i ładuje z niego konfigurację.
2. **Ładowanie pliku DBC**: Ładuje i parsuje plik `.dbc` umieszczony w tym samym folderze co plik wykonywalny programu.
3. **Odczyt ramek CAN**: Odczytuje ramki CAN, dekoduje je na wartość rzeczywistą przy pomocy pliku DBC oraz wyświetla je.

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
5. **Uruchom program**: Użyj polecenia podobnego jak na początku. Powinny być wyświetlane zdekodowane ramki.