// LCARS Voice Interface - Interactive Prototype

class LCARSVoiceInterface {
  constructor() {
    this.isRecording = false;
    this.isTranscribing = false;
    this.audioContext = null;
    this.analyser = null;
    this.mediaStream = null;
    this.animationId = null;

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
    this.history = this.loadHistory();

    this.init();
  }

  init() {
    this.bindEvents();
    this.updateStardate();
    this.renderHistory();
    this.startIdleWaveform();

    // Update stardate every minute
    setInterval(() => this.updateStardate(), 60000);
  }

  bindEvents() {
    this.elements.recordBtn.addEventListener('click', () => this.toggleRecording());
    this.elements.searchInput.addEventListener('input', (e) => this.filterHistory(e.target.value));

    // Window controls
    document.querySelector('.control-btn.minimize')?.addEventListener('click', () => {
      // In Tauri, this would minimize the window
      console.log('Minimize window');
    });

    document.querySelector('.control-btn.close')?.addEventListener('click', () => {
      // In Tauri, this would hide to tray
      console.log('Close to tray');
    });

    // Keyboard shortcut simulation (in real app, handled by Tauri)
    document.addEventListener('keydown', (e) => {
      if (e.key === 'h' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        this.toggleRecording();
      }
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

    if (this.isRecording) {
      await this.stopRecording();
    } else {
      await this.startRecording();
    }
  }

  async startRecording() {
    try {
      this.mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      this.audioContext = new AudioContext();
      this.analyser = this.audioContext.createAnalyser();
      this.analyser.fftSize = 256;

      const source = this.audioContext.createMediaStreamSource(this.mediaStream);
      source.connect(this.analyser);

      this.isRecording = true;
      this.updateUI('recording');
      this.startWaveformAnimation();

    } catch (err) {
      console.error('Failed to start recording:', err);
      this.setStatus('ERROR: NO MIC', 'error');
    }
  }

  async stopRecording() {
    this.isRecording = false;

    // Stop media stream
    if (this.mediaStream) {
      this.mediaStream.getTracks().forEach(track => track.stop());
      this.mediaStream = null;
    }

    // Stop animation
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }

    this.updateUI('transcribing');

    // Simulate transcription (in real app, this calls Whisper)
    await this.simulateTranscription();
  }

  async simulateTranscription() {
    this.isTranscribing = true;

    // Simulate processing time
    await new Promise(resolve => setTimeout(resolve, 1500));

    // Fake transcription result
    const sampleTexts = [
      "Computer, begin recording captain's log supplemental",
      "The anomaly appears to be some kind of temporal distortion",
      "Set a course for the nearest starbase, warp factor six",
      "Run a level three diagnostic on all primary systems",
      "Tea, Earl Grey, hot",
    ];

    const text = sampleTexts[Math.floor(Math.random() * sampleTexts.length)];

    // Add to history
    this.addToHistory(text);

    // Copy to clipboard
    await navigator.clipboard.writeText(text);

    this.isTranscribing = false;
    this.updateUI('ready');
    this.startIdleWaveform();

    // Flash success
    this.flashStatus('COPIED TO CLIPBOARD');
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
    this.elements.statusText.style.color = 'var(--lcars-green)';

    setTimeout(() => {
      this.elements.statusText.textContent = 'READY';
      this.elements.statusText.style.color = '';
    }, 2000);
  }

  startWaveformAnimation() {
    const bufferLength = this.analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);

    const draw = () => {
      if (!this.isRecording) return;

      this.animationId = requestAnimationFrame(draw);
      this.analyser.getByteFrequencyData(dataArray);

      // Calculate audio level
      const average = dataArray.reduce((a, b) => a + b, 0) / bufferLength;
      const level = Math.min(100, (average / 128) * 100);
      this.elements.audioLevel.style.width = `${level}%`;

      // Draw waveform
      this.drawWaveform(dataArray);
    };

    draw();
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
      const v = dataArray[i] / 128.0;
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
    const canvas = this.elements.waveform;
    const ctx = this.waveformCtx;
    const width = canvas.width;
    const height = canvas.height;

    let offset = 0;

    const drawIdle = () => {
      if (this.isRecording) return;

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
      requestAnimationFrame(drawIdle);
    };

    // Clear canvas first
    ctx.fillStyle = 'rgba(0, 0, 0, 1)';
    ctx.fillRect(0, 0, width, height);

    drawIdle();
  }

  // History management
  loadHistory() {
    try {
      const stored = localStorage.getItem('lcars-voice-history');
      return stored ? JSON.parse(stored) : [];
    } catch {
      return [];
    }
  }

  saveHistory() {
    localStorage.setItem('lcars-voice-history', JSON.stringify(this.history));
  }

  addToHistory(text) {
    const entry = {
      id: Date.now(),
      text,
      timestamp: new Date().toISOString(),
    };

    this.history.unshift(entry);

    // Keep only last 100 entries
    if (this.history.length > 100) {
      this.history = this.history.slice(0, 100);
    }

    this.saveHistory();
    this.renderHistory();
  }

  renderHistory(filter = '') {
    const filtered = filter
      ? this.history.filter(h => h.text.toLowerCase().includes(filter.toLowerCase()))
      : this.history;

    this.elements.historyList.innerHTML = filtered.map(entry => {
      const time = new Date(entry.timestamp).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      });

      const truncated = entry.text.length > 50
        ? entry.text.substring(0, 50) + '...'
        : entry.text;

      return `
        <div class="history-item" data-id="${entry.id}" data-text="${entry.text.replace(/"/g, '&quot;')}">
          <div class="item-content">
            <span class="item-text">${truncated}</span>
            <span class="item-time">${time}</span>
          </div>
          <button class="copy-btn" title="Copy to clipboard">⧉</button>
        </div>
      `;
    }).join('');

    // Bind copy buttons
    this.elements.historyList.querySelectorAll('.history-item').forEach(item => {
      item.addEventListener('click', async (e) => {
        if (!e.target.classList.contains('copy-btn')) return;

        const text = item.dataset.text;
        await navigator.clipboard.writeText(text);
        this.flashStatus('COPIED');
      });
    });
  }

  filterHistory(query) {
    this.renderHistory(query);
  }
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  window.lcarsApp = new LCARSVoiceInterface();
});
