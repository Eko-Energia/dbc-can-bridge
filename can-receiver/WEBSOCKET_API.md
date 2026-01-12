# WebSocket API

Serwer WebSocket nasłuchuje domyślnie na `ws://0.0.0.0:8080`.

## Protokół komunikacji

### 1. Połączenie i snapshot

Po nawiązaniu połączenia klient **nie** dostaje automatycznie snapshotu. 
Klient **musi** najpierw wysłać wiadomość `subscribe` żeby określić co chce odbierać.

### 2. Subskrypcja (klient → serwer)

```json
{
  "type": "subscribe",
  "message_names": ["BMS_Status", "Motor_Speed"]
}
```

- `message_names` - lista nazw wiadomości CAN do subskrypcji
- Jeśli pusta lista `[]` - dostaje **wszystko**

**Po wysłaniu subskrypcji** serwer natychmiast odpowie:
1. Jedną wiadomością `snapshot` z obecnym stanem wszystkich subskrybowanych wiadomości
2. Potem ciągłym strumieniem `update` tylko dla subskrybowanych wiadomości

### 3. Snapshot (serwer → klient)

Wysyłany raz po otrzymaniu `subscribe`:

```json
{
  "type": "snapshot",
  "data": {
    "BMS_Status": {
      "signals": [
        {
          "name": "Battery_Voltage",
          "value": 48.5,
          "unit": "V"
        },
        {
          "name": "Battery_Current",
          "value": 12.3,
          "unit": "A"
        }
      ],
      "timestamp": "2026-01-12T14:23:45.123456789+01:00"
    },
    "Motor_Speed": {
      "signals": [
        {
          "name": "RPM",
          "value": 3500.0,
          "unit": "rpm"
        }
      ],
      "timestamp": "2026-01-12T14:23:45.234567890+01:00"
    }
  }
}
```

### 4. Update (serwer → klient)

Wysyłany za każdym razem gdy dana wiadomość CAN się zaktualizuje (tylko subskrybowane):

```json
{
  "type": "update",
  "message_name": "BMS_Status",
  "entry": {
    "signals": [
      {
        "name": "Battery_Voltage",
        "value": 48.6,
        "unit": "V"
      },
      {
        "name": "Battery_Current",
        "value": 12.4,
        "unit": "A"
      }
    ],
    "timestamp": "2026-01-12T14:23:46.123456789+01:00"
  }
}
```

## Przykłady użycia

### JavaScript/TypeScript (przeglądarka)

```javascript
const ws = new WebSocket('ws://localhost:8080');

ws.onopen = () => {
  console.log('Connected to CAN receiver');
  
  // Subskrybuj wybrane wiadomości
  ws.send(JSON.stringify({
    type: 'subscribe',
    message_names: ['BMS_Status', 'Motor_Speed']
  }));
  
  // Lub subskrybuj wszystko:
  // ws.send(JSON.stringify({ type: 'subscribe', message_names: [] }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  if (msg.type === 'snapshot') {
    console.log('Initial state:', msg.data);
    // Zainicjuj UI z pełnym stanem
    Object.entries(msg.data).forEach(([msgName, entry]) => {
      updateUI(msgName, entry);
    });
  } 
  else if (msg.type === 'update') {
    console.log('Update:', msg.message_name, msg.entry);
    // Zaktualizuj tylko ten jeden wpis w UI
    updateUI(msg.message_name, msg.entry);
  }
};

function updateUI(messageName, entry) {
  entry.signals.forEach(signal => {
    console.log(`${messageName}.${signal.name} = ${signal.value} ${signal.unit}`);
    // Aktualizuj elementy DOM, wykresy itp.
  });
}
```

### Python

```python
import asyncio
import websockets
import json

async def can_client():
    async with websockets.connect('ws://localhost:8080') as websocket:
        # Subskrybuj wybrane wiadomości
        await websocket.send(json.dumps({
            'type': 'subscribe',
            'message_names': ['BMS_Status', 'Motor_Speed']
        }))
        
        async for message in websocket:
            data = json.loads(message)
            
            if data['type'] == 'snapshot':
                print('Initial snapshot:')
                for msg_name, entry in data['data'].items():
                    print(f"  {msg_name}:")
                    for signal in entry['signals']:
                        print(f"    {signal['name']}: {signal['value']} {signal['unit']}")
            
            elif data['type'] == 'update':
                print(f"Update {data['message_name']}:")
                for signal in data['entry']['signals']:
                    print(f"  {signal['name']}: {signal['value']} {signal['unit']}")

asyncio.run(can_client())
```

### Rust

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (ws_stream, _) = connect_async("ws://localhost:8080").await?;
    let (mut write, mut read) = ws_stream.split();
    
    // Subskrybuj
    let subscribe_msg = json!({
        "type": "subscribe",
        "message_names": ["BMS_Status", "Motor_Speed"]
    });
    write.send(Message::Text(subscribe_msg.to_string())).await?;
    
    // Odbieraj wiadomości
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            println!("Received: {}", text);
        }
    }
    
    Ok(())
}
```

## Zmiana subskrypcji

Klient może w dowolnym momencie wysłać nową wiadomość `subscribe` - wtedy:
1. Stare subskrypcje są zastępowane nowymi
2. Serwer natychmiast wysyła nowy snapshot dla nowych subskrypcji

```javascript
// Zmień subskrypcję na inny zestaw wiadomości
ws.send(JSON.stringify({
  type: 'subscribe',
  message_names: ['Different_Message']
}));
// → dostaniesz snapshot tylko z Different_Message
```

## Wydajność

- **Brak kopiowania**: Dane są serializowane bezpośrednio z referencji (`&str`, `&[SignalValueDto]`)
- **Filtrowanie**: Aktualizacje są wysyłane tylko dla subskrybowanych wiadomości
- **Non-blocking**: Jeśli kanał do klienta jest pełny, aktualizacja jest pomijana (klient nie blokuje głównej pętli CAN)
- **Timestamp**: Format RFC3339 z nanosekundową precyzją

## Obsługa błędów

- Jeśli klient wyśle nieprawidłowy JSON → logowane jako warning, połączenie trwa
- Jeśli klient się rozłączy → automatyczne cleanup, usunięcie z listy klientów
- Jeśli serwer WebSocket crashuje → główna pętla CAN nadal działa
