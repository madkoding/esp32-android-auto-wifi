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
├── Cargo.toml              # Workspace root
├── firmware/               # ESP32-S2 firmware (embassy-esp32)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point with USB AOA 2.0 handshake
│       ├── usb_aoa.rs      # Android Open Accessory protocol
│       ├── wifi_ap.rs      # Wi-Fi Access Point management
│       └── bridge.rs       # DataForwarder implementation
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
# Install Rust ESP toolchain
cargo install espup
espup install

# Install flash tool
cargo install cargo-espflash

# Source ESP environment
. $HOME/export-esp.sh
```

### Build Firmware

```bash
cd firmware
cargo build --release
```

### Flash to ESP32-S2

```bash
cargo espflash flash --release --monitor
```

### Build Android App

```bash
cd android-app
./gradlew assembleDebug
```

## USB AOA 2.0 Protocol

The Android Open Accessory 2.0 protocol allows the ESP32-S2 to act as a USB host 
that switches the car's head unit into accessory mode:

1. **Detection**: Identify AOA-capable device via USB descriptors
2. **Version Check**: Send AOA_GET_PROTOCOL (vendor request 51)
3. **Send Strings**: Configure accessory identity (manufacturer, model, etc.)
4. **Start**: Send AOA_START (vendor request 53) to enter accessory mode
5. **Enumerate**: Device re-enumerates with accessory VID/PID (0x18D1/0x2D00)

## License

MIT License - See LICENSE file for details.

<!-- AUTO-UPDATE-DATE -->
**Última actualización:** 2026-02-19 17:10:25 -03
