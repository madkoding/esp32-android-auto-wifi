/**
 * WifiAutoService - Background Service for Android Auto WiFi Bridge
 *
 * This service runs in the background and:
 * 1. Monitors for ESP32 WiFi network (SSID: AndroidAuto_XXXX)
 * 2. Automatically connects when detected
 * 3. Establishes TCP connection with ESP32
 * 4. Triggers Android Auto projection when ready
 *
 * Lifecycle:
 * - Started on boot (via BootReceiver)
 * - Runs as foreground service with notification
 * - Manages WiFi and TCP connection state
 */
package com.androidauto.wifi.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.os.Binder
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import com.androidauto.wifi.MainActivity
import com.androidauto.wifi.R
import com.androidauto.wifi.RustBridge
import kotlinx.coroutines.*
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket

/**
 * Connection state for the service
 */
enum class ConnectionState {
    DISCONNECTED,
    SCANNING_WIFI,
    CONNECTING_WIFI,
    WIFI_CONNECTED,
    CONNECTING_TCP,
    TCP_CONNECTED,
    HANDSHAKING,
    READY,
    ERROR
}

/**
 * Main service class for WiFi bridge management
 */
class WifiAutoService : Service() {
    
    companion object {
        private const val TAG = "WifiAutoService"
        private const val NOTIFICATION_ID = 1001
        private const val CHANNEL_ID = "wifi_auto_channel"
        
        // ESP32 network configuration
        private const val ESP32_SSID = "AndroidAutoWiFi"
        private const val ESP32_DEFAULT_IP = "192.168.4.1"
        private const val ESP32_TCP_PORT = 5288
        private const val ESP32_PASSWORD = "android123"
        
        // Reconnect settings
        private const val RECONNECT_DELAY_MS = 10000L
        private const val READ_BUFFER_SIZE = 16384
    }
    
    // Service binder for activity binding
    private val binder = LocalBinder()
    
    // Coroutine scope for async operations
    private val serviceScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    
    // System services
    private lateinit var wifiManager: WifiManager
    private lateinit var connectivityManager: ConnectivityManager
    private lateinit var notificationManager: NotificationManager
    
    // Connection state
    private var connectionState = ConnectionState.DISCONNECTED
    private var tcpSocket: Socket? = null
    private var inputStream: InputStream? = null
    private var outputStream: OutputStream? = null
    
    // Network callback for WiFi connection
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    
    // State listener (for UI updates)
    var stateListener: ((ConnectionState, String?) -> Unit)? = null
    
    inner class LocalBinder : Binder() {
        fun getService(): WifiAutoService = this@WifiAutoService
    }
    
    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Service created")
        
        // Initialize system services
        wifiManager = getSystemService(Context.WIFI_SERVICE) as WifiManager
        connectivityManager = getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        
        // Initialize Rust bridge
        if (!RustBridge.initializeAndCheck()) {
            Log.e(TAG, "Failed to initialize Rust bridge")
            updateState(ConnectionState.ERROR, "Failed to initialize native library")
            return
        }
        
        // Create notification channel
        createNotificationChannel()
        
        // Start as foreground service
        startForeground(NOTIFICATION_ID, createNotification("Initializing..."))
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.i(TAG, "Service started")
        
        // Start scanning for ESP32 network
        serviceScope.launch {
            startWifiConnection()
        }
        
        return START_STICKY
    }
    
    override fun onBind(intent: Intent?): IBinder {
        return binder
    }
    
    override fun onDestroy() {
        super.onDestroy()
        Log.i(TAG, "Service destroyed")
        
        // Cleanup
        disconnect()
        serviceScope.cancel()
        networkCallback?.let { connectivityManager.unregisterNetworkCallback(it) }
    }
    
    /**
     * Create notification channel for Android O+
     */
    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Android Auto WiFi Bridge",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Connection status for Android Auto WiFi bridge"
                setShowBadge(false)
            }
            notificationManager.createNotificationChannel(channel)
        }
    }
    
    /**
     * Create foreground notification
     */
    private fun createNotification(status: String): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE
        )
        
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Android Auto WiFi")
            .setContentText(status)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }
    
    /**
     * Update connection state and notify listeners
     */
    private fun updateState(state: ConnectionState, message: String? = null) {
        connectionState = state
        Log.i(TAG, "State: $state ${message?.let { "($it)" } ?: ""}")
        
        // Update notification
        val statusText = when (state) {
            ConnectionState.DISCONNECTED -> "Disconnected"
            ConnectionState.SCANNING_WIFI -> "Scanning for ESP32..."
            ConnectionState.CONNECTING_WIFI -> "Connecting to WiFi..."
            ConnectionState.WIFI_CONNECTED -> "WiFi connected"
            ConnectionState.CONNECTING_TCP -> "Establishing connection..."
            ConnectionState.TCP_CONNECTED -> "TCP connected"
            ConnectionState.HANDSHAKING -> "Handshaking..."
            ConnectionState.READY -> "Ready for Android Auto"
            ConnectionState.ERROR -> message ?: "Error"
        }
        notificationManager.notify(NOTIFICATION_ID, createNotification(statusText))
        
        // Notify listener
        stateListener?.invoke(state, message)
    }
    
    /**
     * Start WiFi connection to ESP32
     */
    private suspend fun startWifiConnection() {
        updateState(ConnectionState.SCANNING_WIFI)
        
        // Use WiFi Network Specifier (Android 10+)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            connectToEsp32NetworkQ()
        } else {
            // Legacy WiFi connection for older devices
            connectToEsp32NetworkLegacy()
        }
    }
    
    /**
     * Connect to ESP32 using Android 10+ API
     */
    private fun connectToEsp32NetworkQ() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        
        // Don't request again if we already have a pending request
        if (networkCallback != null) {
            Log.i(TAG, "Network request already pending, skipping")
            return
        }
        
        updateState(ConnectionState.CONNECTING_WIFI)
        
        // Build WiFi network specifier - use exact SSID match
        val specifier = WifiNetworkSpecifier.Builder()
            .setSsid(ESP32_SSID)
            .setWpa2Passphrase(ESP32_PASSWORD)
            .build()
        
        // Build network request
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .setNetworkSpecifier(specifier)
            .build()
        
        // Register callback
        networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                super.onAvailable(network)
                Log.i(TAG, "WiFi network available")
                
                // Bind to this network for socket operations
                connectivityManager.bindProcessToNetwork(network)
                
                updateState(ConnectionState.WIFI_CONNECTED)
                
                // Start TCP connection
                serviceScope.launch {
                    connectTcp()
                }
            }
            
            override fun onUnavailable() {
                super.onUnavailable()
                Log.w(TAG, "WiFi network unavailable - user cancelled or timeout")
                updateState(ConnectionState.DISCONNECTED, "Connection cancelled")
                
                // Unregister callback to prevent duplicate dialogs
                networkCallback?.let { 
                    try {
                        connectivityManager.unregisterNetworkCallback(it)
                    } catch (e: Exception) {
                        Log.w(TAG, "Error unregistering callback", e)
                    }
                }
                networkCallback = null
            }
            
            override fun onLost(network: Network) {
                super.onLost(network)
                Log.w(TAG, "WiFi network lost")
                disconnect()
            }
        }
        
        try {
            connectivityManager.requestNetwork(request, networkCallback!!)
        } catch (e: Exception) {
            Log.e(TAG, "Error requesting network", e)
            networkCallback = null
            updateState(ConnectionState.ERROR, "Failed to request network")
        }
    }
    
    /**
     * Legacy WiFi connection for Android < 10
     */
    @Suppress("DEPRECATION")
    private suspend fun connectToEsp32NetworkLegacy() {
        updateState(ConnectionState.ERROR, "Android 10+ required")
        // Legacy implementation would go here
        // Not implementing as target is Android 10+
    }
    
    /**
     * Connect to ESP32 via TCP
     */
    private suspend fun connectTcp() {
        updateState(ConnectionState.CONNECTING_TCP)
        
        try {
            withContext(Dispatchers.IO) {
                // Create socket and connect
                tcpSocket = Socket(ESP32_DEFAULT_IP, ESP32_TCP_PORT).apply {
                    soTimeout = 30000 // 30 second read timeout
                    tcpNoDelay = true // Disable Nagle for low latency
                    keepAlive = true // Keep connection alive
                }
                inputStream = tcpSocket?.getInputStream()
                outputStream = tcpSocket?.getOutputStream()
            }
            
            Log.i(TAG, "TCP connected to $ESP32_DEFAULT_IP:$ESP32_TCP_PORT")
            updateState(ConnectionState.TCP_CONNECTED)
            
            // Simple handshake - send hello message
            performHandshake()
            
        } catch (e: Exception) {
            Log.e(TAG, "TCP connection failed", e)
            updateState(ConnectionState.ERROR, "TCP: ${e.message}")
            disconnect()
        }
    }
    
    /**
     * Perform protocol handshake with ESP32
     */
    private suspend fun performHandshake() {
        updateState(ConnectionState.HANDSHAKING)
        
        try {
            // Send a simple hello message to ESP32
            val helloMsg = "HELLO_ANDROID_AUTO\n".toByteArray()
            withContext(Dispatchers.IO) {
                outputStream?.write(helloMsg)
                outputStream?.flush()
            }
            
            Log.i(TAG, "Handshake sent")
            updateState(ConnectionState.READY)
            
            // Start receive loop
            startReceiveLoop()
            
        } catch (e: Exception) {
            Log.e(TAG, "Handshake failed", e)
            updateState(ConnectionState.ERROR, "Handshake failed")
            disconnect()
        }
    }
    
    /**
     * Start receiving data from ESP32
     */
    private fun startReceiveLoop() {
        serviceScope.launch {
            val buffer = ByteArray(READ_BUFFER_SIZE)
            
            while (isActive && tcpSocket?.isConnected == true && !tcpSocket!!.isClosed) {
                try {
                    val bytesRead = withContext(Dispatchers.IO) {
                        inputStream?.read(buffer) ?: -1
                    }
                    
                    if (bytesRead > 0) {
                        val data = buffer.copyOf(bytesRead)
                        val message = String(data).trim()
                        Log.d(TAG, "Received: $message")
                    } else if (bytesRead == -1) {
                        // Connection closed
                        Log.w(TAG, "TCP connection closed by remote")
                        break
                    }
                } catch (e: java.net.SocketTimeoutException) {
                    // Timeout is normal, send keepalive
                    try {
                        withContext(Dispatchers.IO) {
                            outputStream?.write("PING\n".toByteArray())
                            outputStream?.flush()
                        }
                    } catch (e2: Exception) {
                        Log.e(TAG, "Keepalive failed", e2)
                        break
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Receive error", e)
                    break
                }
            }
            
            // Connection lost
            Log.w(TAG, "Receive loop ended")
            disconnect()
        }
    }
    
    /**
     * Trigger Android Auto projection intent
     */
    private fun triggerAndroidAuto() {
        Log.i(TAG, "Triggering Android Auto projection")
        
        // Send intent to start Android Auto projection
        // This uses the CarProjection API when available
        try {
            val intent = Intent().apply {
                action = "android.car.intent.action.PROJECTION"
                putExtra("android.car.intent.extra.PROJECTION_SOURCE", "ESP32_WIFI_BRIDGE")
            }
            sendBroadcast(intent)
            Log.i(TAG, "Android Auto projection intent sent")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to trigger Android Auto", e)
        }
    }
    
    /**
     * Send data to ESP32
     */
    fun sendData(channel: Int, data: ByteArray): Boolean {
        if (connectionState != ConnectionState.READY) {
            return false
        }
        
        val bytesSent = RustBridge.sendData(channel, data)
        return bytesSent >= 0
    }
    
    /**
     * Disconnect from ESP32
     */
    fun disconnect() {
        Log.i(TAG, "Disconnecting...")
        
        RustBridge.disconnect()
        
        try {
            inputStream?.close()
            outputStream?.close()
            tcpSocket?.close()
        } catch (e: Exception) {
            Log.w(TAG, "Error closing socket", e)
        }
        
        inputStream = null
        outputStream = null
        tcpSocket = null
        
        // Unregister network callback
        networkCallback?.let { 
            try {
                connectivityManager.unregisterNetworkCallback(it)
            } catch (e: Exception) {
                Log.w(TAG, "Error unregistering network callback", e)
            }
        }
        networkCallback = null
        
        // Unbind from network
        try {
            connectivityManager.bindProcessToNetwork(null)
        } catch (e: Exception) {
            Log.w(TAG, "Error unbinding from network", e)
        }
        
        updateState(ConnectionState.DISCONNECTED)
    }
    
    /**
     * Get current connection state
     */
    fun getConnectionState(): ConnectionState = connectionState
    
    /**
     * Get connection statistics
     */
    fun getStats(): String = RustBridge.getStats()
}
