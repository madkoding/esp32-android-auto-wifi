/**
 * BootReceiver - Starts WifiAutoService on device boot
 */
package com.androidauto.wifi.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.ContextCompat
import com.androidauto.wifi.service.WifiAutoService

class BootReceiver : BroadcastReceiver() {
    
    companion object {
        private const val TAG = "BootReceiver"
    }
    
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            Log.i(TAG, "Boot completed, starting WifiAutoService")
            
            Intent(context, WifiAutoService::class.java).also { serviceIntent ->
                ContextCompat.startForegroundService(context, serviceIntent)
            }
        }
    }
}
