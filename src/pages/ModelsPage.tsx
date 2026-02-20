import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
    useModelStore,
    MODEL_CATALOG,
    getRamCompatibility,
    type ModelInfo,
} from '../stores/modelStore';
import { useSettingsStore } from '../stores/settingsStore';
import './ModelsPage.css';

export default function ModelsPage() {
    const {
        installedModels,
        setInstalledModels,
        setIsLoading,
        pullingModel,
        setPullingModel,
        pullProgress,
        setPullProgress,
        systemRAM,
    } = useModelStore();
    const { defaultModel, setDefaultModel } = useSettingsStore();
    const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);

    useEffect(() => {
        loadModels();
    }, []);

    async function loadModels() {
        setIsLoading(true);
        try {
            const models = await invoke<ModelInfo[]>('list_models');
            console.log('[openworld] Installed models:', models.map(m => m.name));
            setInstalledModels(models);
        } catch (err) {
            console.error('Failed to load models:', err);
        }
        setIsLoading(false);
    }

    async function handlePull(modelId: string) {
        console.log('[openworld] Pulling model:', modelId);
        setPullingModel(modelId);
        setPullProgress({ status: 'Starting...' });

        const unlisten = await listen<{ status: string; total?: number; completed?: number }>(
            'model-pull-progress',
            (event) => {
                setPullProgress(event.payload);
            }
        );

        try {
            await invoke('pull_model', { modelName: modelId });
            console.log('[openworld] Pull completed for:', modelId);
            await loadModels();
        } catch (err) {
            console.error('[openworld] Pull failed:', err);
        } finally {
            setPullingModel(null);
            setPullProgress(null);
            unlisten();
        }
    }

    async function handleDelete(modelName: string) {
        try {
            await invoke('delete_model', { modelName });
            await loadModels();
            setDeleteConfirm(null);
        } catch (err) {
            console.error('Delete failed:', err);
        }
    }

    async function handleSetDefault(modelId: string) {
        setDefaultModel(modelId);
        try {
            const config = await invoke<any>('get_config');
            await invoke('save_config_cmd', {
                cfg: { ...config, default_model: modelId },
            });
        } catch (err) {
            console.error('Failed to save default model:', err);
        }
    }

    function isInstalled(modelId: string): boolean {
        return installedModels.some((m) => m.name === modelId || m.name.startsWith(modelId.split(':')[0]));
    }

    function formatSize(bytes: number): string {
        const gb = bytes / (1024 * 1024 * 1024);
        return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
    }

    return (
        <div className="models-page fade-in">
            <div className="models-header">
                <h1>Models</h1>
                <p>Choose and manage your AI models</p>
            </div>

            {/* Installed Models */}
            {installedModels.length > 0 && (
                <section className="models-section">
                    <h2 className="section-title">Your Models</h2>
                    <div className="installed-grid">
                        {installedModels.map((model) => {
                            const catalogEntry = MODEL_CATALOG.find(
                                (c) => c.id === model.name || model.name.startsWith(c.id.split(':')[0])
                            );
                            const isDefault = model.name === defaultModel;

                            return (
                                <div key={model.name} className="card installed-card">
                                    <div className="installed-header">
                                        <span className="installed-emoji">
                                            {catalogEntry?.emoji || '🤖'}
                                        </span>
                                        <div className="installed-info">
                                            <span className="installed-name">
                                                {catalogEntry?.friendlyName || model.name}
                                            </span>
                                            <span className="installed-size">{formatSize(model.size)}</span>
                                        </div>
                                        {isDefault && <span className="badge badge-accent">Active</span>}
                                    </div>
                                    {catalogEntry && (
                                        <p className="installed-desc">{catalogEntry.description}</p>
                                    )}
                                    <div className="installed-actions">
                                        {!isDefault && (
                                            <button
                                                className="btn btn-secondary"
                                                onClick={() => handleSetDefault(model.name)}
                                            >
                                                Set as default
                                            </button>
                                        )}
                                        {deleteConfirm === model.name ? (
                                            <div className="delete-confirm">
                                                <span>Delete?</span>
                                                <button className="btn btn-danger" onClick={() => handleDelete(model.name)}>
                                                    Yes
                                                </button>
                                                <button className="btn btn-ghost" onClick={() => setDeleteConfirm(null)}>
                                                    No
                                                </button>
                                            </div>
                                        ) : (
                                            <button
                                                className="btn btn-ghost"
                                                onClick={() => setDeleteConfirm(model.name)}
                                            >
                                                Remove
                                            </button>
                                        )}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </section>
            )}

            {/* Available Models */}
            <section className="models-section">
                <h2 className="section-title">Available Models</h2>
                <div className="available-grid">
                    {MODEL_CATALOG.map((model) => {
                        const installed = isInstalled(model.id);
                        const isPulling = pullingModel === model.id;
                        const compat = getRamCompatibility(model.ramRequired, systemRAM);
                        const progress =
                            isPulling && pullProgress?.total && pullProgress?.completed
                                ? Math.round((pullProgress.completed / pullProgress.total) * 100)
                                : 0;

                        return (
                            <div key={model.id} className={`card model-catalog-card ${installed ? 'installed' : ''}`}>
                                <div className="catalog-header">
                                    <span className="catalog-emoji">{model.emoji}</span>
                                    <div className="catalog-info">
                                        <span className="catalog-name">{model.friendlyName}</span>
                                        <span className="catalog-best-for">{model.bestFor}</span>
                                    </div>
                                </div>
                                <p className="catalog-desc">{model.description}</p>
                                <div className="catalog-meta">
                                    <span className="catalog-size">{model.sizeGB} GB download</span>
                                    <span className={`catalog-compat compat-${compat}`}>
                                        {compat === 'good' && '✓ Great fit'}
                                        {compat === 'tight' && '⚠ Tight fit'}
                                        {compat === 'insufficient' && '✕ Not enough RAM'}
                                    </span>
                                </div>
                                {isPulling ? (
                                    <div className="catalog-pulling">
                                        <div className="progress-bar">
                                            <div className="progress-fill" style={{ width: `${progress}%` }} />
                                        </div>
                                        <span className="pull-status">{pullProgress?.status || 'Downloading...'} {progress > 0 && `${progress}%`}</span>
                                    </div>
                                ) : installed ? (
                                    <div className="catalog-installed-badge">
                                        <span className="badge badge-success">Installed ✓</span>
                                    </div>
                                ) : (
                                    <button
                                        className="btn btn-primary catalog-download-btn"
                                        onClick={() => handlePull(model.id)}
                                        disabled={compat === 'insufficient'}
                                    >
                                        Get
                                    </button>
                                )}
                            </div>
                        );
                    })}
                </div>
            </section>
        </div>
    );
}
