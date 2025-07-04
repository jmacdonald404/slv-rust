# v0.3.0-alpha

## Phase 3: Stability and Testing


### **TODO**
1. Core UI: Login Splash & Preferences
- [x] Implement a launch splash screen using egui (in ui/), with:
- [x] Username/password fields
- [x] Login button (disabled until fields are filled)
- [x] Preferences/settings button (opens modal)
- [x] Status area for login progress/errors
- [ ] Integrate basic preferences panel accessible from splash:
- [ ] Audio (volume, enable/disable)
- [ ] Graphics (API, vsync, render distance)
- [ ] Network (max bandwidth, timeout)
- [ ] Save/load preferences to config file
- [x] After successful login, transition to main app UI (stub for now)
2. Authentication & Connection
- [x] Implement login flow:
- [x] On login, send credentials to SecondLife login endpoint (grid_uri)
- [x] Parse login response, extract session/circuit info
- [x] Establish UDP connection to SL region server
- [x] Show progress/status updates in UI
- [x] If ToS is required (per login response), show ToS modal after connection but before world entry (stub)
- [x] Only show ToS if required by SL, not before connecting (stub)
- [x] Accept/decline ToS, handle accordingly (stub)
3. Preferences System
- [x] Extend preferences UI to allow editing all user/system settings
- [x] Ensure extended user preferences are only available when logged in
- [x] Persist preferences to disk and reload on startup
4. Clean Disconnect & Robustness
- [x] Ensure clean disconnect from SL server on:
- [x] Normal app exit
- [x] Panic/unexpected error
- [x] Network loss or forced disconnect
- [x] On disconnect, send proper logout message to SL server
- [x] Show disconnect reason/status in UI
- [x] Implement error handling for all network and UI operations
- [x] Manual logout from in-world UI
5. Testing & Build Flags
- [ ] Add debug/release build flags for logging, diagnostics, and error reporting
- [ ] Test login, disconnect, and reconnect flows
- [ ] Add unit/integration tests for networking and UI logic
6. In-World UI (stub)
- [x] Show in-world UI state after world entry
- [x] Stub chat panel
- [x] Stub inventory panel
- [x] Stub preferences panel
- [ ] Implement full in-world UI features (chat, inventory, preferences, scene, etc)