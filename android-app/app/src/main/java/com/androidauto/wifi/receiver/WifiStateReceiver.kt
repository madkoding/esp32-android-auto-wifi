/**
 * WifiStateReceiver - Monitors WiFi state changes
 */
package com.androidauto.wifi.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.util.Log

class WifiStateReceiver : BroadcastReceiver() {
    
    companion object {
        private const val TAG = "WifiStateReceiver"
    }
    
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            WifiManager.WIFI_STATE_CHANGED_ACTION -> {
                val state = intent.getIntExtra(WifiManager.EXTRA_WIFI_STATE, WifiManager.WIFI_STATE_UNKNOWN)
                Log.d(TAG, "WiFi state changed: $state")
            }
            WifiManager.NETWORK_STATE_CHANGED_ACTION -> {
                Log.d(TAG, "Network state changed")
            }
        }
    }
}
