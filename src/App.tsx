import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from './stores/settingsStore';
import Layout from './components/Layout';
import SetupWizard from './pages/SetupWizard';
import ChatPage from './pages/ChatPage';
import ModelsPage from './pages/ModelsPage';
import SettingsPage from './pages/SettingsPage';
import PluginsPage from './pages/PluginsPage';

function App() {
  const { setupComplete, loadFromConfig } = useSettingsStore();
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function init() {
      try {
        const config = await invoke<{
          theme: string;
          default_model: string;
          setup_complete: boolean;
          system_prompt: string;
          ollama_host: string;
        }>('get_config');
        loadFromConfig(config);
      } catch (err) {
        console.error('Failed to load config:', err);
      }
      setLoading(false);
    }
    init();
  }, []);

  if (loading) {
    return (
      <div style={{
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'var(--bg-primary)',
      }}>
        <div style={{
          width: 40,
          height: 40,
          border: '3px solid var(--border)',
          borderTopColor: 'var(--accent)',
          borderRadius: '50%',
          animation: 'spin 0.8s linear infinite',
        }} />
      </div>
    );
  }

  if (!setupComplete) {
    return (
      <BrowserRouter>
        <Routes>
          <Route path="*" element={<SetupWizard />} />
        </Routes>
      </BrowserRouter>
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<Navigate to="/chat" replace />} />
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/chat/:conversationId" element={<ChatPage />} />
          <Route path="/models" element={<ModelsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/plugins" element={<PluginsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/chat" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
