# ESP32 Android Auto WiFi Dongle

A Wireless Android Auto bridge using ESP32-S2 that enables wireless projection from an Android phone to a car's head unit.

## Architecture Overview

```
┌─────────────────┐     Wi-Fi      ┌─────────────────┐     USB (AOA 2.0)    ┌──────────────┐
│  Android Phone  │ ◄─────────────►│    ESP32-S2     │◄───────────────────►│   Car Head   │
│  (AA Client)    │   Projection   │    (Bridge)     │   Accessory Mode    │    Unit      │
└─────────────────┘     Stream     └─────────────────┘                     └──────────────┘
```

## Project Structure

```
esp32-android-auto-wifi/
├── Cargo.toml              # Workspace root (shared + android rust-core)
├── firmware/               # ESP32-C3 firmware (PlatformIO / Arduino C++)
│   ├── platformio.ini
│   └── src/
│       └── main.cpp        # WiFi AP + TCP bridge (Android Auto protocol)
├── shared/                 # Shared protocol logic (DRY/SOLID)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs     # Message definitions
│       ├── buffer.rs       # Zero-copy buffer implementation
│       └── traits.rs       # DataForwarder trait
└── android-app/
    ├── rust-core/          # Rust JNI library
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── app/                # Kotlin Android app
        └── ...
```

## Low-Latency Strategy: Zero-Copy Buffers

The system employs a zero-copy architecture to minimize latency:

1. **Static Ring Buffers**: Pre-allocated buffers avoid heap allocation during runtime
2. **Direct DMA Access**: USB and Wi-Fi peripherals read/write directly to shared buffers
3. **No Intermediate Copies**: Data flows USB → Buffer → Wi-Fi without memcpy operations
4. **Lock-Free Design**: Uses atomic operations for buffer management in async context

## Building

### Prerequisites

```bash
# Install PlatformIO Core
curl -fsSL https://raw.githubusercontent.com/platformio/platformio-core-installer/master/get-platformio.py | python3 -
export PATH=$PATH:$HOME/.platformio/penv/bin
```

### Build Firmware

```bash
# PlatformIO (Arduino framework, ESP32-C3)
cd firmware
pio run
```

### Flash to ESP32-C3

```bash
cd firmware
pio run -t upload
```

### Build Android App

```bash
cd android-app
./gradlew assembleDebug
```

## USB AOA 2.0 Protocol

The Android Open Accessory 2.0 protocol allows the ESP32 to act as a USB host 
that switches the car's head unit into accessory mode:

1. **Detection**: Identify AOA-capable device via USB descriptors
2. **Version Check**: Send AOA_GET_PROTOCOL (vendor request 51)
3. **Send Strings**: Configure accessory identity (manufacturer, model, etc.)
4. **Start**: Send AOA_START (vendor request 53) to enter accessory mode
5. **Enumerate**: Device re-enumerates with accessory VID/PID (0x18D1/0x2D00)

## License

MIT License - See LICENSE file for details.

<!-- AUTO-UPDATE-DATE -->
**Última actualización:** 2026-02-26 15:51:00 -03
## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=madkoding/esp32-android-auto-wifi&type=Date)](https://star-history.com/#madkoding/esp32-android-auto-wifi&Date)
