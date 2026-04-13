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
  /// Full recent history (no citations), newest first. `activeIndex` points into this.
  history: Briefing[];
  /// Which entry in `history` the widget is currently displaying.
  activeIndex: number;
  /// The full briefing (with citations) for the active index. Lazy-loaded on nav.
  active: BriefingWithCitations | null;
  isLoading: boolean;
  isRunning: boolean;
  error: string | null;

  /// Load the latest briefing and surrounding history. Called on mount and
  /// whenever the backend emits `briefing-ready`.
  fetchLatest: () => Promise<void>;
  /// Step by `delta` (+1 = older, -1 = newer). No-op at edges.
  navigate: (delta: number) => Promise<void>;
  /// Generate a new briefing and reset the view to the newest entry.
  runNow: () => Promise<void>;
  reset: () => void;
}

const HISTORY_LIMIT = 30;

export const useBriefingStore = create<BriefingStore>((set, get) => ({
  history: [],
  activeIndex: 0,
  active: null,
  isLoading: false,
  isRunning: false,
  error: null,

  fetchLatest: async () => {
    set({ isLoading: true, error: null });
    try {
      const transport = getTransport();
      const history = await transport.invoke<Briefing[]>('list_briefings', { limit: HISTORY_LIMIT });
      if (history.length === 0) {
        set({ history: [], activeIndex: 0, active: null, isLoading: false });
        return;
      }
      const active = await transport.invoke<BriefingWithCitations>('get_briefing', { id: history[0].id });
      set({ history, activeIndex: 0, active, isLoading: false });
    } catch (error) {
      const msg = String(error);
      // A 404 just means no briefing has been generated yet — not an error state.
      if (msg.includes('404') || msg.toLowerCase().includes('not found')) {
        set({ history: [], activeIndex: 0, active: null, isLoading: false });
      } else {
        set({ error: msg, isLoading: false });
      }
    }
  },

  navigate: async (delta: number) => {
    const { history, activeIndex } = get();
    const next = activeIndex + delta;
    if (next < 0 || next >= history.length) return;
    set({ activeIndex: next, isLoading: true, error: null });
    try {
      const active = await getTransport().invoke<BriefingWithCitations>('get_briefing', { id: history[next].id });
      set({ active, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  runNow: async () => {
    set({ isRunning: true, error: null });
    try {
      const transport = getTransport();
      const active = await transport.invoke<BriefingWithCitations>('run_briefing_now');
      // Refresh history so the new briefing is at index 0; keep the freshly-returned
      // active object so we don't need a second round-trip.
      const history = await transport.invoke<Briefing[]>('list_briefings', { limit: HISTORY_LIMIT });
      set({ history, activeIndex: 0, active, isRunning: false });
    } catch (error) {
      set({ error: String(error), isRunning: false });
    }
  },

  reset: () =>
    set({ history: [], activeIndex: 0, active: null, isLoading: false, isRunning: false, error: null }),
}));
