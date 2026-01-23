# Quick Start Guide

This guide will help you build and deploy the ESP32 Android Auto WiFi Bridge.

## Prerequisites

### For Firmware (ESP32-S2)

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Install ESP Rust toolchain**:
   ```bash
   # Install espup (ESP Rust toolchain manager)
   cargo install espup
   
   # Install the ESP32 Rust toolchain
   espup install
   
   # Source the environment (add to .bashrc/.zshrc for persistence)
   . $HOME/export-esp.sh
   ```

3. **Install cargo-espflash** (for flashing):
   ```bash
   cargo install cargo-espflash espflash
   ```

4. **Install system dependencies** (Linux):
   ```bash
   # Ubuntu/Debian
   sudo apt-get install -y git wget flex bison gperf python3 python3-pip \
       python3-venv cmake ninja-build ccache libffi-dev libssl-dev dfu-util \
       libusb-1.0-0 libudev-dev
   
   # Add udev rules for ESP32 (required for non-root flashing)
   sudo usermod -a -G dialout $USER
   # Log out and log back in for group changes to take effect
   ```

### For Android App

1. **Install Android SDK**:
   - Download [Android Studio](https://developer.android.com/studio)
   - Or install command-line tools only

2. **Install Android NDK** (required for Rust JNI):
   ```bash
   # Via Android Studio: SDK Manager > SDK Tools > NDK
   # Or via command line:
   sdkmanager "ndk;25.2.9519653"
   ```

3. **Install Rust Android targets**:
   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
   ```

4. **Install cargo-ndk** (optional, for manual builds):
   ```bash
   cargo install cargo-ndk
   ```

---

## Building the Firmware

### Option 1: Using VS Code Tasks (Recommended)

1. Open the project in VS Code
2. Press `Ctrl+Shift+P` → "Tasks: Run Task"
3. Select **"Firmware: Build (Release)"**

### Option 2: Command Line

```bash
# Navigate to firmware directory
cd firmware

# Build debug version
cargo build

# Build release version (optimized, smaller)
cargo build --release
```

### Build Output

The compiled firmware will be at:
- Debug: `target/xtensa-esp32s2-none-elf/debug/firmware`
- Release: `target/xtensa-esp32s2-none-elf/release/firmware`

---

## Flashing the Firmware

### Step 1: Connect ESP32-S2

1. Connect ESP32-S2 to your computer via USB
2. Put ESP32-S2 in **bootloader mode**:
   - Hold the **BOOT** button
   - Press and release the **RESET** button
   - Release the **BOOT** button
   - (Some boards enter bootloader automatically)

### Step 2: Identify the Serial Port

```bash
# Linux
ls /dev/ttyUSB* /dev/ttyACM*

# The ESP32-S2 usually appears as /dev/ttyUSB0 or /dev/ttyACM0
```

### Step 3: Flash the Firmware

#### Option A: Using VS Code Tasks

1. Press `Ctrl+Shift+P` → "Tasks: Run Task"
2. Select **"Firmware: Flash (Release)"**
3. Select the serial port when prompted
4. Wait for flashing to complete

#### Option B: Using Command Line

```bash
cd firmware

# Flash and open serial monitor
cargo espflash flash --release --monitor

# Or specify port explicitly
cargo espflash flash --release --monitor --port /dev/ttyUSB0

# Flash without monitor
cargo espflash flash --release
```

#### Option C: Using espflash directly

```bash
# Flash the binary directly
espflash flash target/xtensa-esp32s2-none-elf/release/firmware

# With specific port and baud rate
espflash flash --port /dev/ttyUSB0 --baud 921600 \
    target/xtensa-esp32s2-none-elf/release/firmware
```

### Step 4: Monitor Serial Output

```bash
# Start serial monitor
cargo espflash monitor

# Or with espflash
espflash monitor --port /dev/ttyUSB0
```

Expected output:
```
ESP32-S2 Android Auto WiFi Bridge
==================================
Firmware version: 0.1.0
Heap initialized: 72 KB
Peripherals initialized
Embassy runtime initialized
Zero-copy buffers initialized: 32 KB each
...
```

---

## Building the Android APK

### Option 1: Using VS Code Tasks

1. Press `Ctrl+Shift+P` → "Tasks: Run Task"
2. Select **"Android: Build Debug APK"**

### Option 2: Using Command Line

```bash
cd android-app

# Make gradlew executable (first time only)
chmod +x gradlew

# Build debug APK
./gradlew assembleDebug

# Build release APK
./gradlew assembleRelease
```

### APK Output Location

- Debug: `android-app/app/build/outputs/apk/debug/app-debug.apk`
- Release: `android-app/app/build/outputs/apk/release/app-release.apk`

---

## Installing the APK

### Option 1: Using VS Code Tasks

1. Connect Android device via USB (enable USB debugging)
2. Press `Ctrl+Shift+P` → "Tasks: Run Task"
3. Select **"Android: Install Debug APK"**

### Option 2: Using ADB

```bash
# Check device is connected
adb devices

# Install debug APK
adb install android-app/app/build/outputs/apk/debug/app-debug.apk

# Install and replace existing
adb install -r android-app/app/build/outputs/apk/debug/app-debug.apk
```

### Option 3: Manual Installation

1. Copy the APK to your Android device
2. Open the file and install (enable "Install from unknown sources" if prompted)

---

## Full Deployment (Firmware + APK)

### Using VS Code Task

1. Connect ESP32-S2 via USB
2. Connect Android device via USB
3. Press `Ctrl+Shift+P` → "Tasks: Run Task"
4. Select **"Deploy: Full System"**

This will:
1. Run all tests
2. Build and flash firmware (release)
3. Build and install Android APK

### Manual Steps

```bash
# 1. Flash firmware
cd firmware
cargo espflash flash --release
cd ..

# 2. Build and install APK
cd android-app
./gradlew installDebug
```

---

## Troubleshooting

### Firmware Issues

| Problem | Solution |
|---------|----------|
| `Permission denied: /dev/ttyUSB0` | Add user to dialout group: `sudo usermod -a -G dialout $USER` then log out/in |
| `Failed to connect to ESP32-S2` | Enter bootloader mode (hold BOOT, press RESET) |
| `espflash not found` | Run `cargo install espflash cargo-espflash` |
| `Toolchain not found` | Run `. $HOME/export-esp.sh` |

### Android Issues

| Problem | Solution |
|---------|----------|
| `SDK location not found` | Create `local.properties` with `sdk.dir=/path/to/Android/Sdk` |
| `NDK not found` | Install NDK via Android Studio SDK Manager |
| `adb: device not found` | Enable USB debugging on Android device |
| `INSTALL_FAILED_UPDATE_INCOMPATIBLE` | Uninstall existing app: `adb uninstall com.androidauto.wifi` |

### Build Issues

```bash
# Clean and rebuild everything
cargo clean
cd android-app && ./gradlew clean && cd ..

# Rebuild
cargo build --workspace
cd android-app && ./gradlew assembleDebug
```

---

## Development Tips

### Watch Mode (Firmware)

```bash
# Install cargo-watch
cargo install cargo-watch

# Auto-rebuild on changes
cd firmware
cargo watch -x build
```

### View Android Logs

```bash
# Filter logs for our app
adb logcat -s RustCore:V WifiAutoService:V RustBridge:V

# Or use VS Code task: "Android: Run Logcat (Rust Core)"
```

### Serial Monitor Shortcuts

- `Ctrl+R` - Reset ESP32
- `Ctrl+C` - Exit monitor

---

## Next Steps

1. Power on ESP32-S2 with firmware
2. Install and open Android app
3. App will automatically scan for "AndroidAuto_XXXX" WiFi network
4. Connect your car's head unit via USB to ESP32-S2
5. Enjoy wireless Android Auto!

For detailed architecture and API documentation, see [README.md](README.md).
