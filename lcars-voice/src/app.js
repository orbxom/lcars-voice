// LCARS Voice Interface - Tauri Integration

class LCARSVoiceInterface {
  constructor() {
    this.isRecording = false;
    this.isTranscribing = false;
    this.animationId = null;
    this.idleAnimationId = null;
    this.currentMode = 'VoiceNote';
    this.isPaused = false;
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
      pauseBtn: document.getElementById('pause-btn'),
      modeBtn: document.getElementById('mode-btn'),
      modeValue: document.getElementById('mode-value'),
      modeDropdown: document.getElementById('mode-dropdown'),
      modeOptions: document.querySelectorAll('.mode-option'),
      jiraInput: document.getElementById('jira-input'),
      markBtn: document.getElementById('mark-btn'),
      marksList: document.getElementById('marks-list'),
      meetingControls: document.getElementById('meeting-controls'),
    };

    this.waveformCtx = this.elements.waveform.getContext('2d');
    this.history = [];
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

    // Pause button
    this.elements.pauseBtn?.addEventListener('click', () => this.togglePause());

    // JIRA mark
    this.elements.markBtn?.addEventListener('click', () => this.addTimestampMark());
    this.elements.jiraInput?.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.addTimestampMark();
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
      this.updateUI('ready');
      this.startIdleWaveform();
      this.flashStatus('ERROR: ' + event.payload);
    });

    listen('meeting-saved', (event) => {
      console.log('[LCARS] event: Meeting saved to', event.payload);
      this.isRecording = false;
      this.isPaused = false;
      this.stopElapsedTimer();
      this.stopWaveformAnimation();
      this.elements.meetingControls.style.display = 'none';
      this.elements.pauseBtn.querySelector('.button-text').textContent = 'PAUSE';
      this.updateUI('ready');
      this.startIdleWaveform();
      this.flashStatus('MEETING SAVED');
    });

    listen('meeting-paused', () => {
      console.log('[LCARS] event: Meeting paused');
      this.isPaused = true;
      this.elements.pauseBtn.querySelector('.button-text').textContent = 'RESUME';
      this.elements.pauseBtn.classList.add('paused');
      this.elements.statusText.textContent = 'PAUSED';
      this.stopWaveformAnimation();
    });

    listen('meeting-resumed', () => {
      console.log('[LCARS] event: Meeting resumed');
      this.isPaused = false;
      this.elements.pauseBtn.querySelector('.button-text').textContent = 'PAUSE';
      this.elements.pauseBtn.classList.remove('paused');
      this.startWaveformAnimation();
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
    // Show/hide pause button based on mode
    if (this.currentMode === 'Meeting') {
      this.elements.pauseBtn.style.display = '';
    } else {
      this.elements.pauseBtn.style.display = 'none';
    }
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

  async togglePause() {
    if (!this.isRecording) return;
    try {
      if (this.isPaused) {
        await window.__TAURI__.core.invoke('resume_recording');
      } else {
        await window.__TAURI__.core.invoke('pause_recording');
      }
    } catch (e) {
      console.error('[LCARS] app: Pause/resume failed:', e);
      this.flashStatus('ERROR: ' + e);
    }
  }

  async addTimestampMark() {
    if (!this.isRecording || this.isPaused) return;
    const ticket = this.elements.jiraInput.value.trim() || null;
    try {
      await window.__TAURI__.core.invoke('add_timestamp_mark', { ticket, note: null });
      this.elements.jiraInput.value = '';
      await this.loadMarks();
    } catch (e) {
      console.error('[LCARS] app: Failed to add timestamp mark:', e);
    }
  }

  async loadMarks() {
    try {
      const marks = await window.__TAURI__.core.invoke('get_timestamp_marks');
      this.renderMarks(marks);
    } catch (e) {
      console.error('[LCARS] app: Failed to load marks:', e);
    }
  }

  renderMarks(marks) {
    this.elements.marksList.innerHTML = '';
    marks.forEach(mark => {
      const item = document.createElement('div');
      item.className = 'mark-item';

      const timeSpan = document.createElement('span');
      timeSpan.className = 'mark-time';
      timeSpan.textContent = mark.time;
      item.appendChild(timeSpan);

      const ticketSpan = document.createElement('span');
      ticketSpan.className = 'mark-ticket';
      ticketSpan.textContent = mark.ticket ? ` \u2192 ${mark.ticket}` : '';
      item.appendChild(ticketSpan);

      this.elements.marksList.appendChild(item);
    });
    this.elements.marksList.scrollTop = this.elements.marksList.scrollHeight;
  }

  startElapsedTimer() {
    this.stopElapsedTimer();
    this.timerInterval = setInterval(async () => {
      if (!this.isRecording || this.isPaused) return;
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
          this.elements.meetingControls.style.display = 'block';
          this.elements.pauseBtn.style.display = '';
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
        this.elements.meetingControls.style.display = 'none';
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

    let noiseData = new Array(64).fill(0);
    let levelHistory = new Array(64).fill(0);

    const draw = () => {
      if (!this.isRecording) return;
      this.animationId = requestAnimationFrame(draw);

      // Shift history left and add new level
      levelHistory.shift();

      // Get real audio level from backend
      window.__TAURI__.core.invoke('get_audio_level')
        .then(level => {
          const scaled = Math.min(80, level * 500);
          levelHistory.push(scaled);
        })
        .catch(() => {
          levelHistory.push(Math.random() * 50);
        });

      // Smooth the data
      for (let i = 0; i < noiseData.length; i++) {
        noiseData[i] += ((levelHistory[i] || 0) - noiseData[i]) * 0.3;
      }

      this.drawWaveform(noiseData);
    };

    draw();
  }

  stopWaveformAnimation() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  drawWaveform(dataArray) {
    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;

    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.fillRect(0, 0, width, height);

    ctx.lineWidth = 2;
    ctx.strokeStyle = '#FF9900';
    ctx.shadowBlur = 10;
    ctx.shadowColor = '#FF9900';

    ctx.beginPath();

    const sliceWidth = width / dataArray.length;
    let x = 0;

    for (let i = 0; i < dataArray.length; i++) {
      const v = dataArray[i] / 50;
      const y = (v * height) / 2;

      if (i === 0) {
        ctx.moveTo(x, height / 2 + y - height / 4);
      } else {
        ctx.lineTo(x, height / 2 + y - height / 4);
      }

      x += sliceWidth;
    }

    ctx.lineTo(width, height / 2);
    ctx.stroke();
    ctx.shadowBlur = 0;
  }

  startIdleWaveform() {
    // Cancel any existing idle animation first
    if (this.idleAnimationId) {
      cancelAnimationFrame(this.idleAnimationId);
      this.idleAnimationId = null;
    }

    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;

    let offset = 0;

    const drawIdle = () => {
      if (this.isRecording || this.isTranscribing) {
        this.idleAnimationId = null;
        return;
      }

      ctx.fillStyle = 'rgba(0, 0, 0, 0.1)';
      ctx.fillRect(0, 0, width, height);

      ctx.lineWidth = 1;
      ctx.strokeStyle = 'rgba(153, 153, 255, 0.5)';

      ctx.beginPath();

      for (let x = 0; x < width; x++) {
        const y = height / 2 + Math.sin((x + offset) * 0.05) * 5;
        if (x === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      }

      ctx.stroke();

      offset += 2;
      this.idleAnimationId = requestAnimationFrame(drawIdle);
    };

    // Clear canvas first
    ctx.fillStyle = 'rgba(0, 0, 0, 1)';
    ctx.fillRect(0, 0, width, height);

    drawIdle();
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
