import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '../stores/settingsStore';
import { useModelStore, getFriendlyModelName } from '../stores/modelStore';
import './SettingsPage.css';

export default function SettingsPage() {
    const { theme, setTheme, defaultModel, setDefaultModel, systemPrompt, setSystemPrompt } =
        useSettingsStore();
    const { installedModels, setInstalledModels } = useModelStore();
    const [saved, setSaved] = useState(false);

    useEffect(() => {
        invoke<any[]>('list_models')
            .then(setInstalledModels)
            .catch(console.error);
    }, []);

    async function handleSave() {
        try {
            await invoke('save_config_cmd', {
                cfg: {
                    theme,
                    default_model: defaultModel,
                    setup_complete: true,
                    system_prompt: systemPrompt,
                    ollama_host: 'http://localhost:11434',
                },
            });
            setSaved(true);
            setTimeout(() => setSaved(false), 2000);
        } catch (err) {
            console.error('Failed to save settings:', err);
        }
    }

    function handleThemeToggle() {
        const newTheme = theme === 'dark' ? 'light' : 'dark';
        setTheme(newTheme);
    }

    return (
        <div className="settings-page fade-in">
            <div className="settings-header">
                <h1>Settings</h1>
                <p>Customize your OpenWorld experience</p>
            </div>

            <div className="settings-sections">
                {/* Appearance */}
                <section className="settings-section card">
                    <h3 className="settings-section-title">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <circle cx="12" cy="12" r="5" />
                            <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                        </svg>
                        Appearance
                    </h3>
                    <div className="setting-row">
                        <div className="setting-info">
                            <span className="setting-label">Theme</span>
                            <span className="setting-desc">Choose between dark and light mode</span>
                        </div>
                        <button className="theme-toggle" onClick={handleThemeToggle}>
                            <div className={`toggle-track ${theme}`}>
                                <div className="toggle-thumb">
                                    {theme === 'dark' ? '🌙' : '☀️'}
                                </div>
                            </div>
                        </button>
                    </div>
                </section>

                {/* Default Model */}
                <section className="settings-section card">
                    <h3 className="settings-section-title">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z" />
                        </svg>
                        Default AI Model
                    </h3>
                    <div className="setting-row">
                        <div className="setting-info">
                            <span className="setting-label">Choose your default model</span>
                            <span className="setting-desc">This model will be used for new conversations</span>
                        </div>
                        <select
                            className="input model-select"
                            value={defaultModel}
                            onChange={(e) => setDefaultModel(e.target.value)}
                        >
                            {installedModels.map((m) => (
                                <option key={m.name} value={m.name}>
                                    {getFriendlyModelName(m.name)}
                                </option>
                            ))}
                        </select>
                    </div>
                </section>

                {/* Custom Instructions */}
                <section className="settings-section card">
                    <h3 className="settings-section-title">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M12 20h9M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
                        </svg>
                        Custom Instructions
                    </h3>
                    <div className="setting-col">
                        <span className="setting-label">Tell the AI about yourself</span>
                        <span className="setting-desc">
                            These instructions will be included with every conversation
                        </span>
                        <textarea
                            className="input system-prompt-input"
                            placeholder="e.g., I'm a software engineer who prefers concise answers..."
                            value={systemPrompt}
                            onChange={(e) => setSystemPrompt(e.target.value)}
                            rows={4}
                        />
                    </div>
                </section>

                {/* Save */}
                <div className="settings-save">
                    <button className="btn btn-primary btn-lg" onClick={handleSave}>
                        {saved ? '✓ Saved!' : 'Save Settings'}
                    </button>
                </div>
            </div>
        </div>
    );
}
