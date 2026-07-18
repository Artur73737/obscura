# Obscura Stealth — IDL File Map

Reference degli IDL Chromium 136.0.7103.114 in `stealth/ch/` per implementazione fingerprint in `bootstrap.js`.

## Core DOM

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `navigator.idl` | Navigator + mixin inclusions (NavigatorID, NavigatorUA, NavigatorLanguage, NavigatorOnLine, NavigatorConcurrentHardware, NavigatorCookies, NavigatorDeviceMemory, NavigatorAutomationInformation) | §1 Navigator Properties, §19/§51 Navigator checklist |
| `navigator_id.idl` | NavigatorID: vendor, vendorSub, productSub, appName, appCodeName, product, platform, userAgent, appVersion | §1 (vendor, platform, userAgent, appVersion) |
| `navigator_language.idl` | NavigatorLanguage: language, languages | §1 (language, languages) |
| `navigator_ua.idl` | NavigatorUA: userAgentData | §1 (userAgentData, getHighEntropyValues) |
| `window.idl` | Window.prototype: innerWidth, outerWidth, screenX, screenY, devicePixelRatio, locationbar, menubar, toolbar, scrollbars, statusbar, personalbar, chrome, trustedTypes, crossOriginIsolated, isSecureContext, opener, originAgentCluster, credentialless | §2 Window & Chrome, §3 Screen & Viewport, §50 Window misc, §52 Window.prototype checklist |
| `window_or_worker_global_scope.idl` | WindowOrWorkerGlobalScope: performance, fetch, structuredClone, queueMicrotask, createImageBitmap, crossOriginIsolated, isSecureContext, caches | §8 Timing & Performance |
| `bar_prop.idl` | BarProp: visible (locationbar, menubar, toolbar, scrollbars, statusbar, personalbar) | §50 (BarProp visible values) |
| `user_activation.idl` | UserActivation: hasBeenActive, isActive | §33 |

## Web Device APIs (7 gap-fill)

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `usb.idl` + `navigator_usb.idl` | WebUSB: USB, USBDevice, navigator.usb | §28 |
| `bluetooth.idl` + `navigator_bluetooth.idl` | Web Bluetooth: Bluetooth, BluetoothDevice, navigator.bluetooth | §30 |
| `hid.idl` + `navigator_hid.idl` | WebHID: HID, HIDDevice, navigator.hid | §29 |
| `serial.idl` | Web Serial: Serial, SerialPort, navigator.serial | §27 |
| `xr_system.idl` + `navigator_xr.idl` | WebXR: XRSystem, XRSession, navigator.xr | §37 |
| `media_session.idl` | MediaSession: metadata, playbackState, setActionHandler | §36 |
| `trusted_type_policy_factory.idl` + `trusted_html.idl` + `trusted_script.idl` | TrustedTypes: TrustedTypePolicyFactory, TrustedHTML, TrustedScript, TrustedScriptURL | §41 |

## Media & Graphics

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `webgl_rendering_context_base.h` (header C++) | ~300 parametri WebGL: MAX_TEXTURE_SIZE, MAX_VIEWPORT_DIMS, ALIASED_LINE_WIDTH_RANGE, UNMASKED_VENDOR/RENDERER_WEBGL, ecc. | §4 (WebGL parameters) |
| `webgl_extension.h` | Lista estensioni WebGL supportate (~60) | §4 (WebGL extensions) |
| `gpu.idl` / `gpu_adapter.idl` / `gpu_adapter_info.idl` / `gpu_device.idl` / `gpu_device_lost_info.idl` | WebGPU: adapter, info (vendor, architecture, device, description), limits, features | §18 |
| `media_devices.idl` | MediaDevices: enumerateDevices, getUserMedia, getDisplayMedia, getSupportedConstraints | §10, §45 |
| `media_capabilities.idl` | MediaCapabilities: decodingInfo, encodingInfo | §10, §38 |

## Security & Credentials

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `permissions.idl` | Permissions: query, request, revoke, requestAll + permission names | §13 |
| `credentials_container.idl` | CredentialsContainer: get, store, create, preventSilentAccess | §22 |
| `navigator_credentials.idl` | navigator.credentials property | §22 |
| `navigator_login.idl` | navigator.login (FedCM): setStatus | §40 |
| `crypto.idl` / `subtle_crypto.idl` | Web Crypto API | — |

## Storage & Workers

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `service_worker_container.idl` + `navigator_service_worker.idl` | ServiceWorkerContainer: controller, ready, register, getRegistration | §43 |
| `locks.idl` + `navigator_locks.idl` | Web Locks: LockManager, request, query | §26 |
| `wake_lock.idl` + `navigator_wake_lock.idl` | Screen Wake Lock: request, WakeLockSentinel | §31 |

## Input & Device APIs

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `gamepad.idl` | Gamepad, GamepadButton, GamepadEvent, navigator.getGamepads | §21 |
| `keyboard.idl` | Keyboard: getLayoutMap, lock, unlock | §24 |
| `virtual_keyboard.idl` + `navigator_virtual_keyboard.idl` | VirtualKeyboard: boundingRect, overlaysContent, show, hide | §34 |
| `input_device_capabilities.idl` | InputDeviceCapabilities: firesTouchEvents | §1 (maxTouchPoints context) |
| `clipboard.idl` + `navigator_clipboard.idl` | Async Clipboard: read, readText, write, writeText, ClipboardItem | §32 |
| `device_posture.idl` + `navigator_device_posture.idl` | DevicePosture: type, onchange | §40 |

## PWA & UI

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `window_controls_overlay.idl` + `navigator_window_controls_overlay.idl` | WindowControlsOverlay: visible, getTitlebarAreaRect | §35 |

## Audio & CSS

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `webaudio_audio_context.idl` + `webaudio_offline_audio_context.idl` | AudioContext, OfflineAudioContext: createOscillator, createDynamicsCompressor, startRendering | §5 (AudioContext fingerprint) |
| `css.idl` | CSS: supports(), CSS.supports() | §11 |

## Feature Flags

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `runtime_enabled_features.json5` | Feature flag esatti Chrome 136: quali API sono on/off per default (WebUSB, WebBluetooth, WebHID, WebSerial, WebXR, FedCM, FileSystemAccess, WakeLock, ecc.) | Tutte le sezioni che hanno `RuntimeEnabled=` |

## Policy

| File | Cosa definisce | Sezioni fingerprint |
|---|---|---|
| `document_policy_features.json5` | Document/feature policy names supportati | §41 (featurePolicy) |

---

## Struttura file IDL

Ogni file IDL definisce interfacce WebIDL con:
- `readonly attribute` → JS accessor `{ get, set: undefined, enumerable: true, configurable: true }`
- `attribute` (read/write) → JS accessor `{ get, set, enumerable: true, configurable: true }`
- `[SameObject]` → sempre lo stesso oggetto (reference identity)
- `[SecureContext]` → disponibile solo su HTTPS/localhost
- `[Exposed=Window]` → disponibile solo in Window (non Worker)
- `[RuntimeEnabled=...]` → dietro feature flag
- `[Measure]` / `[HighEntropy=Direct]` → metriche use counter
