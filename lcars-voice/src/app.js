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
      audioLevel: document.getElementById('audio-level'),
      waveform: document.getElementById('waveform'),
      historyList: document.getElementById('history-list'),
      searchInput: document.getElementById('search-input'),
      stardate: document.getElementById('stardate'),
    };

    this.waveformCtx = this.elements.waveform.getContext('2d');
    this.history = [];

    this.init();
  }

  async init() {
    this.bindEvents();
    this.bindTauriEvents();
    this.updateStardate();
    await this.loadHistory();
    this.renderHistory();
    this.startIdleWaveform();

    // Update stardate every minute
    setInterval(() => this.updateStardate(), 60000);
  }

  bindEvents() {
    this.elements.recordBtn.addEventListener('click', () => this.toggleRecording());
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
        await appWindow.hide();
      } catch (e) {
        console.error('Failed to hide window:', e);
      }
    });
  }

  bindTauriEvents() {
    if (!window.__TAURI__?.event) {
      console.error('Tauri API not available');
      return;
    }
    const { listen } = window.__TAURI__.event;

    listen('recording-started', () => {
      this.isRecording = true;
      this.updateUI('recording');
      this.startWaveformAnimation();
    });

    listen('transcribing', () => {
      this.isRecording = false;
      this.isTranscribing = true;
      this.stopWaveformAnimation();
      this.updateUI('transcribing');
    });

    listen('transcription-complete', async (event) => {
      this.isTranscribing = false;
      const text = event.payload;

      // Copy to clipboard via Tauri
      try {
        const { writeText } = window.__TAURI__.clipboard;
        await writeText(text);
      } catch (e) {
        console.error('Failed to copy to clipboard:', e);
      }

      // Reload history from database
      await this.loadHistory();
      this.renderHistory();
      this.updateUI('ready');
      this.startIdleWaveform();
      this.flashStatus('COPIED TO CLIPBOARD');
    });

    listen('transcription-error', (event) => {
      this.isTranscribing = false;
      this.isRecording = false;
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
    if (this.isTranscribing) return;

    if (!window.__TAURI__?.core) {
      console.error('Tauri core API not available');
      this.flashStatus('ERROR: Tauri not available');
      return;
    }

    try {
      if (this.isRecording) {
        await window.__TAURI__.core.invoke('stop_recording');
      } else {
        await window.__TAURI__.core.invoke('start_recording');
      }
    } catch (e) {
      console.error('Recording toggle failed:', e);
      this.flashStatus('ERROR: ' + e);
    }
  }

  updateUI(state) {
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
        this.elements.audioLevel.style.width = '0%';
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

      // Calculate simulated audio level
      const average = noiseData.reduce((a, b) => a + b, 0) / noiseData.length;
      const level = Math.min(100, (average / 80) * 100);
      this.elements.audioLevel.style.width = `${level}%`;

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
          const { writeText } = window.__TAURI__.clipboard;
          await writeText(text);
          this.flashStatus('COPIED');
        } catch (e) {
          console.error('Failed to copy to clipboard:', e);
          this.flashStatus('ERROR: CLIPBOARD');
        }
      });
    });
  }
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  window.lcarsApp = new LCARSVoiceInterface();
});
