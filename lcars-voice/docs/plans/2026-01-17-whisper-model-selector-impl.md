# Whisper Model Selector - TDD Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add UI control to change Whisper transcription model from the footer bar.

**Architecture:** Tauri Store plugin for persistence, Rust commands for get/set, frontend dropdown in footer.

**Tech Stack:** Tauri v2, tauri-plugin-store, Rust, vanilla JavaScript, CSS

---

## Task 1: Add Store Plugin Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`

**Step 1: Add tauri-plugin-store to Cargo.toml**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-store = "2"
```

**Step 2: Add store permission to capabilities**

In `src-tauri/capabilities/default.json`, add to permissions array:

```json
"store:allow-get",
"store:allow-set",
"store:allow-save",
"store:allow-load"
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors (plugin not registered yet, but deps resolve)

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/capabilities/default.json
git commit -m "chore: add tauri-plugin-store dependency"
```

---

## Task 2: Write Failing Test for get_whisper_model Command

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Write the failing test**

Add at the bottom of `src-tauri/src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_whisper_models() {
        // Valid models should be accepted
        assert!(is_valid_whisper_model("base"));
        assert!(is_valid_whisper_model("small"));
        assert!(is_valid_whisper_model("medium"));
        assert!(is_valid_whisper_model("large"));
    }

    #[test]
    fn test_invalid_whisper_models() {
        // Invalid models should be rejected
        assert!(!is_valid_whisper_model("tiny"));
        assert!(!is_valid_whisper_model("xlarge"));
        assert!(!is_valid_whisper_model(""));
        assert!(!is_valid_whisper_model("BASE")); // case sensitive
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test`
Expected: FAIL with "cannot find function `is_valid_whisper_model`"

**Step 3: Commit failing test**

```bash
git add src-tauri/src/main.rs
git commit -m "test: add failing tests for whisper model validation"
```

---

## Task 3: Make Validation Test Pass

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Write minimal implementation**

Add before `AppState` struct in `src-tauri/src/main.rs`:

```rust
const VALID_WHISPER_MODELS: &[&str] = &["base", "small", "medium", "large"];

fn is_valid_whisper_model(model: &str) -> bool {
    VALID_WHISPER_MODELS.contains(&model)
}
```

**Step 2: Run test to verify it passes**

Run: `cd src-tauri && cargo test`
Expected: PASS - all tests pass

**Step 3: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: add whisper model validation function"
```

---

## Task 4: Register Store Plugin in main.rs

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Add store plugin import**

Add to imports at top of `src-tauri/src/main.rs`:

```rust
use tauri_plugin_store::StoreExt;
```

**Step 2: Register plugin in builder**

In the `tauri::Builder::default()` chain, add after `.plugin(tauri_plugin_clipboard_manager::init())`:

```rust
.plugin(tauri_plugin_store::Builder::default().build())
```

**Step 3: Verify it compiles and tests still pass**

Run: `cd src-tauri && cargo test && cargo check`
Expected: All tests pass, compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: register tauri store plugin"
```

---

## Task 5: Write Failing Test for Default Model Behavior

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/main.rs`:

```rust
#[test]
fn test_default_whisper_model() {
    assert_eq!(get_default_whisper_model(), "base");
}

#[test]
fn test_model_fallback_chain() {
    // When no store value and no env var, should return "base"
    let model = resolve_whisper_model(None, None);
    assert_eq!(model, "base");

    // When store has value, use it
    let model = resolve_whisper_model(Some("medium".to_string()), None);
    assert_eq!(model, "medium");

    // When store is empty but env var set, use env var
    let model = resolve_whisper_model(None, Some("large".to_string()));
    assert_eq!(model, "large");

    // Store takes precedence over env var
    let model = resolve_whisper_model(Some("small".to_string()), Some("large".to_string()));
    assert_eq!(model, "small");
}
```

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test`
Expected: FAIL with "cannot find function"

**Step 3: Commit failing test**

```bash
git add src-tauri/src/main.rs
git commit -m "test: add failing tests for model resolution"
```

---

## Task 6: Make Model Resolution Tests Pass

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Write minimal implementation**

Add after `is_valid_whisper_model` function:

```rust
fn get_default_whisper_model() -> &'static str {
    "base"
}

fn resolve_whisper_model(store_value: Option<String>, env_value: Option<String>) -> String {
    store_value
        .or(env_value)
        .unwrap_or_else(|| get_default_whisper_model().to_string())
}
```

**Step 2: Run test to verify it passes**

Run: `cd src-tauri && cargo test`
Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: add model resolution with fallback chain"
```

---

## Task 7: Add get_whisper_model Tauri Command

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Add the command**

Add after `transcribe_audio` command:

```rust
#[tauri::command]
async fn get_whisper_model(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let store_value = store.get("whisper_model").and_then(|v| v.as_str().map(String::from));
    let env_value = std::env::var("WHISPER_MODEL").ok();
    Ok(resolve_whisper_model(store_value, env_value))
}
```

**Step 2: Register command in invoke_handler**

Add `get_whisper_model` to the `tauri::generate_handler!` macro:

```rust
.invoke_handler(tauri::generate_handler![
    get_history,
    search_history,
    add_transcription,
    start_recording,
    stop_recording,
    transcribe_audio,
    get_whisper_model
])
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: add get_whisper_model command"
```

---

## Task 8: Add set_whisper_model Tauri Command

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Add the command**

Add after `get_whisper_model` command:

```rust
#[tauri::command]
async fn set_whisper_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    if !is_valid_whisper_model(&model) {
        return Err(format!("Invalid model: {}. Valid options: {:?}", model, VALID_WHISPER_MODELS));
    }
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("whisper_model", serde_json::json!(model));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 2: Register command in invoke_handler**

Add `set_whisper_model` to the `tauri::generate_handler!` macro:

```rust
.invoke_handler(tauri::generate_handler![
    get_history,
    search_history,
    add_transcription,
    start_recording,
    stop_recording,
    transcribe_audio,
    get_whisper_model,
    set_whisper_model
])
```

**Step 3: Verify it compiles and tests pass**

Run: `cd src-tauri && cargo test && cargo check`
Expected: All tests pass, compiles

**Step 4: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: add set_whisper_model command with validation"
```

---

## Task 9: Update Transcription to Use Store

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Create helper function to get current model**

Add after `set_whisper_model` command:

```rust
fn get_current_model(app: &tauri::AppHandle) -> String {
    let store_value = app.store("settings.json")
        .ok()
        .and_then(|s| s.get("whisper_model"))
        .and_then(|v| v.as_str().map(String::from));
    let env_value = std::env::var("WHISPER_MODEL").ok();
    resolve_whisper_model(store_value, env_value)
}
```

**Step 2: Update stop_recording to use dynamic model**

In `stop_recording` function, replace:
```rust
let model = state.model.clone();
```

With:
```rust
let model = get_current_model(&app);
```

Do this for both occurrences in the function.

**Step 3: Update hotkey handler to use dynamic model**

In the hotkey handler thread (around line 206), replace:
```rust
&state.model,
```

With:
```rust
&get_current_model(&app_clone),
```

**Step 4: Update toggle file handler similarly**

In the toggle file watcher thread, replace `&state.model` with `&get_current_model(&app_clone)`.

**Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles without errors

**Step 6: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat: use store for whisper model in transcription"
```

---

## Task 10: Remove Static Model from AppState

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Remove model field from AppState**

Change AppState struct from:
```rust
struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    venv_path: PathBuf,
    model: String,
}
```

To:
```rust
struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    venv_path: PathBuf,
}
```

**Step 2: Remove model initialization in main()**

Remove these lines:
```rust
let model = std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string());
println!("[LCARS] main: Using whisper model = {}", model);
```

And remove `model,` from the AppState initialization.

**Step 3: Update add_transcription calls**

Find all calls to `db.add_transcription` and update them to get model dynamically. For example:
```rust
let _ = db.add_transcription(&text, None, &get_current_model(&app_clone));
```

**Step 4: Verify tests pass and it compiles**

Run: `cd src-tauri && cargo test && cargo check`
Expected: All pass

**Step 5: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "refactor: remove static model from AppState, use store"
```

---

## Task 11: Add Footer HTML Structure

**Files:**
- Modify: `src/index.html`

**Step 1: Remove the elbow-bottom-right SVG**

In `src/index.html`, remove this SVG element from `.lcars-right`:
```html
<svg class="elbow-bottom-right" viewBox="0 0 24 90" preserveAspectRatio="none">
  <path d="M0,0 L24,0 L24,68 A22,22 0 0,0 2,90 L0,90 Z" fill="currentColor"/>
</svg>
```

**Step 2: Replace footer structure**

Replace the entire `.lcars-footer` div:
```html
<div class="lcars-footer">
  <div class="footer-bar">
    <span class="footer-info">LCARS v2.47</span>
    <span class="footer-sep">│</span>
    <span class="footer-info">WHISPER: BASE</span>
  </div>
</div>
```

With:
```html
<div class="lcars-footer">
  <div class="footer-segment version">
    <span class="footer-info">LCARS v2.47</span>
  </div>
  <div class="footer-gap"></div>
  <div class="footer-segment model-selector" id="model-selector">
    <button class="model-btn" id="model-btn">
      <span class="model-label">WHISPER:</span>
      <span class="model-value" id="model-value">BASE</span>
      <span class="model-arrow">▾</span>
    </button>
    <div class="model-dropdown" id="model-dropdown">
      <button class="model-option" data-model="base">BASE</button>
      <button class="model-option" data-model="small">SMALL</button>
      <button class="model-option" data-model="medium">MEDIUM</button>
      <button class="model-option" data-model="large">LARGE</button>
    </div>
  </div>
  <div class="footer-gap"></div>
</div>
```

**Step 3: Verify page loads**

Run: `cd src-tauri && cargo tauri dev`
Expected: App launches, footer shows new structure (unstyled)

**Step 4: Commit**

```bash
git add src/index.html
git commit -m "feat: add whisper model selector HTML structure"
```

---

## Task 12: Add Footer CSS - Base Layout

**Files:**
- Modify: `src/styles.css`

**Step 1: Replace footer styles**

Find the `FOOTER BAR` section in `src/styles.css` and replace entirely:

```css
/* ============================================
   FOOTER BAR
   ============================================ */
.lcars-footer {
  height: var(--footer-height);
  display: flex;
  align-items: stretch;
  margin-left: 62px;
  margin-right: var(--accent-width);
  margin-top: calc(-1 * var(--footer-height));
  gap: 0;
}

.footer-segment {
  background: var(--lcars-tan);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 16px;
}

.footer-segment.version {
  border-radius: 0 0 0 20px;
}

.footer-gap {
  width: var(--gap);
  background: var(--lcars-black);
}

.footer-info {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  color: var(--lcars-black);
}
```

**Step 2: Verify styling**

Run: `cd src-tauri && cargo tauri dev`
Expected: Footer shows two tan segments with black gaps between

**Step 3: Commit**

```bash
git add src/styles.css
git commit -m "style: add footer segment layout with gaps"
```

---

## Task 13: Add Model Selector Button CSS

**Files:**
- Modify: `src/styles.css`

**Step 1: Add model selector styles**

Add after the footer segment styles:

```css
/* Model Selector */
.model-selector {
  position: relative;
  padding: 0;
}

.model-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 16px;
  height: 100%;
  border: none;
  background: var(--lcars-tan);
  color: var(--lcars-black);
  font-family: var(--font-display);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  cursor: pointer;
  transition: background 0.15s ease;
}

.model-btn:hover {
  background: var(--lcars-tan-light);
}

.model-btn.active {
  background: var(--lcars-orange);
}

.model-label {
  opacity: 0.7;
}

.model-value {
  font-weight: 700;
}

.model-arrow {
  font-size: 10px;
  transition: transform 0.15s ease;
}

.model-btn.active .model-arrow {
  transform: rotate(180deg);
}
```

**Step 2: Verify button styling**

Run: `cd src-tauri && cargo tauri dev`
Expected: Model button displays correctly with hover state

**Step 3: Commit**

```bash
git add src/styles.css
git commit -m "style: add model selector button styles"
```

---

## Task 14: Add Dropdown CSS

**Files:**
- Modify: `src/styles.css`

**Step 1: Add dropdown styles**

Add after model button styles:

```css
/* Model Dropdown */
.model-dropdown {
  position: absolute;
  bottom: 100%;
  right: 0;
  background: var(--lcars-black);
  border: 2px solid var(--lcars-tan);
  border-radius: 12px 12px 0 0;
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 120px;
  opacity: 0;
  visibility: hidden;
  transform: translateY(10px);
  transition: all 0.15s ease;
}

.model-dropdown.open {
  opacity: 1;
  visibility: visible;
  transform: translateY(0);
}

.model-option {
  padding: 10px 16px;
  border: none;
  border-radius: 8px;
  background: var(--lcars-tan);
  color: var(--lcars-black);
  font-family: var(--font-display);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 2px;
  cursor: pointer;
  transition: all 0.1s ease;
  text-align: left;
}

.model-option:hover {
  background: var(--lcars-orange);
}

.model-option.selected {
  background: var(--lcars-orange);
  box-shadow: inset 0 0 0 2px var(--lcars-orange-light);
}
```

**Step 2: Verify dropdown styling (manually add .open class to test)**

Run: `cd src-tauri && cargo tauri dev`
Open dev tools, add `open` class to `.model-dropdown` manually.
Expected: Dropdown appears with animation, options styled correctly

**Step 3: Commit**

```bash
git add src/styles.css
git commit -m "style: add model dropdown styles with animation"
```

---

## Task 15: Remove Bottom-Right Elbow CSS

**Files:**
- Modify: `src/styles.css`

**Step 1: Remove elbow-bottom-right styles**

Delete these lines from `src/styles.css`:

```css
.elbow-bottom-right {
  width: 24px; /* Match accent strip width */
  height: 90px; /* Combines bottom strip + footer area */
  color: var(--lcars-blue);
  display: block;
  flex-shrink: 0;
}
```

**Step 2: Verify no visual regression**

Run: `cd src-tauri && cargo tauri dev`
Expected: App looks correct, no broken styles

**Step 3: Commit**

```bash
git add src/styles.css
git commit -m "style: remove unused elbow-bottom-right styles"
```

---

## Task 16: Add Model Selector JavaScript - Load Current Model

**Files:**
- Modify: `src/app.js`

**Step 1: Add model elements to constructor**

In the `elements` object in constructor, add:

```javascript
modelBtn: document.getElementById('model-btn'),
modelValue: document.getElementById('model-value'),
modelDropdown: document.getElementById('model-dropdown'),
modelOptions: document.querySelectorAll('.model-option'),
```

**Step 2: Add currentModel property**

After `this.history = [];` add:

```javascript
this.currentModel = 'base';
this.dropdownOpen = false;
```

**Step 3: Add loadCurrentModel method**

Add after `renderHistory()` method:

```javascript
async loadCurrentModel() {
  try {
    const model = await window.__TAURI__.core.invoke('get_whisper_model');
    this.currentModel = model;
    this.updateModelDisplay();
    console.log('[LCARS] app: Loaded whisper model =', model);
  } catch (e) {
    console.error('[LCARS] app: Failed to load whisper model:', e);
  }
}

updateModelDisplay() {
  this.elements.modelValue.textContent = this.currentModel.toUpperCase();
  this.elements.modelOptions.forEach(opt => {
    opt.classList.toggle('selected', opt.dataset.model === this.currentModel);
  });
}
```

**Step 4: Call loadCurrentModel in init()**

In the `init()` method, after `this.renderHistory();` add:

```javascript
await this.loadCurrentModel();
```

**Step 5: Verify model loads**

Run: `cd src-tauri && cargo tauri dev`
Expected: Console shows "[LCARS] app: Loaded whisper model = base"

**Step 6: Commit**

```bash
git add src/app.js
git commit -m "feat: load current whisper model on init"
```

---

## Task 17: Add Dropdown Toggle JavaScript

**Files:**
- Modify: `src/app.js`

**Step 1: Add dropdown toggle methods**

Add after `updateModelDisplay()`:

```javascript
toggleDropdown() {
  this.dropdownOpen = !this.dropdownOpen;
  this.elements.modelDropdown.classList.toggle('open', this.dropdownOpen);
  this.elements.modelBtn.classList.toggle('active', this.dropdownOpen);
}

closeDropdown() {
  this.dropdownOpen = false;
  this.elements.modelDropdown.classList.remove('open');
  this.elements.modelBtn.classList.remove('active');
}
```

**Step 2: Bind dropdown events in bindEvents()**

Add at the end of `bindEvents()`:

```javascript
// Model selector dropdown
this.elements.modelBtn?.addEventListener('click', (e) => {
  e.stopPropagation();
  this.toggleDropdown();
});

// Close dropdown when clicking outside
document.addEventListener('click', (e) => {
  if (!e.target.closest('.model-selector')) {
    this.closeDropdown();
  }
});
```

**Step 3: Verify toggle works**

Run: `cd src-tauri && cargo tauri dev`
Click model button - dropdown opens. Click outside - closes.
Expected: Smooth animation, arrow rotates

**Step 4: Commit**

```bash
git add src/app.js
git commit -m "feat: add dropdown toggle functionality"
```

---

## Task 18: Add Model Selection JavaScript

**Files:**
- Modify: `src/app.js`

**Step 1: Add setWhisperModel method**

Add after `closeDropdown()`:

```javascript
async setWhisperModel(model) {
  try {
    await window.__TAURI__.core.invoke('set_whisper_model', { model });
    this.currentModel = model;
    this.updateModelDisplay();
    this.closeDropdown();
    this.flashStatus('MODEL: ' + model.toUpperCase());
    console.log('[LCARS] app: Set whisper model =', model);
  } catch (e) {
    console.error('[LCARS] app: Failed to set whisper model:', e);
    this.flashStatus('ERROR: ' + e);
  }
}
```

**Step 2: Bind option click events**

Add to `bindEvents()` after the dropdown toggle code:

```javascript
// Model option selection
this.elements.modelOptions.forEach(opt => {
  opt.addEventListener('click', (e) => {
    e.stopPropagation();
    const model = opt.dataset.model;
    this.setWhisperModel(model);
  });
});
```

**Step 3: Verify model selection works**

Run: `cd src-tauri && cargo tauri dev`
1. Click model button
2. Select "MEDIUM"
Expected: Dropdown closes, button shows "WHISPER: MEDIUM", status flashes "MODEL: MEDIUM"

**Step 4: Commit**

```bash
git add src/app.js
git commit -m "feat: add model selection with persistence"
```

---

## Task 19: Manual Integration Test

**Files:** None (testing only)

**Step 1: Full workflow test**

Run: `cd src-tauri && cargo tauri dev`

Test checklist:
- [ ] App launches with footer showing "WHISPER: BASE"
- [ ] Click model button - dropdown opens upward
- [ ] Select "LARGE" - dropdown closes, shows "WHISPER: LARGE"
- [ ] Close app and reopen - still shows "WHISPER: LARGE" (persistence)
- [ ] Record and transcribe - verify console shows model used
- [ ] Ctrl+Shift+H hotkey still works

**Step 2: Verify test passes**

All checklist items should work correctly.

**Step 3: Commit test evidence**

No commit needed - manual test

---

## Task 20: Final Cleanup and Production Build

**Files:**
- Modify: `src-tauri/src/main.rs` (if needed)

**Step 1: Remove any debug print statements if desired**

Optional: Keep or remove `println!` statements as preferred.

**Step 2: Run full test suite**

Run: `cd src-tauri && cargo test`
Expected: All tests pass

**Step 3: Production build**

Run: `cd src-tauri && cargo tauri build`
Expected: Builds successfully, creates .deb and .AppImage

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: whisper model selector complete

- Add footer dropdown to select whisper model (base/small/medium/large)
- Persist selection using Tauri store plugin
- Dynamic model resolution with fallback chain
- LCARS-styled dropdown with upward expansion"
```

---

## Summary

**Total Tasks:** 20
**Estimated Commits:** 18

**Test Coverage:**
- Rust unit tests: Model validation, fallback resolution
- Manual integration: Full UI workflow

**Key Files Modified:**
- `src-tauri/Cargo.toml` - Store plugin dependency
- `src-tauri/capabilities/default.json` - Store permissions
- `src-tauri/src/main.rs` - Commands, validation, store integration
- `src/index.html` - Footer structure
- `src/styles.css` - Footer and dropdown styles
- `src/app.js` - Model selector logic
