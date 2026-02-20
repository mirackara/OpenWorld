import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useSettingsStore } from '../stores/settingsStore';
import { MODEL_CATALOG, useModelStore, getRamCompatibility, type ModelCatalogEntry } from '../stores/modelStore';
import './SetupWizard.css';

type Step = 'welcome' | 'preparing' | 'pick-model' | 'downloading' | 'ready';

interface OllamaStatus {
    stage: string;
    message: string;
    progress: number | null;
}

export default function SetupWizard() {
    const [step, setStep] = useState<Step>('welcome');
    const [selectedModel, setSelectedModel] = useState<ModelCatalogEntry | null>(null);
    const [downloadProgress, setDownloadProgress] = useState(0);
    const [downloadStatus, setDownloadStatus] = useState('');
    const [prepareMessage, setPrepareMessage] = useState('Setting things up...');
    const [prepareProgress, setPrepareProgress] = useState<number | null>(null);
    const [prepareError, setPrepareError] = useState(false);
    const { systemRAM } = useModelStore();
    const { setSetupComplete, setDefaultModel } = useSettingsStore();

    async function startOllamaSetup() {
        setPrepareError(false);
        setPrepareMessage('Checking AI engine...');
        setPrepareProgress(null);
        setStep('preparing');

        const unlisten = await listen<OllamaStatus>('ollama-setup-status', (event) => {
            const { stage, message, progress } = event.payload;
            setPrepareMessage(message);
            setPrepareProgress(progress);

            if (stage === 'ready') {
                // Ollama confirmed ready — safe to pick a model
                setTimeout(() => setStep('pick-model'), 600);
            } else if (stage === 'error') {
                setPrepareError(true);
            }
        });

        try {
            await invoke('ensure_ollama');
            // If command succeeds but we haven't moved yet, move now
            // (edge case: ready event might not have fired yet)
            unlisten();
            // Double-check readiness and advance
            const running = await invoke<boolean>('check_ollama');
            if (running) {
                setStep('pick-model');
            }
        } catch (err) {
            console.error('Ollama setup failed:', err);
            setPrepareError(true);
            setPrepareMessage(String(err));
            unlisten();
        }
    }

    function handleGetStarted() {
        startOllamaSetup();
    }

    function handleRetry() {
        startOllamaSetup();
    }

    async function handleModelSelect(model: ModelCatalogEntry) {
        setSelectedModel(model);
    }

    async function handleDownload() {
        if (!selectedModel) return;
        setStep('downloading');
        setDownloadProgress(0);
        setDownloadStatus('Starting download...');

        const unlisten = await listen<{ status: string; total?: number; completed?: number }>(
            'model-pull-progress',
            (event) => {
                const { status, total, completed } = event.payload;
                setDownloadStatus(status);
                if (total && completed) {
                    setDownloadProgress(Math.round((completed / total) * 100));
                }
            }
        );

        try {
            await invoke('pull_model', { modelName: selectedModel.id });
            setStep('ready');
        } catch (err) {
            console.error('Download failed:', err);
            setDownloadStatus(`Download failed. Please check your internet connection and try again.`);
        } finally {
            unlisten();
        }
    }

    async function handleFinish() {
        if (selectedModel) {
            setDefaultModel(selectedModel.id);
        }
        setSetupComplete(true);
        try {
            await invoke('save_config_cmd', {
                cfg: {
                    theme: 'dark',
                    default_model: selectedModel?.id || 'llama3:8b',
                    setup_complete: true,
                    system_prompt: '',
                    ollama_host: 'http://localhost:11434',
                },
            });
        } catch (err) {
            console.error('Failed to save config:', err);
        }
        window.location.href = '/';
    }

    return (
        <div className="setup-container">
            <div className="setup-card slide-up">
                {step === 'welcome' && (
                    <div className="setup-step">
                        <div className="setup-logo">
                            <div className="logo-orb">
                                <span className="logo-icon">🌍</span>
                            </div>
                        </div>
                        <h1 className="setup-title">Welcome to OpenWorld</h1>
                        <p className="setup-subtitle">
                            Your AI assistant, running entirely on your machine.
                            <br />Private. Fast. Yours.
                        </p>
                        <button className="btn btn-primary btn-lg setup-cta" onClick={handleGetStarted}>
                            Let's get started
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <path d="M5 12h14M12 5l7 7-7 7" />
                            </svg>
                        </button>
                    </div>
                )}

                {step === 'preparing' && (
                    <div className="setup-step">
                        {!prepareError ? (
                            <>
                                <div className="download-animation">
                                    <div className="download-spinner" />
                                </div>
                                <h2 className="setup-step-title">Getting everything ready</h2>
                                <p className="setup-step-subtitle">{prepareMessage}</p>
                                {prepareProgress !== null && (
                                    <div className="progress-bar" style={{ maxWidth: 400, margin: '0 auto' }}>
                                        <div className="progress-fill" style={{ width: `${prepareProgress * 100}%` }} />
                                    </div>
                                )}
                            </>
                        ) : (
                            <>
                                <div className="setup-logo">
                                    <div className="logo-orb error">
                                        <span className="logo-icon">!</span>
                                    </div>
                                </div>
                                <h2 className="setup-step-title">Couldn't start the AI engine</h2>
                                <p className="setup-step-subtitle">
                                    This usually means the download was interrupted or the engine couldn't start.
                                </p>
                                <button className="btn btn-primary btn-lg setup-cta" onClick={handleRetry}>
                                    Try again
                                </button>
                            </>
                        )}
                    </div>
                )}

                {step === 'pick-model' && (
                    <div className="setup-step">
                        <h2 className="setup-step-title">Pick your AI</h2>
                        <p className="setup-step-subtitle">
                            Choose an AI model to get started. You can always add more later.
                        </p>

                        <div className="model-grid">
                            {MODEL_CATALOG.filter(m => m.ramRequired <= 16).map((model) => {
                                const compat = getRamCompatibility(model.ramRequired, systemRAM);
                                return (
                                    <div
                                        key={model.id}
                                        className={`model-card card card-interactive ${selectedModel?.id === model.id ? 'selected' : ''}`}
                                        onClick={() => handleModelSelect(model)}
                                    >
                                        <div className="model-card-header">
                                            <span className="model-emoji">{model.emoji}</span>
                                            <span className="model-name">{model.friendlyName}</span>
                                            {selectedModel?.id === model.id && (
                                                <svg className="model-check" width="20" height="20" viewBox="0 0 24 24" fill="var(--accent)" stroke="none">
                                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                                                </svg>
                                            )}
                                        </div>
                                        <p className="model-description">{model.description}</p>
                                        <div className="model-meta">
                                            <span className="badge badge-accent">{model.bestFor}</span>
                                            <span className="model-size">{model.sizeGB} GB</span>
                                        </div>
                                        <div className={`model-compat compat-${compat}`}>
                                            {compat === 'good' && '✓ Great for your machine'}
                                            {compat === 'tight' && '⚠ May run slowly'}
                                            {compat === 'insufficient' && '✕ Needs more RAM'}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>

                        <button
                            className="btn btn-primary btn-lg setup-cta"
                            disabled={!selectedModel}
                            onClick={handleDownload}
                        >
                            Download & Continue
                        </button>
                    </div>
                )}

                {step === 'downloading' && (
                    <div className="setup-step">
                        <div className="download-animation">
                            <div className="download-spinner" />
                        </div>
                        <h2 className="setup-step-title">
                            Downloading {selectedModel?.emoji} {selectedModel?.friendlyName}
                        </h2>
                        <p className="setup-step-subtitle">{downloadStatus}</p>
                        <div className="progress-bar" style={{ maxWidth: 400, margin: '0 auto' }}>
                            <div className="progress-fill" style={{ width: `${downloadProgress}%` }} />
                        </div>
                        <p className="download-percent">{downloadProgress}%</p>
                    </div>
                )}

                {step === 'ready' && (
                    <div className="setup-step">
                        <div className="setup-logo">
                            <div className="logo-orb ready">
                                <span className="logo-icon">✓</span>
                            </div>
                        </div>
                        <h2 className="setup-step-title">You're all set!</h2>
                        <p className="setup-step-subtitle">
                            {selectedModel?.emoji} {selectedModel?.friendlyName} is ready to chat.
                        </p>
                        <button className="btn btn-primary btn-lg setup-cta" onClick={handleFinish}>
                            Start chatting
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <path d="M5 12h14M12 5l7 7-7 7" />
                            </svg>
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
