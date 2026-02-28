// LCARS Voice Interface - Tauri Integration

class LCARSVoiceInterface {
  constructor() {
    this.isRecording = false;
    this.isTranscribing = false;
    this.animationId = null;
    this.idleAnimationId = null;
    this.transcribeAnimationId = null;
    this.currentMode = 'VoiceNote';
    this.timerInterval = null;

    this.elements = {
      frame: document.querySelector('.lcars-frame'),
      recordBtn: document.getElementById('record-btn'),
      statusIndicator: document.getElementById('status-indicator'),
      statusText: document.querySelector('.status-text'),
      waveform: document.getElementById('waveform'),
      historyList: document.getElementById('history-list'),
      searchInput: document.getElementById('search-input'),
      stardate: document.getElementById('stardate'),
      modelBtn: document.getElementById('model-btn'),
      modelValue: document.getElementById('model-value'),
      modelDropdown: document.getElementById('model-dropdown'),
      modelOptions: document.querySelectorAll('.model-option'),
      appVersion: document.getElementById('app-version'),
      modeBtn: document.getElementById('mode-btn'),
      modeValue: document.getElementById('mode-value'),
      modeDropdown: document.getElementById('mode-dropdown'),
      modeOptions: document.querySelectorAll('.mode-option'),
      sectionDivider: document.querySelector('.section-divider'),
      historySection: document.querySelector('.history-section'),
      meetingDivider: document.querySelector('.meeting-divider'),
      meetingSection: document.querySelector('.meeting-section'),
      meetingList: document.getElementById('meeting-list'),
    };

    this.waveformCtx = this.elements.waveform.getContext('2d');
    this.history = [];
    this.meetings = [];
    this.transcribingMeetings = new Set();
    this.currentModel = 'base';
    this.dropdownOpen = false;

    this.init();
  }

  async init() {
    console.log('[LCARS] app: Initializing LCARSVoiceInterface');
    this.bindEvents();
    this.bindTauriEvents();
    this.updateStardate();
    await this.loadHistory();
    this.renderHistory();
    await this.loadMeetingHistory();
    this.renderMeetingHistory();
    await this.loadCurrentModel();
    await this.loadCurrentMode();
    await this.loadAppVersion();
    this.startIdleWaveform();

    // Update stardate every minute
    setInterval(() => this.updateStardate(), 60000);
    console.log('[LCARS] app: Initialization complete');
  }

  bindEvents() {
    console.log('[LCARS] app: Binding UI events');
    this.elements.recordBtn.addEventListener('click', () => {
      console.log('[LCARS] app: Record button clicked');
      this.toggleRecording();
    });
    let searchTimeout;
    this.elements.searchInput.addEventListener('input', (e) => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => this.filterHistory(e.target.value), 200);
    });

    // Window controls for Tauri
    document.querySelector('.control-btn.minimize')?.addEventListener('click', async () => {
      try {
        const appWindow = window.__TAURI__.window.getCurrentWindow();
        await appWindow.minimize();
      } catch (e) {
        console.error('Failed to minimize window:', e);
      }
    });

    document.querySelector('.control-btn.close')?.addEventListener('click', async () => {
      try {
        const appWindow = window.__TAURI__.window.getCurrentWindow();
        await appWindow.close();
      } catch (e) {
        console.error('Failed to close window:', e);
      }
    });

    // Window dragging - manual implementation for the header area
    document.querySelector('.lcars-header')?.addEventListener('mousedown', async (e) => {
      // Don't drag if clicking on buttons
      if (e.target.closest('.control-btn') || e.target.closest('button')) {
        return;
      }
      if (e.buttons === 1) {
        try {
          const appWindow = window.__TAURI__.window.getCurrentWindow();
          await appWindow.startDragging();
        } catch (err) {
          console.error('Failed to start dragging:', err);
        }
      }
    });

    // Event delegation for history list copy buttons
    this.elements.historyList.addEventListener('click', async (e) => {
      const copyBtn = e.target.closest('.copy-btn');
      if (!copyBtn) return;
      const historyItem = copyBtn.closest('.history-item');
      if (!historyItem) return;
      const text = historyItem.dataset.text;
      try {
        await this.copyToClipboard(text);
        this.flashStatus('COPIED');
      } catch (err) {
        console.error('Failed to copy to clipboard:', err);
        this.flashStatus('ERROR: CLIPBOARD');
      }
    });

    // Meeting list action buttons (transcribe / copy)
    this.elements.meetingList.addEventListener('click', async (e) => {
      const transcribeBtn = e.target.closest('.transcribe-btn');
      if (transcribeBtn) {
        const meetingItem = transcribeBtn.closest('.meeting-item');
        if (meetingItem) {
          const id = parseInt(meetingItem.dataset.id, 10);
          this.transcribeMeeting(id);
        }
        return;
      }
      const copyBtn = e.target.closest('.copy-transcript-btn');
      if (copyBtn) {
        const meetingItem = copyBtn.closest('.meeting-item');
        if (meetingItem) {
          const id = parseInt(meetingItem.dataset.id, 10);
          const meeting = this.meetings.find(m => m.id === id);
          if (meeting && meeting.transcript) {
            try {
              await this.copyToClipboard(meeting.transcript);
              this.flashStatus('TRANSCRIPT COPIED');
            } catch (err) {
              console.error('[LCARS] app: Failed to copy transcript:', err);
              this.flashStatus('ERROR: CLIPBOARD');
            }
          }
        }
        return;
      }
    });

    // Meeting list double-click to rename
    this.elements.meetingList.addEventListener('dblclick', (e) => {
      const itemText = e.target.closest('.item-text');
      if (!itemText) return;
      const meetingItem = itemText.closest('.meeting-item');
      if (!meetingItem) return;
      const id = parseInt(meetingItem.dataset.id, 10);
      this.startRenaming(id, itemText);
    });

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
      if (!e.target.closest('.mode-selector')) {
        this.closeModeDropdown();
      }
    });

    // Model option selection
    this.elements.modelOptions.forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        const model = opt.dataset.model;
        this.setWhisperModel(model);
      });
    });

    // Mode selector
    this.elements.modeBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      this.toggleModeDropdown();
    });

    this.elements.modeOptions.forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        this.setRecordingMode(opt.dataset.mode);
      });
    });

  }

  bindTauriEvents() {
    console.log('[LCARS] app: Binding Tauri events');
    if (!window.__TAURI__?.event) {
      console.error('[LCARS] app: Tauri API not available');
      return;
    }
    const { listen } = window.__TAURI__.event;

    listen('recording-started', () => {
      console.log('[LCARS] event: Received recording-started');
      this.isRecording = true;
      console.log('[LCARS] state: isRecording = true');
      this.updateUI('recording');
      this.startWaveformAnimation();
    });

    listen('transcribing', () => {
      console.log('[LCARS] event: Received transcribing');
      this.isRecording = false;
      this.isTranscribing = true;
      console.log('[LCARS] state: isRecording = false, isTranscribing = true');
      this.stopWaveformAnimation();
      this.updateUI('transcribing');
      this.startTranscribingAnimation();
    });

    listen('transcription-complete', async (event) => {
      console.log('[LCARS] event: Received transcription-complete');
      this.isTranscribing = false;
      const text = event.payload;
      console.log('[LCARS] event: Transcription text =', JSON.stringify(text));
      console.log('[LCARS] event: Transcription text length =', text ? text.length : 'null/undefined');

      try {
        await this.copyToClipboard(text);
        console.log('[LCARS] app: Copied to clipboard successfully');
      } catch (e) {
        console.error('[LCARS] app: Failed to copy to clipboard:', e);
      }

      // Reload history from database
      await this.loadHistory();
      this.renderHistory();
      this.stopTranscribingAnimation();
      this.updateUI('ready');
      this.startIdleWaveform();
      this.flashStatus('COPIED TO CLIPBOARD');
    });

    listen('transcription-error', (event) => {
      console.log('[LCARS] event: Received transcription-error');
      console.log('[LCARS] event: Error payload =', event.payload);
      this.isTranscribing = false;
      this.isRecording = false;
      console.log('[LCARS] state: isRecording = false, isTranscribing = false');
      this.stopWaveformAnimation();
      this.stopTranscribingAnimation();
      this.updateUI('ready');
      this.startIdleWaveform();
      this.flashStatus('ERROR: ' + event.payload);
    });

    listen('meeting-saved', async (event) => {
      console.log('[LCARS] event: Meeting saved:', event.payload);
      this.isRecording = false;
      this.stopElapsedTimer();
      this.stopWaveformAnimation();
      this.stopTranscribingAnimation();
      this.updateUI('ready');
      this.startIdleWaveform();
      await this.loadMeetingHistory();
      this.renderMeetingHistory();
      this.flashStatus('MEETING SAVED');
    });

    listen('model-download-progress', (event) => {
      const { model, percent, bytes_downloaded, total_bytes } = event.payload;
      console.log(`[LCARS] event: Model download ${model} ${percent}%`);
      this.showDownloadProgress(model, percent);
    });
  }

  updateStardate() {
    const now = new Date();
    const hours = now.getHours();
    const minutes = now.getMinutes().toString().padStart(2, '0');
    const ampm = hours >= 12 ? 'PM' : 'AM';
    const displayHours = hours % 12 || 12;
    this.elements.stardate.textContent = `${displayHours}:${minutes} ${ampm}`;
  }

  async loadAppVersion() {
    try {
      const version = await window.__TAURI__.app.getVersion();
      this.elements.appVersion.textContent = version;
    } catch (err) {
      console.error('[LCARS] app: Failed to load app version:', err);
    }
  }

  async loadCurrentMode() {
    try {
      const mode = await window.__TAURI__.core.invoke('get_recording_mode');
      this.currentMode = mode;
      this.updateModeDisplay();
    } catch (e) {
      console.error('[LCARS] app: Failed to load recording mode:', e);
    }
  }

  updateModeDisplay() {
    const label = this.currentMode === 'Meeting' ? 'MEETING' : 'VOICE NOTE';
    this.elements.modeValue.textContent = label;
    this.elements.modeOptions.forEach(opt => {
      opt.classList.toggle('selected', opt.dataset.mode === this.currentMode);
    });
    const isMeeting = this.currentMode === 'Meeting';
    this.elements.sectionDivider.style.display = isMeeting ? 'none' : '';
    this.elements.historySection.style.display = isMeeting ? 'none' : '';
    this.elements.meetingDivider.style.display = isMeeting ? '' : 'none';
    this.elements.meetingSection.style.display = isMeeting ? '' : 'none';
  }

  toggleModeDropdown() {
    const dropdown = this.elements.modeDropdown;
    const btn = this.elements.modeBtn;
    const isOpen = dropdown.classList.contains('open');
    dropdown.classList.toggle('open', !isOpen);
    btn.classList.toggle('active', !isOpen);
  }

  closeModeDropdown() {
    this.elements.modeDropdown.classList.remove('open');
    this.elements.modeBtn.classList.remove('active');
  }

  async setRecordingMode(mode) {
    try {
      await window.__TAURI__.core.invoke('set_recording_mode', { mode });
      this.currentMode = mode;
      this.updateModeDisplay();
      this.closeModeDropdown();
      this.flashStatus('MODE: ' + (mode === 'Meeting' ? 'MEETING' : 'VOICE NOTE'));
    } catch (e) {
      console.error('[LCARS] app: Failed to set recording mode:', e);
      this.flashStatus('ERROR: ' + e);
    }
  }

  startElapsedTimer() {
    this.stopElapsedTimer();
    this.timerInterval = setInterval(async () => {
      if (!this.isRecording) return;
      try {
        const elapsed = await window.__TAURI__.core.invoke('get_elapsed_time');
        const total = Math.floor(elapsed);
        const h = Math.floor(total / 3600);
        const m = Math.floor((total % 3600) / 60);
        const s = total % 60;
        const timeStr = `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
        this.elements.statusText.textContent = timeStr;
      } catch (e) {}
    }, 1000);
  }

  stopElapsedTimer() {
    if (this.timerInterval) {
      clearInterval(this.timerInterval);
      this.timerInterval = null;
    }
  }

  async toggleRecording() {
    console.log('[LCARS] app: toggleRecording() called, isRecording =', this.isRecording, 'isTranscribing =', this.isTranscribing);
    if (this.isTranscribing) {
      console.log('[LCARS] app: Currently transcribing, ignoring toggle');
      return;
    }

    if (!window.__TAURI__?.core) {
      console.error('[LCARS] app: Tauri core API not available');
      this.flashStatus('ERROR: Tauri not available');
      return;
    }

    try {
      if (this.isRecording) {
        console.log('[LCARS] app: Invoking stop_recording command');
        await window.__TAURI__.core.invoke('stop_recording');
        console.log('[LCARS] app: stop_recording command completed');
      } else {
        console.log('[LCARS] app: Invoking start_recording command');
        await window.__TAURI__.core.invoke('start_recording');
        console.log('[LCARS] app: start_recording command completed');
      }
    } catch (e) {
      console.error('[LCARS] app: Recording toggle failed:', e);
      this.flashStatus('ERROR: ' + e);
    }
  }

  updateUI(state) {
    console.log('[LCARS] app: updateUI() called with state =', state);
    this.elements.frame.classList.remove('recording', 'transcribing');
    this.elements.statusIndicator.classList.remove('recording', 'transcribing');
    this.elements.recordBtn.classList.remove('recording');

    switch (state) {
      case 'recording':
        this.elements.frame.classList.add('recording');
        this.elements.statusIndicator.classList.add('recording');
        this.elements.recordBtn.classList.add('recording');
        if (this.currentMode === 'Meeting') {
          this.elements.statusText.textContent = '00:00:00';
          this.elements.recordBtn.querySelector('.button-text').textContent = 'STOP MEETING';
          this.startElapsedTimer();
        } else {
          this.elements.statusText.textContent = 'RECORDING';
          this.elements.recordBtn.querySelector('.button-text').textContent = 'STOP';
        }
        break;

      case 'transcribing':
        this.elements.frame.classList.add('transcribing');
        this.elements.statusIndicator.classList.add('transcribing');
        this.elements.statusText.textContent = 'TRANSCRIBING';
        this.elements.recordBtn.querySelector('.button-text').textContent = 'PROCESSING';
        break;

      case 'ready':
      default:
        this.elements.statusText.textContent = 'READY';
        if (this.currentMode === 'Meeting') {
          this.elements.recordBtn.querySelector('.button-text').textContent = 'START MEETING';
        } else {
          this.elements.recordBtn.querySelector('.button-text').textContent = 'RECORD';
        }
        this.stopElapsedTimer();
        break;
    }
  }

  async copyToClipboard(text) {
    await window.__TAURI__.clipboardManager.writeText(text);
  }

  flashStatus(message) {
    const originalText = this.elements.statusText.textContent;
    this.elements.statusText.textContent = message;
    this.elements.statusText.style.color = message.startsWith('ERROR') ? 'var(--lcars-red)' : 'var(--lcars-green)';

    setTimeout(() => {
      this.elements.statusText.textContent = 'READY';
      this.elements.statusText.style.color = '';
    }, 2000);
  }

  showDownloadProgress(model, percent) {
    const container = document.getElementById('download-progress');
    const modelName = document.getElementById('download-model-name');
    const fill = document.getElementById('progress-bar-fill');
    const percentLabel = document.getElementById('download-percent');

    container.style.display = 'block';
    modelName.textContent = model.toUpperCase();
    fill.style.width = `${percent}%`;
    percentLabel.textContent = `${percent}%`;

    if (percent >= 100) {
      setTimeout(() => {
        container.style.display = 'none';
      }, 1500);
    }
  }

  startWaveformAnimation() {
    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;

    // Smoothed display buffer — lerps toward real data each frame
    let smoothed = null;
    let pending = false;

    const draw = () => {
      if (!this.isRecording) return;
      this.animationId = requestAnimationFrame(draw);

      // Fetch real waveform samples from the Rust backend (non-blocking)
      if (!pending) {
        pending = true;
        window.__TAURI__.core.invoke('get_waveform_data')
          .then(data => {
            if (!smoothed) {
              smoothed = new Float32Array(data.length);
            }
            // Exponential smoothing toward incoming samples (high factor = responsive)
            for (let i = 0; i < data.length; i++) {
              smoothed[i] += (data[i] - smoothed[i]) * 0.55;
            }
            pending = false;
          })
          .catch(() => { pending = false; });
      }

      if (smoothed) {
        this.drawWaveform(smoothed);
      }
    };

    draw();
  }

  stopWaveformAnimation() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  drawWaveform(samples) {
    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;
    const mid = height / 2;

    // Fade previous frame for slight trail effect
    ctx.fillStyle = 'rgba(0, 0, 0, 0.35)';
    ctx.fillRect(0, 0, width, height);

    // Adaptive scaling: make the peak fill ~85% of half-height.
    // Mic samples are typically tiny (0.01–0.1 for speech), so we need
    // a large multiplier.  Cap gain to avoid amplifying silence/noise.
    let peak = 0;
    for (let i = 0; i < samples.length; i++) {
      const abs = Math.abs(samples[i]);
      if (abs > peak) peak = abs;
    }
    const scale = peak > 0.003
      ? Math.min((mid * 0.6) / peak, mid * 80)
      : mid * 0.3;

    // Draw filled waveform (top half mirror + bottom half)
    ctx.beginPath();
    const sliceWidth = width / (samples.length - 1);

    // Upper contour (positive)
    for (let i = 0; i < samples.length; i++) {
      const x = i * sliceWidth;
      const y = mid - samples[i] * scale;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    // Lower contour (mirrored) — walk back
    for (let i = samples.length - 1; i >= 0; i--) {
      const x = i * sliceWidth;
      const y = mid + samples[i] * scale;
      ctx.lineTo(x, y);
    }
    ctx.closePath();

    // Semi-transparent orange fill
    ctx.fillStyle = 'rgba(255, 153, 0, 0.25)';
    ctx.fill();

    // Bright orange stroke for the center waveform line
    ctx.lineWidth = 2;
    ctx.strokeStyle = '#FF9900';
    ctx.shadowBlur = 8;
    ctx.shadowColor = '#FF9900';

    ctx.beginPath();
    for (let i = 0; i < samples.length; i++) {
      const x = i * sliceWidth;
      const y = mid - samples[i] * scale;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.shadowBlur = 0;

    // Thin center-line reference
    ctx.strokeStyle = 'rgba(255, 153, 0, 0.15)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, mid);
    ctx.lineTo(width, mid);
    ctx.stroke();
  }

  startIdleWaveform() {
    if (this.idleAnimationId) {
      cancelAnimationFrame(this.idleAnimationId);
      this.idleAnimationId = null;
    }

    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;
    const mid = height / 2;

    let t = 0;

    // Wave definitions: [frequency, speed, amplitude, color, lineWidth]
    const waves = [
      { freq: 0.025, speed: 0.8, amp: 10, color: 'rgba(153, 153, 255, 0.5)',  lw: 1.5 }, // primary blue
      { freq: 0.045, speed: 1.3, amp:  6, color: 'rgba(204, 153, 204, 0.35)', lw: 1   }, // purple harmonic
      { freq: 0.08,  speed: 2.0, amp:  3, color: 'rgba(170, 170, 255, 0.2)',  lw: 0.5 }, // fast noise floor
    ];

    const drawIdle = () => {
      if (this.isRecording || this.isTranscribing) {
        this.idleAnimationId = null;
        return;
      }

      // Fade previous frame
      ctx.fillStyle = 'rgba(0, 0, 0, 0.12)';
      ctx.fillRect(0, 0, width, height);

      // Faint horizontal grid lines
      ctx.strokeStyle = 'rgba(153, 153, 255, 0.07)';
      ctx.lineWidth = 1;
      for (const frac of [0.25, 0.5, 0.75]) {
        const gy = height * frac;
        ctx.beginPath();
        ctx.moveTo(0, gy);
        ctx.lineTo(width, gy);
        ctx.stroke();
      }

      // Draw each wave layer
      for (const w of waves) {
        ctx.lineWidth = w.lw;
        ctx.strokeStyle = w.color;
        ctx.beginPath();
        for (let x = 0; x <= width; x++) {
          const y = mid + Math.sin(x * w.freq + t * w.speed * 0.02) * w.amp;
          if (x === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }

      // Traveling pulse along primary wave
      const pulseX = ((t * 1.2) % (width + 40)) - 20;
      const pulseY = mid + Math.sin(pulseX * waves[0].freq + t * waves[0].speed * 0.02) * waves[0].amp;
      const grad = ctx.createRadialGradient(pulseX, pulseY, 0, pulseX, pulseY, 14);
      grad.addColorStop(0, 'rgba(170, 170, 255, 0.6)');
      grad.addColorStop(0.4, 'rgba(153, 153, 255, 0.2)');
      grad.addColorStop(1, 'rgba(153, 153, 255, 0)');
      ctx.fillStyle = grad;
      ctx.fillRect(pulseX - 14, pulseY - 14, 28, 28);

      t++;
      this.idleAnimationId = requestAnimationFrame(drawIdle);
    };

    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, width, height);
    drawIdle();
  }

  startTranscribingAnimation() {
    this.stopTranscribingAnimation();

    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;
    const mid = height / 2;

    // Generate a fixed set of random bar heights so the pattern is stable
    const barCount = 48;
    const barWidth = width / barCount;
    const barHeights = Array.from({ length: barCount }, () => 0.15 + Math.random() * 0.85);

    let scanX = 0;
    const scanSpeed = 1.8; // pixels per frame

    // Clear to black before starting
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, width, height);

    const draw = () => {
      this.transcribeAnimationId = requestAnimationFrame(draw);

      // Fade everything toward black
      ctx.fillStyle = 'rgba(0, 0, 0, 0.08)';
      ctx.fillRect(0, 0, width, height);

      // Draw bars — brightest near the scan line, fading away behind it
      for (let i = 0; i < barCount; i++) {
        const bx = i * barWidth;
        const dist = scanX - (bx + barWidth / 2);
        // Bars light up as the scan passes over them, then fade
        let intensity;
        if (dist < 0) {
          // Ahead of scan — dim anticipation glow
          intensity = Math.max(0, 1 - Math.abs(dist) / 40) * 0.15;
        } else if (dist < 60) {
          // Just passed — bright
          intensity = 1 - dist / 60;
        } else {
          intensity = 0;
        }

        if (intensity > 0.01) {
          const h = barHeights[i] * mid * 0.85 * intensity;
          const alpha = intensity * 0.6;
          ctx.fillStyle = `rgba(255, 153, 0, ${alpha})`;
          ctx.fillRect(bx + 1, mid - h, barWidth - 2, h * 2);
        }
      }

      // Draw scan line
      const grad = ctx.createLinearGradient(scanX - 6, 0, scanX + 6, 0);
      grad.addColorStop(0, 'rgba(255, 153, 0, 0)');
      grad.addColorStop(0.5, 'rgba(255, 153, 0, 0.9)');
      grad.addColorStop(1, 'rgba(255, 153, 0, 0)');
      ctx.fillStyle = grad;
      ctx.fillRect(scanX - 6, 0, 12, height);

      // Advance scan, wrap around
      scanX += scanSpeed;
      if (scanX > width + 60) {
        scanX = -20;
        // Regenerate bar heights each sweep for variety
        for (let i = 0; i < barCount; i++) {
          barHeights[i] = 0.15 + Math.random() * 0.85;
        }
      }
    };

    draw();
  }

  stopTranscribingAnimation() {
    if (this.transcribeAnimationId) {
      cancelAnimationFrame(this.transcribeAnimationId);
      this.transcribeAnimationId = null;
    }
  }

  // History management via Tauri commands
  async loadHistory() {
    try {
      const history = await window.__TAURI__.core.invoke('get_history', { limit: 100 });
      this.history = history;
    } catch (e) {
      console.error('Failed to load history:', e);
      this.history = [];
    }
  }

  async filterHistory(query) {
    if (query) {
      try {
        this.history = await window.__TAURI__.core.invoke('search_history', { query, limit: 100 });
      } catch (e) {
        console.error('Failed to search history:', e);
      }
    } else {
      await this.loadHistory();
    }
    this.renderHistory();
  }

  renderHistory() {
    this.elements.historyList.innerHTML = this.history.map(entry => {
      const time = new Date(entry.timestamp).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });

      const truncated = entry.text.length > 50
        ? entry.text.substring(0, 50) + '...'
        : entry.text;

      // Escape HTML and quotes
      const escapedText = entry.text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');

      const escapedTruncated = truncated
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');

      return `
        <div class="history-item" data-id="${entry.id}" data-text="${escapedText}">
          <div class="item-content">
            <span class="item-text">${escapedTruncated}</span>
            <span class="item-time">${time}</span>
          </div>
          <button class="copy-btn" title="Copy to clipboard">&#x29C9;</button>
        </div>
      `;
    }).join('');
  }

  async loadMeetingHistory() {
    try {
      const meetings = await window.__TAURI__.core.invoke('get_meeting_history', { limit: 100 });
      this.meetings = meetings;
    } catch (e) {
      console.error('[LCARS] app: Failed to load meeting history:', e);
      this.meetings = [];
    }
  }

  renderMeetingHistory() {
    this.elements.meetingList.innerHTML = this.meetings.map(entry => {
      const time = new Date(entry.timestamp).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });

      const date = new Date(entry.timestamp).toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
      });

      const totalSec = Math.floor(entry.duration_ms / 1000);
      const h = Math.floor(totalSec / 3600);
      const m = Math.floor((totalSec % 3600) / 60);
      const s = totalSec % 60;
      const duration = `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;

      const sizeMB = (entry.size_bytes / (1024 * 1024)).toFixed(1);

      const escapedFilename = entry.filename
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');

      // Determine button state
      let actionBtn = '';
      const isTranscribing = this.transcribingMeetings.has(entry.id);
      if (isTranscribing) {
        actionBtn = '<button class="meeting-action-btn processing" disabled>PROCESSING...</button>';
      } else if (entry.transcript) {
        actionBtn = '<button class="meeting-action-btn copy-transcript-btn">COPY</button>';
      } else {
        actionBtn = '<button class="meeting-action-btn transcribe-btn">TRANSCRIBE</button>';
      }

      return `
        <div class="history-item meeting-item" data-id="${entry.id}">
          <div class="item-content">
            <span class="item-text">${escapedFilename}</span>
            <div style="display: flex; gap: 12px; align-items: center;">
              <span class="item-duration">${duration}</span>
              <span class="item-size">${sizeMB} MB</span>
              <span class="item-time">${date} ${time}</span>
              ${actionBtn}
            </div>
          </div>
        </div>
      `;
    }).join('');
  }

  async transcribeMeeting(id) {
    console.log('[LCARS] app: Transcribing meeting', id);
    this.transcribingMeetings.add(id);
    this.renderMeetingHistory();

    try {
      const transcript = await window.__TAURI__.core.invoke('transcribe_meeting', { id });
      // Update the local meeting object with the transcript
      const meeting = this.meetings.find(m => m.id === id);
      if (meeting) {
        meeting.transcript = transcript;
      }
      this.flashStatus('TRANSCRIPTION COMPLETE');
    } catch (e) {
      console.error('[LCARS] app: Meeting transcription failed:', e);
      this.flashStatus('ERROR: ' + e);
    } finally {
      this.transcribingMeetings.delete(id);
      this.renderMeetingHistory();
    }
  }

  startRenaming(id, spanElement) {
    const meeting = this.meetings.find(m => m.id === id);
    if (!meeting) return;

    const currentName = meeting.filename;
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'rename-input';
    input.value = currentName;
    spanElement.textContent = '';
    spanElement.appendChild(input);
    input.focus();

    // Select text before .wav extension
    const dotIndex = currentName.lastIndexOf('.');
    input.setSelectionRange(0, dotIndex > 0 ? dotIndex : currentName.length);

    let saved = false;
    const save = async () => {
      if (saved) return;
      saved = true;
      const newName = input.value.trim();
      if (newName && newName !== currentName) {
        try {
          await window.__TAURI__.core.invoke('rename_meeting', { id, newFilename: newName });
          meeting.filename = newName;
          this.flashStatus('RENAMED');
        } catch (e) {
          console.error('[LCARS] app: Rename failed:', e);
          this.flashStatus('ERROR: RENAME');
        }
      }
      this.renderMeetingHistory();
    };

    const cancel = () => {
      if (saved) return;
      saved = true;
      this.renderMeetingHistory();
    };

    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        save();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancel();
      }
    });
    input.addEventListener('blur', () => save());
  }

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

  async setWhisperModel(model) {
    try {
      const downloaded = await window.__TAURI__.core.invoke('is_model_downloaded', { model });
      if (!downloaded) {
        this.flashStatus('DOWNLOADING MODEL: ' + model.toUpperCase());
      }
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
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  window.lcarsApp = new LCARSVoiceInterface();
});
