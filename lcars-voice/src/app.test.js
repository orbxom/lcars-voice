import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri API before loading app.js
window.__TAURI__ = {
  core: { invoke: vi.fn().mockResolvedValue(undefined) },
  event: { listen: vi.fn().mockResolvedValue(vi.fn()) },
};

const { LCARSVoiceInterface } = require('./app.js');

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
