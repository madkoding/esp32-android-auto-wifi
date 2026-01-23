/**
 * MainActivity - Main UI for Android Auto WiFi Bridge
 *
 * Provides:
 * - Connection status display
 * - Manual connect/disconnect controls
 * - Statistics view
 * - Service management
 */
package com.androidauto.wifi

import android.Manifest
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import com.androidauto.wifi.databinding.ActivityMainBinding
import com.androidauto.wifi.service.ConnectionState
import com.androidauto.wifi.service.WifiAutoService

class MainActivity : AppCompatActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        private const val PERMISSION_REQUEST_CODE = 1001
    }
    
    private lateinit var binding: ActivityMainBinding
    
    // Service binding
    private var wifiAutoService: WifiAutoService? = null
    private var serviceBound = false
    
    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            val binder = service as WifiAutoService.LocalBinder
            wifiAutoService = binder.getService()
            serviceBound = true
            
            // Set state listener
            wifiAutoService?.stateListener = { state, message ->
                runOnUiThread {
                    updateUI(state, message)
                }
            }
            
            // Update UI with current state
            wifiAutoService?.let {
                updateUI(it.getConnectionState(), null)
            }
            
            Log.i(TAG, "Service connected")
        }
        
        override fun onServiceDisconnected(name: ComponentName?) {
            wifiAutoService = null
            serviceBound = false
            Log.i(TAG, "Service disconnected")
        }
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        
        setupUI()
        checkPermissions()
    }
    
    override fun onStart() {
        super.onStart()
        
        // Bind to service
        Intent(this, WifiAutoService::class.java).also { intent ->
            bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
        }
    }
    
    override fun onStop() {
        super.onStop()
        
        // Unbind from service
        if (serviceBound) {
            wifiAutoService?.stateListener = null
            unbindService(serviceConnection)
            serviceBound = false
        }
    }
    
    /**
     * Setup UI elements and click listeners
     */
    private fun setupUI() {
        binding.apply {
            // Connect button
            btnConnect.setOnClickListener {
                if (wifiAutoService?.getConnectionState() == ConnectionState.READY) {
                    wifiAutoService?.disconnect()
                } else {
                    startService()
                }
            }
            
            // Refresh stats button
            btnRefreshStats.setOnClickListener {
                updateStats()
            }
        }
    }
    
    /**
     * Check and request required permissions
     */
    private fun checkPermissions() {
        val permissions = arrayOf(
            Manifest.permission.ACCESS_FINE_LOCATION,
            Manifest.permission.ACCESS_COARSE_LOCATION,
            Manifest.permission.ACCESS_WIFI_STATE,
            Manifest.permission.CHANGE_WIFI_STATE
        )
        
        val permissionsToRequest = permissions.filter {
            ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
        }
        
        if (permissionsToRequest.isNotEmpty()) {
            ActivityCompat.requestPermissions(
                this,
                permissionsToRequest.toTypedArray(),
                PERMISSION_REQUEST_CODE
            )
        } else {
            // All permissions granted, start service
            startService()
        }
    }
    
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        
        if (requestCode == PERMISSION_REQUEST_CODE) {
            if (grantResults.all { it == PackageManager.PERMISSION_GRANTED }) {
                startService()
            } else {
                Toast.makeText(
                    this,
                    "Permissions required for WiFi scanning",
                    Toast.LENGTH_LONG
                ).show()
            }
        }
    }
    
    /**
     * Start the WiFi Auto service
     */
    private fun startService() {
        Intent(this, WifiAutoService::class.java).also { intent ->
            ContextCompat.startForegroundService(this, intent)
        }
    }
    
    /**
     * Update UI based on connection state
     */
    private fun updateUI(state: ConnectionState, message: String?) {
        binding.apply {
            // Update status text
            tvStatus.text = when (state) {
                ConnectionState.DISCONNECTED -> "Disconnected"
                ConnectionState.SCANNING_WIFI -> "Scanning for ESP32..."
                ConnectionState.CONNECTING_WIFI -> "Connecting to WiFi..."
                ConnectionState.WIFI_CONNECTED -> "WiFi Connected"
                ConnectionState.CONNECTING_TCP -> "Connecting to ESP32..."
                ConnectionState.TCP_CONNECTED -> "TCP Connected"
                ConnectionState.HANDSHAKING -> "Handshaking..."
                ConnectionState.READY -> "Ready for Android Auto"
                ConnectionState.ERROR -> message ?: "Error"
            }
            
            // Update status indicator color
            val colorRes = when (state) {
                ConnectionState.READY -> android.R.color.holo_green_light
                ConnectionState.ERROR -> android.R.color.holo_red_light
                ConnectionState.DISCONNECTED -> android.R.color.darker_gray
                else -> android.R.color.holo_orange_light
            }
            statusIndicator.setBackgroundColor(ContextCompat.getColor(this@MainActivity, colorRes))
            
            // Update button text
            btnConnect.text = if (state == ConnectionState.READY) "Disconnect" else "Connect"
            
            // Update stats if connected
            if (state == ConnectionState.READY) {
                updateStats()
            }
        }
    }
    
    /**
     * Update statistics display
     */
    private fun updateStats() {
        wifiAutoService?.let { service ->
            try {
                val statsJson = service.getStats()
                binding.tvStats.text = formatStats(statsJson)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to get stats", e)
            }
        }
    }
    
    /**
     * Format stats JSON for display
     */
    private fun formatStats(json: String): String {
        return try {
            val obj = org.json.JSONObject(json)
            buildString {
                appendLine("Session ID: ${obj.optInt("session_id", 0)}")
                appendLine("Bytes Sent: ${formatBytes(obj.optLong("bytes_sent", 0))}")
                appendLine("Bytes Received: ${formatBytes(obj.optLong("bytes_received", 0))}")
            }
        } catch (e: Exception) {
            "Stats unavailable"
        }
    }
    
    /**
     * Format bytes to human-readable string
     */
    private fun formatBytes(bytes: Long): String {
        return when {
            bytes < 1024 -> "$bytes B"
            bytes < 1024 * 1024 -> "${bytes / 1024} KB"
            else -> "${bytes / (1024 * 1024)} MB"
        }
    }
}
