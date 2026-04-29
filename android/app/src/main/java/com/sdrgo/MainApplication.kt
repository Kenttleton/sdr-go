package com.sdrgo

import android.app.Application
import android.content.res.Configuration

import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.common.ReleaseLevel
import com.facebook.react.defaults.DefaultNewArchitectureEntryPoint

import com.facebook.soloader.SoLoader
import expo.modules.ApplicationLifecycleDispatcher
import expo.modules.ExpoReactHostFactory

class MainApplication : Application(), ReactApplication {

  override val reactHost: ReactHost
    get() = ExpoReactHostFactory.getDefaultReactHost(
      applicationContext,
      PackageList(this).packages.apply { add(SdrPackage()) }
    )

  override fun onCreate() {
    super.onCreate()
    DefaultNewArchitectureEntryPoint.releaseLevel = try {
      ReleaseLevel.valueOf(BuildConfig.REACT_NATIVE_RELEASE_LEVEL.uppercase())
    } catch (e: IllegalArgumentException) {
      ReleaseLevel.STABLE
    }
    System.loadLibrary("sdr_core")
    loadReactNative(this)
    // SoLoader is now initialized. Explicitly load the app's C++ module (libsdrgo.so)
    // because DefaultSoLoader only looks for "appmodules" and silently ignores failures.
    // libsdrgo.so contains the OnLoad.cpp that installs JSI bindings (PlatformConstants, etc.).
    SoLoader.loadLibrary("sdrgo")
    ApplicationLifecycleDispatcher.onApplicationCreate(this)
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    ApplicationLifecycleDispatcher.onConfigurationChanged(this, newConfig)
  }
}
