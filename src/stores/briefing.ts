import { create } from 'zustand';
import { getTransport } from '../lib/transport';

export interface Briefing {
  id: string;
  content: string;
  created_at: string;
  atom_count: number;
  last_run_at: string;
}

export interface BriefingCitation {
  id: string;
  briefing_id: string;
  citation_index: number;
  atom_id: string;
  excerpt: string;
  source_url?: string | null;
}

export interface BriefingWithCitations {
  briefing: Briefing;
  citations: BriefingCitation[];
}

interface BriefingStore {
  latest: BriefingWithCitations | null;
  isLoading: boolean;
  isRunning: boolean;
  error: string | null;

  fetchLatest: () => Promise<void>;
  runNow: () => Promise<void>;
  reset: () => void;
}

export const useBriefingStore = create<BriefingStore>((set) => ({
  latest: null,
  isLoading: false,
  isRunning: false,
  error: null,

  fetchLatest: async () => {
    set({ isLoading: true, error: null });
    try {
      const latest = await getTransport().invoke<BriefingWithCitations | null>('get_latest_briefing');
      set({ latest, isLoading: false });
    } catch (error) {
      const msg = String(error);
      // A 404 just means no briefing has been generated yet — not an error state.
      if (msg.includes('404') || msg.toLowerCase().includes('not found')) {
        set({ latest: null, isLoading: false });
      } else {
        set({ error: msg, isLoading: false });
      }
    }
  },

  runNow: async () => {
    set({ isRunning: true, error: null });
    try {
      const latest = await getTransport().invoke<BriefingWithCitations>('run_briefing_now');
      set({ latest, isRunning: false });
    } catch (error) {
      set({ error: String(error), isRunning: false });
    }
  },

  reset: () => set({ latest: null, isLoading: false, isRunning: false, error: null }),
}));
