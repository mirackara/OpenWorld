import { create } from 'zustand';

export interface ModelInfo {
    name: string;
    size: number;
    modified_at: string;
    digest: string;
    details?: {
        format?: string;
        family?: string;
        parameter_size?: string;
        quantization_level?: string;
    };
}

export interface ModelCatalogEntry {
    id: string;
    friendlyName: string;
    icon: string;
    description: string;
    bestFor: string;
    sizeGB: number;
    ramRequired: number; // in GB
}

import LlamaIcon from '../assets/models/llama.svg';
import MistralIcon from '../assets/models/mistral.svg';
import GemmaIcon from '../assets/models/gemma.svg';
import PhiIcon from '../assets/models/phi.svg';

export const MODEL_CATALOG: ModelCatalogEntry[] = [
    {
        id: 'llama3:8b',
        friendlyName: 'Llama 3',
        icon: LlamaIcon,
        description: 'Fast, well-rounded, and great at everyday tasks',
        bestFor: 'General chat, writing, Q&A',
        sizeGB: 4.7,
        ramRequired: 8,
    },
    {
        id: 'mistral:7b',
        friendlyName: 'Mistral',
        icon: MistralIcon,
        description: 'Compact but surprisingly smart',
        bestFor: 'Quick answers, coding help',
        sizeGB: 4.1,
        ramRequired: 8,
    },
    {
        id: 'gemma2:9b',
        friendlyName: 'Gemma 2',
        icon: GemmaIcon,
        description: "Google's lightweight model with strong reasoning",
        bestFor: 'Analysis, research',
        sizeGB: 5.4,
        ramRequired: 8,
    },
    {
        id: 'phi3:mini',
        friendlyName: 'Phi-3 Mini',
        icon: PhiIcon,
        description: 'Tiny but capable — runs on almost anything',
        bestFor: 'Low-spec machines',
        sizeGB: 2.3,
        ramRequired: 4,
    },
    {
        id: 'llama3:70b',
        friendlyName: 'Llama 3 70B',
        icon: LlamaIcon,
        description: 'The heavy hitter — closest to GPT-4 quality',
        bestFor: 'Complex reasoning, long documents',
        sizeGB: 40,
        ramRequired: 48,
    },
];

interface ModelState {
    installedModels: ModelInfo[];
    isLoading: boolean;
    pullProgress: { status: string; total?: number; completed?: number } | null;
    pullingModel: string | null;
    systemRAM: number; // in GB

    setInstalledModels: (models: ModelInfo[]) => void;
    setIsLoading: (val: boolean) => void;
    setPullProgress: (progress: { status: string; total?: number; completed?: number } | null) => void;
    setPullingModel: (name: string | null) => void;
    setSystemRAM: (gb: number) => void;
}

export const useModelStore = create<ModelState>((set) => ({
    installedModels: [],
    isLoading: false,
    pullProgress: null,
    pullingModel: null,
    systemRAM: 16,

    setInstalledModels: (models) => set({ installedModels: models }),
    setIsLoading: (val) => set({ isLoading: val }),
    setPullProgress: (progress) => set({ pullProgress: progress }),
    setPullingModel: (name) => set({ pullingModel: name }),
    setSystemRAM: (gb) => set({ systemRAM: gb }),
}));

export function getFriendlyModelName(modelId: string): string {
    const entry = MODEL_CATALOG.find((m) => m.id === modelId);
    if (entry) return entry.friendlyName;
    // For models not in catalog, clean up the ID
    const name = modelId.split(':')[0];
    return name.charAt(0).toUpperCase() + name.slice(1);
}

export function getRamCompatibility(required: number, available: number): 'good' | 'tight' | 'insufficient' {
    if (available >= required * 1.5) return 'good';
    if (available >= required) return 'tight';
    return 'insufficient';
}
