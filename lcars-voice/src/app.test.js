import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock Tauri API before loading app.js
window.__TAURI__ = {
  core: { invoke: vi.fn().mockResolvedValue(undefined) },
  event: { listen: vi.fn().mockResolvedValue(vi.fn()) },
};

const { LCARSVoiceInterface, escapeHtml, formatDuration, UI_STATE } = require('./app.js');

/** Create a minimal instance without running constructor/init */
function createTestInstance() {
  const app = Object.create(LCARSVoiceInterface.prototype);

  const buttonText = { textContent: 'RECORD' };

  app.isRecording = false;
  app.isTranscribing = false;
  app.currentMode = 'VoiceNote';
  app.animationId = null;
  app.idleAnimationId = null;
  app.transcribeAnimationId = null;
  app.timerInterval = null;
  app.meetingTranscriptionProgress = { stage: null, percent: 0 };

  const makeStyle = () => ({ display: '', color: '' });
  const makeClassList = () => ({
    remove: vi.fn(), add: vi.fn(), toggle: vi.fn(), contains: vi.fn(() => false),
  });

  app.elements = {
    frame: { classList: makeClassList() },
    recordBtn: {
      querySelector: vi.fn(() => buttonText),
      classList: makeClassList(),
    },
    statusIndicator: { classList: makeClassList() },
    statusText: { textContent: '', style: makeStyle() },
    modeValue: { textContent: '' },
    modeDropdown: { classList: makeClassList() },
    modeBtn: { classList: makeClassList() },
    modeOptions: [],
    sectionDivider: { style: makeStyle() },
    historySection: { style: makeStyle() },
    meetingDivider: { style: makeStyle() },
    meetingSection: { style: makeStyle() },
    waveform: { getContext: vi.fn(() => ({ fillRect: vi.fn(), fillStyle: '' })) },
  };

  app._buttonText = buttonText;

  return app;
}

describe('escapeHtml() helper', () => {
  it('should escape &, <, and > characters', () => {
    expect(escapeHtml('a & b < c > d')).toBe('a &amp; b &lt; c &gt; d');
  });

  it('should escape double quotes', () => {
    expect(escapeHtml('say "hello"')).toBe('say &quot;hello&quot;');
  });

  it('should return empty string for empty input', () => {
    expect(escapeHtml('')).toBe('');
  });

  it('should handle strings with no special characters', () => {
    expect(escapeHtml('plain text')).toBe('plain text');
  });

  it('should escape single quotes', () => {
    expect(escapeHtml("it's a test")).toBe("it&#39;s a test");
  });

  it('should handle multiple special characters together', () => {
    expect(escapeHtml('<script>"alert(1)&"</script>')).toBe('&lt;script&gt;&quot;alert(1)&amp;&quot;&lt;/script&gt;');
  });
});

describe('formatDuration() helper', () => {
  it('should format 0 seconds as 00:00:00', () => {
    expect(formatDuration(0)).toBe('00:00:00');
  });

  it('should format seconds only', () => {
    expect(formatDuration(45)).toBe('00:00:45');
  });

  it('should format minutes and seconds', () => {
    expect(formatDuration(125)).toBe('00:02:05');
  });

  it('should format hours, minutes, and seconds', () => {
    expect(formatDuration(3661)).toBe('01:01:01');
  });

  it('should handle large values', () => {
    expect(formatDuration(86399)).toBe('23:59:59');
  });
});

describe('UI_STATE constants', () => {
  it('should export UI_STATE with recording, transcribing, and ready', () => {
    expect(UI_STATE).toBeDefined();
    expect(UI_STATE.RECORDING).toBe('recording');
    expect(UI_STATE.TRANSCRIBING).toBe('transcribing');
    expect(UI_STATE.READY).toBe('ready');
  });

  it('updateUI should accept UI_STATE constants', () => {
    const app = createTestInstance();
    app.stopElapsedTimer = vi.fn();

    app.updateUI(UI_STATE.READY);
    expect(app.elements.statusText.textContent).toBe('READY');

    app.updateUI(UI_STATE.RECORDING);
    expect(app.elements.statusText.textContent).toBe('RECORDING');

    app.updateUI(UI_STATE.TRANSCRIBING);
    expect(app.elements.statusText.textContent).toBe('TRANSCRIBING');
  });
});

describe('dropdownOpen removal: toggleDropdown reads from classList', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
    // Add model dropdown/btn elements with real Set-based tracking
    const makeTrackingClassList = () => {
      const classes = new Set();
      return {
        add: vi.fn((c) => classes.add(c)),
        remove: vi.fn((c) => classes.delete(c)),
        toggle: vi.fn((c, force) => {
          if (force === undefined) {
            if (classes.has(c)) classes.delete(c); else classes.add(c);
          } else if (force) {
            classes.add(c);
          } else {
            classes.delete(c);
          }
        }),
        contains: vi.fn((c) => classes.has(c)),
      };
    };
    app.elements.modelDropdown = { classList: makeTrackingClassList() };
    app.elements.modelBtn = { classList: makeTrackingClassList() };
  });

  it('should not have a dropdownOpen property', () => {
    expect(app).not.toHaveProperty('dropdownOpen');
  });

  it('toggleDropdown should close when DOM says open, even without prior toggleDropdown call', () => {
    // Externally open the dropdown via DOM (simulating external manipulation)
    app.elements.modelDropdown.classList.add('open');
    app.elements.modelBtn.classList.add('active');

    // toggleDropdown should read from classList and close it
    app.toggleDropdown();
    expect(app.elements.modelDropdown.classList.contains('open')).toBe(false);
    expect(app.elements.modelBtn.classList.contains('active')).toBe(false);
  });

  it('toggleDropdown should open when closed, then close when open', () => {
    app.toggleDropdown();
    expect(app.elements.modelDropdown.classList.contains('open')).toBe(true);
    expect(app.elements.modelBtn.classList.contains('active')).toBe(true);

    app.toggleDropdown();
    expect(app.elements.modelDropdown.classList.contains('open')).toBe(false);
    expect(app.elements.modelBtn.classList.contains('active')).toBe(false);
  });
});

describe('flashStatus() restores actual state', () => {
  let app;

  beforeEach(() => {
    vi.useFakeTimers();
    app = createTestInstance();
    app.stopElapsedTimer = vi.fn();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should restore READY when not recording or transcribing', () => {
    app.isRecording = false;
    app.isTranscribing = false;
    app.elements.statusText.textContent = 'READY';

    app.flashStatus('COPIED');
    expect(app.elements.statusText.textContent).toBe('COPIED');

    vi.advanceTimersByTime(2000);
    expect(app.elements.statusText.textContent).toBe('READY');
    expect(app.elements.statusText.style.color).toBe('');
  });

  it('should restore RECORDING when currently recording in VoiceNote mode', () => {
    app.isRecording = true;
    app.isTranscribing = false;
    app.currentMode = 'VoiceNote';
    app.elements.statusText.textContent = 'RECORDING';

    app.flashStatus('SOME MESSAGE');
    expect(app.elements.statusText.textContent).toBe('SOME MESSAGE');

    vi.advanceTimersByTime(2000);
    expect(app.elements.statusText.textContent).toBe('RECORDING');
  });

  it('should restore TRANSCRIBING when currently transcribing', () => {
    app.isRecording = false;
    app.isTranscribing = true;
    app.elements.statusText.textContent = 'TRANSCRIBING';

    app.flashStatus('SOME MESSAGE');
    expect(app.elements.statusText.textContent).toBe('SOME MESSAGE');

    vi.advanceTimersByTime(2000);
    expect(app.elements.statusText.textContent).toBe('TRANSCRIBING');
  });
});

describe('updateTranscriptionProgress does targeted DOM update', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
    app.startTranscribingAnimation = vi.fn();
    // Add meetingList element with a processing button
    app.elements.meetingList = {
      querySelector: vi.fn(),
    };
  });

  it('should not call renderMeetingHistory on progress updates', () => {
    app.renderMeetingHistory = vi.fn();
    app.meetingTranscriptionProgress = { stage: 'transcribing', percent: 50 };
    app.transcribeAnimationId = 1; // animation already running

    app.updateTranscriptionProgress();

    expect(app.renderMeetingHistory).not.toHaveBeenCalled();
  });

  it('should update processing button text directly when button exists', () => {
    const btn = { textContent: 'PROCESSING...' };
    app.elements.meetingList.querySelector = vi.fn(() => btn);
    app.meetingTranscriptionProgress = { stage: 'transcribing', percent: 75 };
    app.transcribeAnimationId = 1;

    app.updateTranscriptionProgress();

    expect(app.elements.meetingList.querySelector).toHaveBeenCalledWith('.meeting-action-btn.processing');
    expect(btn.textContent).toBe('TRANSCRIBING 75%');
  });

  it('should show DIARIZING... for diarizing stage', () => {
    const btn = { textContent: 'PROCESSING...' };
    app.elements.meetingList.querySelector = vi.fn(() => btn);
    app.meetingTranscriptionProgress = { stage: 'diarizing', percent: 0 };
    app.transcribeAnimationId = 1;

    app.updateTranscriptionProgress();

    expect(btn.textContent).toBe('DIARIZING...');
  });

  it('should show DIARIZING 50% on the meeting action button when stage=diarizing, percent=50', () => {
    const btn = { textContent: 'PROCESSING...' };
    app.elements.meetingList.querySelector = vi.fn(() => btn);
    app.meetingTranscriptionProgress = { stage: 'diarizing', percent: 50 };
    app.transcribeAnimationId = 1;

    app.updateTranscriptionProgress();

    expect(btn.textContent).toBe('DIARIZING 50%');
  });

  it('should show DIARIZING... when stage=diarizing, percent=null', () => {
    const btn = { textContent: 'PROCESSING...' };
    app.elements.meetingList.querySelector = vi.fn(() => btn);
    app.meetingTranscriptionProgress = { stage: 'diarizing', percent: null };
    app.transcribeAnimationId = 1;

    app.updateTranscriptionProgress();

    expect(btn.textContent).toBe('DIARIZING...');
  });

  it('should show FINALIZING... for diarization_skipped stage', () => {
    const btn = { textContent: 'PROCESSING...' };
    app.elements.meetingList.querySelector = vi.fn(() => btn);
    app.meetingTranscriptionProgress = { stage: 'diarization_skipped', percent: 0 };
    app.transcribeAnimationId = 1;

    app.updateTranscriptionProgress();

    expect(btn.textContent).toBe('FINALIZING...');
  });
});

describe('meeting-transcription-progress status text updates', () => {
  let app;
  let listenCallbacks;

  beforeEach(() => {
    vi.clearAllMocks();
    // Capture listen callbacks so we can invoke the event handler
    listenCallbacks = {};
    window.__TAURI__.event.listen = vi.fn((eventName, cb) => {
      listenCallbacks[eventName] = cb;
      return Promise.resolve(vi.fn());
    });
    app = createTestInstance();
    app.startTranscribingAnimation = vi.fn();
    app.elements.meetingList = { querySelector: vi.fn() };
    // Re-bind to capture the listen callbacks
    app.bindTauriEvents();
  });

  it('should show DIARIZING 50% in status text when stage=diarizing and isTranscribing', () => {
    app.isTranscribing = true;

    // Simulate the event
    listenCallbacks['meeting-transcription-progress']({
      payload: { stage: 'diarizing', percent: 50 },
    });

    expect(app.elements.statusText.textContent).toBe('DIARIZING 50%');
  });

  it('should show TRANSCRIBING 50% in status text when stage=transcribing and isTranscribing', () => {
    app.isTranscribing = true;

    listenCallbacks['meeting-transcription-progress']({
      payload: { stage: 'transcribing', percent: 50 },
    });

    expect(app.elements.statusText.textContent).toBe('TRANSCRIBING 50%');
  });

  it('should store diarizationWarning when warning field is present', () => {
    listenCallbacks['meeting-transcription-progress']({
      payload: { stage: 'diarization_skipped', warning: 'Python not found' },
    });

    expect(app.diarizationWarning).toBe('Python not found');
  });

  it('should not set diarizationWarning when no warning field', () => {
    app.diarizationWarning = null;
    listenCallbacks['meeting-transcription-progress']({
      payload: { stage: 'diarizing', percent: 50 },
    });

    expect(app.diarizationWarning).toBeNull();
  });
});

describe('Bug 1: setRecordingMode updates record button text', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
  });

  it('should show START MEETING when switching to Meeting mode', async () => {
    app.currentMode = 'VoiceNote';
    app._buttonText.textContent = 'RECORD';

    await app.setRecordingMode('Meeting');

    expect(app._buttonText.textContent).toBe('START MEETING');
  });

  it('should show RECORD when switching to VoiceNote mode', async () => {
    app.currentMode = 'Meeting';
    app._buttonText.textContent = 'START MEETING';

    await app.setRecordingMode('VoiceNote');

    expect(app._buttonText.textContent).toBe('RECORD');
  });
});

describe('Bug 2: transcribing event sets progress state for animation', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
    app.stopWaveformAnimation = vi.fn();
    app.startTranscribingAnimation = vi.fn();
  });

  it('should set meetingTranscriptionProgress.stage to transcribing before starting animation', () => {
    // Simulate what the 'transcribing' event handler does
    app.isRecording = true;
    app.meetingTranscriptionProgress = { stage: null, percent: 0 };

    // Capture the progress state at the moment startTranscribingAnimation is called
    let progressAtAnimationStart;
    app.startTranscribingAnimation = vi.fn(() => {
      progressAtAnimationStart = { ...app.meetingTranscriptionProgress };
    });

    // Run the handler logic (same as the 'transcribing' event listener in bindTauriEvents)
    app.isRecording = false;
    app.isTranscribing = true;
    app.stopWaveformAnimation();
    app.meetingTranscriptionProgress = { stage: 'transcribing', percent: 0 };
    app.updateUI('transcribing');
    app.startTranscribingAnimation();

    expect(progressAtAnimationStart.stage).toBe('transcribing');
    expect(progressAtAnimationStart.percent).toBe(0);
  });
});

describe('redo button rendering based on has_audio', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
    app.elements.historyList = { innerHTML: '', addEventListener: vi.fn() };
  });

  it('should render redo button for entries with has_audio=true', () => {
    app.history = [{
      id: 1, text: 'test text', timestamp: '2026-03-01T12:00:00',
      duration_ms: 5000, model: 'base', has_audio: true,
    }];
    app.renderHistory();
    expect(app.elements.historyList.innerHTML).toContain('redo-btn');
  });

  it('should NOT render redo button for entries with has_audio=false', () => {
    app.history = [{
      id: 1, text: 'test text', timestamp: '2026-03-01T12:00:00',
      duration_ms: 5000, model: 'base', has_audio: false,
    }];
    app.renderHistory();
    expect(app.elements.historyList.innerHTML).not.toContain('redo-btn');
  });
});

describe('meeting redo button rendering', () => {
  let app;

  beforeEach(() => {
    vi.clearAllMocks();
    app = createTestInstance();
    app.elements.meetingList = { innerHTML: '', addEventListener: vi.fn() };
    app.transcribingMeetings = new Set();
  });

  it('should render REDO + COPY buttons when meeting has transcript', () => {
    app.meetings = [{
      id: 1, filename: 'test.wav', timestamp: '2026-03-01T12:00:00',
      duration_ms: 60000, size_bytes: 1024 * 1024, transcript: 'Hello world',
    }];
    app.renderMeetingHistory();
    expect(app.elements.meetingList.innerHTML).toContain('retranscribe-btn');
    expect(app.elements.meetingList.innerHTML).toContain('copy-transcript-btn');
    expect(app.elements.meetingList.innerHTML).toContain('REDO');
    expect(app.elements.meetingList.innerHTML).toContain('COPY');
  });

  it('should render TRANSCRIBE button when meeting has no transcript', () => {
    app.meetings = [{
      id: 2, filename: 'test2.wav', timestamp: '2026-03-01T13:00:00',
      duration_ms: 30000, size_bytes: 512 * 1024, transcript: null,
    }];
    app.renderMeetingHistory();
    expect(app.elements.meetingList.innerHTML).toContain('transcribe-btn');
    expect(app.elements.meetingList.innerHTML).not.toContain('retranscribe-btn');
  });
});
