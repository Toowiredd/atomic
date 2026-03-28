import { create } from 'zustand';
import { getTransport } from '../lib/transport';
import { useAtomsStore } from './atoms';
import { useTagsStore } from './tags';
import { useWikiStore } from './wiki';
import { useChatStore } from './chat';

export interface DatabaseInfo {
  id: string;
  name: string;
  is_default: boolean;
  created_at: string;
  last_opened_at: string | null;
}

export interface DatabaseStats {
  atom_count: number;
}

interface DatabasesStore {
  databases: DatabaseInfo[];
  activeId: string | null;
  isLoading: boolean;
  error: string | null;

  fetchDatabases: () => Promise<void>;
  createDatabase: (name: string) => Promise<DatabaseInfo>;
  renameDatabase: (id: string, name: string) => Promise<void>;
  deleteDatabase: (id: string) => Promise<void>;
  switchDatabase: (id: string) => Promise<void>;
  setDefaultDatabase: (id: string) => Promise<void>;
  getDatabaseStats: (id: string) => Promise<DatabaseStats>;
}

export const useDatabasesStore = create<DatabasesStore>((set, get) => ({
  databases: [],
  activeId: null,
  isLoading: false,
  error: null,

  fetchDatabases: async () => {
    set({ isLoading: true, error: null });
    try {
      const transport = getTransport();
      const result = await transport.invoke('list_databases', {}) as {
        databases: DatabaseInfo[];
        active_id: string;
      };
      set({
        databases: result.databases,
        activeId: result.active_id,
        isLoading: false,
      });
    } catch (e) {
      set({ error: String(e), isLoading: false });
    }
  },

  createDatabase: async (name: string) => {
    const transport = getTransport();
    const info = await transport.invoke('create_database', { name }) as DatabaseInfo;
    await get().fetchDatabases();
    return info;
  },

  renameDatabase: async (id: string, name: string) => {
    const transport = getTransport();
    await transport.invoke('rename_database', { id, name });
    await get().fetchDatabases();
  },

  deleteDatabase: async (id: string) => {
    const transport = getTransport();
    const wasActive = get().activeId === id;
    await transport.invoke('delete_database', { id });
    await get().fetchDatabases();

    // If the deleted DB was active, the backend switched to default —
    // reset data stores to load the new active DB's data
    if (wasActive) {
      useAtomsStore.getState().reset();
      useTagsStore.getState().reset();
      useWikiStore.getState().reset();
      useChatStore.getState().reset();
      useTagsStore.getState().fetchTags();
      useAtomsStore.getState().fetchAtoms();
    }
  },

  setDefaultDatabase: async (id: string) => {
    const transport = getTransport();
    await transport.invoke('set_default_database', { id });
    await get().fetchDatabases();
  },

  getDatabaseStats: async (id: string) => {
    const transport = getTransport();
    return await transport.invoke('get_database_stats', { id }) as DatabaseStats;
  },

  switchDatabase: async (id: string) => {
    const transport = getTransport();
    await transport.invoke('activate_database', { id });
    set({ activeId: id });

    // Reset all data stores to force refetch
    useAtomsStore.getState().reset();
    useTagsStore.getState().reset();
    useWikiStore.getState().reset();
    useChatStore.getState().reset();

    // Refetch data for the new database
    useTagsStore.getState().fetchTags();
    useAtomsStore.getState().fetchAtoms();
  },
}));
