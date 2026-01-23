/**
 * ESP32-C3 Android Auto WiFi Bridge
 * 
 * This firmware creates a WiFi Access Point that acts as a bridge
 * between an Android device and Android Auto head units.
 * 
 * Compatible with ESP32-C3 rev 0.4 (early silicon)
 */

#include <Arduino.h>
#include <WiFi.h>
#include <WiFiAP.h>
#include <WiFiClient.h>
#include <WiFiServer.h>

// ============================================================================
// Configuration
// ============================================================================

// WiFi AP Settings
const char* AP_SSID = "AndroidAutoWiFi";
const char* AP_PASSWORD = "android123";
const int AP_CHANNEL = 6;
const bool AP_HIDDEN = false;
const int AP_MAX_CONNECTIONS = 1;

// Network Settings
const IPAddress AP_IP(192, 168, 4, 1);
const IPAddress AP_GATEWAY(192, 168, 4, 1);
const IPAddress AP_SUBNET(255, 255, 255, 0);

// TCP Server for Android Auto communication
const int AA_PORT = 5288;
WiFiServer aaServer(AA_PORT);
WiFiClient aaClient;

// ============================================================================
// State
// ============================================================================

enum class State {
    INIT,
    AP_STARTING,
    AP_READY,
    CLIENT_CONNECTED,
    AA_ACTIVE,
    ERROR
};

State currentState = State::INIT;
unsigned long lastHeartbeat = 0;
unsigned long clientConnectedTime = 0;
uint32_t heartbeatCount = 0;

// ============================================================================
// Function Declarations
// ============================================================================

void setupWiFiAP();
void handleClient();
void printStatus();
String stateToString(State s);

// ============================================================================
// Setup
// ============================================================================

void setup() {
    // Initialize serial
    Serial.begin(115200);
    delay(1000); // Wait for serial to stabilize
    
    Serial.println();
    Serial.println("╔══════════════════════════════════════════╗");
    Serial.println("║  ESP32-C3 Android Auto WiFi Bridge       ║");
    Serial.println("║  Version 1.0.0                           ║");
    Serial.println("╚══════════════════════════════════════════╝");
    Serial.println();
    
    // Print chip info
    Serial.printf("Chip Model: %s\n", ESP.getChipModel());
    Serial.printf("Chip Revision: %d\n", ESP.getChipRevision());
    Serial.printf("CPU Frequency: %d MHz\n", ESP.getCpuFreqMHz());
    Serial.printf("Flash Size: %d KB\n", ESP.getFlashChipSize() / 1024);
    Serial.printf("Free Heap: %d bytes\n", ESP.getFreeHeap());
    Serial.println();
    
    // Setup WiFi AP
    setupWiFiAP();
}

// ============================================================================
// Main Loop
// ============================================================================

void loop() {
    unsigned long now = millis();
    
    // Heartbeat every 5 seconds
    if (now - lastHeartbeat >= 5000) {
        lastHeartbeat = now;
        heartbeatCount++;
        printStatus();
    }
    
    // Handle state machine
    switch (currentState) {
        case State::AP_READY:
            // Check for new clients
            if (aaServer.hasClient()) {
                if (aaClient && aaClient.connected()) {
                    // Already have a client, reject new one
                    WiFiClient reject = aaServer.available();
                    reject.stop();
                    Serial.println("[WARN] Rejected additional client connection");
                } else {
                    aaClient = aaServer.available();
                    if (aaClient) {
                        currentState = State::CLIENT_CONNECTED;
                        clientConnectedTime = now;
                        Serial.println("[INFO] Client connected!");
                        Serial.printf("       Remote IP: %s\n", aaClient.remoteIP().toString().c_str());
                    }
                }
            }
            break;
            
        case State::CLIENT_CONNECTED:
        case State::AA_ACTIVE:
            handleClient();
            break;
            
        case State::ERROR:
            // Try to recover
            delay(5000);
            setupWiFiAP();
            break;
            
        default:
            break;
    }
    
    // Small delay to prevent watchdog issues
    delay(10);
}

// ============================================================================
// WiFi AP Setup
// ============================================================================

void setupWiFiAP() {
    currentState = State::AP_STARTING;
    Serial.println("[INFO] Starting WiFi Access Point...");
    
    // Disconnect any existing connections
    WiFi.disconnect(true);
    WiFi.mode(WIFI_AP);
    
    // Configure AP
    if (!WiFi.softAPConfig(AP_IP, AP_GATEWAY, AP_SUBNET)) {
        Serial.println("[ERROR] AP Config failed!");
        currentState = State::ERROR;
        return;
    }
    
    // Start AP
    if (!WiFi.softAP(AP_SSID, AP_PASSWORD, AP_CHANNEL, AP_HIDDEN, AP_MAX_CONNECTIONS)) {
        Serial.println("[ERROR] AP Start failed!");
        currentState = State::ERROR;
        return;
    }
    
    // Start TCP server
    aaServer.begin();
    aaServer.setNoDelay(true);
    
    Serial.println("[OK] WiFi AP started successfully!");
    Serial.printf("     SSID: %s\n", AP_SSID);
    Serial.printf("     Password: %s\n", AP_PASSWORD);
    Serial.printf("     IP: %s\n", WiFi.softAPIP().toString().c_str());
    Serial.printf("     AA Port: %d\n", AA_PORT);
    Serial.println();
    Serial.println("📱 Connect your phone to this WiFi network");
    Serial.println("   Then open the Android Auto WiFi app");
    Serial.println();
    
    currentState = State::AP_READY;
}

// ============================================================================
// Client Handler
// ============================================================================

void handleClient() {
    if (!aaClient || !aaClient.connected()) {
        Serial.println("[INFO] Client disconnected");
        currentState = State::AP_READY;
        aaClient.stop();
        return;
    }
    
    // Check for incoming data
    while (aaClient.available()) {
        // Read data
        uint8_t buffer[512];
        int len = aaClient.read(buffer, sizeof(buffer));
        
        if (len > 0) {
            Serial.printf("[DATA] Received %d bytes from client\n", len);
            
            // For now, just echo back (placeholder for AA protocol)
            // In a real implementation, this would handle Android Auto protocol
            
            // Mark as active if receiving data
            if (currentState == State::CLIENT_CONNECTED) {
                currentState = State::AA_ACTIVE;
                Serial.println("[INFO] Android Auto session active");
            }
        }
    }
}

// ============================================================================
// Status Print
// ============================================================================

void printStatus() {
    Serial.printf("[HEARTBEAT #%d] State: %s | Heap: %d | Clients: %d\n",
        heartbeatCount,
        stateToString(currentState).c_str(),
        ESP.getFreeHeap(),
        WiFi.softAPgetStationNum()
    );
}

String stateToString(State s) {
    switch (s) {
        case State::INIT: return "INIT";
        case State::AP_STARTING: return "AP_STARTING";
        case State::AP_READY: return "AP_READY";
        case State::CLIENT_CONNECTED: return "CLIENT_CONNECTED";
        case State::AA_ACTIVE: return "AA_ACTIVE";
        case State::ERROR: return "ERROR";
        default: return "UNKNOWN";
    }
}
