import { create } from 'zustand';

interface SettingsState {
    theme: 'dark' | 'light';
    defaultModel: string;
    setupComplete: boolean;
    systemPrompt: string;

    setTheme: (theme: 'dark' | 'light') => void;
    setDefaultModel: (model: string) => void;
    setSetupComplete: (val: boolean) => void;
    setSystemPrompt: (prompt: string) => void;
    loadFromConfig: (config: {
        theme: string;
        default_model: string;
        setup_complete: boolean;
        system_prompt: string;
    }) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
    theme: 'dark',
    defaultModel: 'llama3:8b',
    setupComplete: false,
    systemPrompt: '',

    setTheme: (theme) => {
        document.documentElement.setAttribute('data-theme', theme);
        set({ theme });
    },
    setDefaultModel: (model) => set({ defaultModel: model }),
    setSetupComplete: (val) => set({ setupComplete: val }),
    setSystemPrompt: (prompt) => set({ systemPrompt: prompt }),
    loadFromConfig: (config) => {
        const theme = config.theme === 'light' ? 'light' : 'dark';
        document.documentElement.setAttribute('data-theme', theme);
        set({
            theme: theme as 'dark' | 'light',
            defaultModel: config.default_model || 'llama3:8b',
            setupComplete: config.setup_complete,
            systemPrompt: config.system_prompt || '',
        });
    },
}));
