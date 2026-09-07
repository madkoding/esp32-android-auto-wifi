# Quick Start Guide

This guide will help you build and deploy the ESP32 Android Auto WiFi Bridge.

## Prerequisites

### For Firmware (ESP32-C3, PlatformIO / Arduino)

1. **Install PlatformIO Core** (or use the PlatformIO IDE extension in VS Code):
   ```bash
   # Linux/macOS
   curl -fsSL https://raw.githubusercontent.com/platformio/platformio-core-installer/master/get-platformio.py | python3 -

   # Add to PATH (Linux)
   echo 'export PATH=$PATH:$HOME/.platformio/penv/bin' >> ~/.bashrc
   source ~/.bashrc
   ```

2. **Install system dependencies** (Linux, for serial access):
   ```bash
   # Ubuntu/Debian
   sudo apt-get install -y python3 python3-pip git
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

1. Open the project in VS Code (with the PlatformIO IDE extension)
2. Press `Ctrl+Shift+P` → "Tasks: Run Task"
3. Select **"Firmware: Build (Debug)"**

### Option 2: Command Line

```bash
# Navigate to firmware directory
cd firmware

# Build (debug)
pio run

# Build release (optimized, smaller)
pio run -e esp32c3
```

### Build Output

The compiled firmware will be at:
- `firmware/.pio/build/esp32c3/firmware.bin`

---

## Flashing the Firmware

### Step 1: Connect ESP32-C3

1. Connect ESP32-C3 to your computer via USB

### Step 2: Identify the Serial Port

```bash
# Linux
ls /dev/ttyUSB* /dev/ttyACM*

# The ESP32-C3 usually appears as /dev/ttyUSB0 or /dev/ttyACM0
```

### Step 3: Flash the Firmware

#### Option A: Using VS Code Tasks

1. Press `Ctrl+Shift+P` → "Tasks: Run Task"
2. Select **"Firmware: Flash (Upload)"**
3. Wait for flashing to complete

#### Option B: Using Command Line

```bash
cd firmware

# Flash and open serial monitor
pio run -t upload

# Or specify port explicitly
pio run -t upload --upload-port /dev/ttyUSB0
```

### Step 4: Monitor Serial Output

```bash
# Start serial monitor
pio device monitor
```

Expected output:
```
╔══════════════════════════════════════════╗
║  ESP32-C3 Android Auto WiFi Bridge       ║
║  Version 1.0.0                           ║
╚══════════════════════════════════════════╝
Chip Model: ESP32-C3
...
[OK] WiFi AP started successfully!
     SSID: AndroidAutoWiFi
     IP: 192.168.4.1
     AA Port: 5288
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

1. Connect ESP32-C3 via USB
2. Connect Android device via USB
3. Press `Ctrl+Shift+P` → "Tasks: Run Task"
4. Run **"Firmware: Flash (Upload)"** then **"Android: Install Debug APK"**

### Using Command Line

```bash
# Flash firmware
cd firmware
pio run -t upload

# Install APK
cd ../android-app
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `pio: command not found` | Add PlatformIO to PATH: `export PATH=$PATH:$HOME/.platformio/penv/bin` |
| `Failed to connect to ESP32-C3` | Check the USB cable (data, not charge-only) and the serial port |
| `Permission denied` on serial port | Add user to `dialout` group and re-login |
| `A fatal error occurred: Failed to write to target flash` | Hold BOOT button while flashing, or lower `upload_speed` in `platformio.ini` |
| Phone can't find the WiFi network | The AP SSID is `AndroidAutoWiFi` (password `android123`), channel 6 |
