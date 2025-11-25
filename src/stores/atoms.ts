import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface Atom {
  id: string;
  content: string;
  source_url: string | null;
  created_at: string;
  updated_at: string;
  embedding_status: 'pending' | 'processing' | 'complete' | 'failed';
}

export interface Tag {
  id: string;
  name: string;
  parent_id: string | null;
  created_at: string;
}

export interface AtomWithTags extends Atom {
  tags: Tag[];
}

interface AtomsStore {
  atoms: AtomWithTags[];
  isLoading: boolean;
  error: string | null;
  fetchAtoms: () => Promise<void>;
  fetchAtomsByTag: (tagId: string) => Promise<void>;
  createAtom: (content: string, sourceUrl?: string, tagIds?: string[]) => Promise<AtomWithTags>;
  updateAtom: (id: string, content: string, sourceUrl?: string, tagIds?: string[]) => Promise<AtomWithTags>;
  deleteAtom: (id: string) => Promise<void>;
  clearError: () => void;
}

export const useAtomsStore = create<AtomsStore>((set) => ({
  atoms: [],
  isLoading: false,
  error: null,

  fetchAtoms: async () => {
    set({ isLoading: true, error: null });
    try {
      const atoms = await invoke<AtomWithTags[]>('get_all_atoms');
      set({ atoms, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  fetchAtomsByTag: async (tagId: string) => {
    set({ isLoading: true, error: null });
    try {
      const atoms = await invoke<AtomWithTags[]>('get_atoms_by_tag', { tagId });
      set({ atoms, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  createAtom: async (content: string, sourceUrl?: string, tagIds?: string[]) => {
    set({ error: null });
    try {
      const atom = await invoke<AtomWithTags>('create_atom', {
        content,
        sourceUrl: sourceUrl || null,
        tagIds: tagIds || [],
      });
      set((state) => ({ atoms: [atom, ...state.atoms] }));
      return atom;
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },

  updateAtom: async (id: string, content: string, sourceUrl?: string, tagIds?: string[]) => {
    set({ error: null });
    try {
      const atom = await invoke<AtomWithTags>('update_atom', {
        id,
        content,
        sourceUrl: sourceUrl || null,
        tagIds: tagIds || [],
      });
      set((state) => ({
        atoms: state.atoms.map((a) => (a.id === id ? atom : a)),
      }));
      return atom;
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },

  deleteAtom: async (id: string) => {
    set({ error: null });
    try {
      await invoke('delete_atom', { id });
      set((state) => ({
        atoms: state.atoms.filter((a) => a.id !== id),
      }));
    } catch (error) {
      set({ error: String(error) });
      throw error;
    }
  },

  clearError: () => set({ error: null }),
}));

