// LCARS Voice Interface - Tauri Integration

class LCARSVoiceInterface {
  constructor() {
    this.isRecording = false;
    this.isTranscribing = false;
    this.animationId = null;
    this.idleAnimationId = null;

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
    this.elements.searchInput.addEventListener('input', (e) => this.filterHistory(e.target.value));

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
        } catch (e) {
          console.error('Failed to start dragging:', e);
        }
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
    });

    // Model option selection
    this.elements.modelOptions.forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        const model = opt.dataset.model;
        this.setWhisperModel(model);
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
    });

    listen('transcription-complete', async (event) => {
      console.log('[LCARS] event: Received transcription-complete');
      this.isTranscribing = false;
      const text = event.payload;
      console.log('[LCARS] event: Transcription text =', JSON.stringify(text));
      console.log('[LCARS] event: Transcription text length =', text ? text.length : 'null/undefined');

      // Copy to clipboard via Tauri
      try {
        console.log('[LCARS] app: Copying to clipboard, text =', JSON.stringify(text));
        console.log('[LCARS] app: window.__TAURI__ keys =', Object.keys(window.__TAURI__ || {}));

        // Try different clipboard API paths for Tauri 2.x
        let writeText;
        if (window.__TAURI__?.clipboard?.writeText) {
          console.log('[LCARS] app: Using window.__TAURI__.clipboard.writeText');
          writeText = window.__TAURI__.clipboard.writeText;
        } else if (window.__TAURI__?.clipboardManager?.writeText) {
          console.log('[LCARS] app: Using window.__TAURI__.clipboardManager.writeText');
          writeText = window.__TAURI__.clipboardManager.writeText;
        } else {
          console.error('[LCARS] app: No clipboard API found!');
          console.log('[LCARS] app: __TAURI__ structure:', JSON.stringify(Object.keys(window.__TAURI__ || {})));
          throw new Error('Clipboard API not found');
        }

        await writeText(text);
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
  }

  updateStardate() {
    // Fun stardate calculation (not canon-accurate, just decorative)
    const now = new Date();
    const year = now.getFullYear();
    const start = new Date(year, 0, 0);
    const diff = now - start;
    const oneDay = 1000 * 60 * 60 * 24;
    const dayOfYear = Math.floor(diff / oneDay);
    const stardate = ((year - 2000) * 1000 + dayOfYear + (now.getHours() / 24)).toFixed(1);
    this.elements.stardate.textContent = stardate;
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
        this.elements.statusText.textContent = 'RECORDING';
        this.elements.recordBtn.querySelector('.button-text').textContent = 'STOP';
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
        this.elements.recordBtn.querySelector('.button-text').textContent = 'RECORD';
        break;
    }
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

  // Simulated waveform animation during recording (no Web Audio API needed)
  startWaveformAnimation() {
    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;

    let offset = 0;
    let noiseData = new Array(64).fill(0).map(() => Math.random() * 50);

    const draw = () => {
      if (!this.isRecording) return;

      this.animationId = requestAnimationFrame(draw);

      // Update noise data
      for (let i = 0; i < noiseData.length; i++) {
        noiseData[i] += (Math.random() - 0.5) * 20;
        noiseData[i] = Math.max(10, Math.min(80, noiseData[i]));
      }

      // Draw waveform
      this.drawWaveform(noiseData);

      offset += 2;
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

    // Bind copy buttons
    this.elements.historyList.querySelectorAll('.history-item').forEach(item => {
      item.addEventListener('click', async (e) => {
        if (!e.target.classList.contains('copy-btn')) return;

        const text = item.dataset.text;
        try {
          const writeText = window.__TAURI__.clipboardManager?.writeText;
          if (!writeText) throw new Error('Clipboard API not available');
          await writeText(text);
          this.flashStatus('COPIED');
        } catch (e) {
          console.error('Failed to copy to clipboard:', e);
          this.flashStatus('ERROR: CLIPBOARD');
        }
      });
    });
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
