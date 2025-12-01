import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useAtomsStore } from '../stores/atoms';
import { useTagsStore } from '../stores/tags';
import { useUIStore } from '../stores/ui';
import type { AtomWithTags } from '../stores/atoms';

// Embedding complete - fast, no tags (just embedding status update)
interface EmbeddingCompletePayload {
  atom_id: string;
  status: 'complete' | 'failed';
  error?: string;
}

// Tagging complete - slower, has tag info
interface TaggingCompletePayload {
  atom_id: string;
  status: 'complete' | 'failed' | 'skipped';
  error?: string;
  tags_extracted: string[];
  new_tags_created: string[];
}

// Embeddings reset - when provider/model changes and all atoms need re-embedding
interface EmbeddingsResetPayload {
  pending_count: number;
  reason: string;
}

export function useEmbeddingEvents() {
  const updateAtomStatus = useAtomsStore((s) => s.updateAtomStatus);
  const fetchTags = useTagsStore((s) => s.fetchTags);
  const fetchAtoms = useAtomsStore((s) => s.fetchAtoms);
  const addAtomToStore = useAtomsStore((s) => s.addAtom);
  const addLoadingOperation = useUIStore((s) => s.addLoadingOperation);
  const removeLoadingOperation = useUIStore((s) => s.removeLoadingOperation);

  useEffect(() => {
    // Listen for atom-created events (from HTTP API / browser extension)
    const unlistenAtomCreated = listen<AtomWithTags>('atom-created', (event) => {
      console.log('Atom created via HTTP API:', event.payload);
      addAtomToStore(event.payload);
    });

    // Listen for embedding-complete events (fast, embedding only)
    const unlistenEmbeddingComplete = listen<EmbeddingCompletePayload>('embedding-complete', (event) => {
      console.log('Embedding complete event:', event.payload);
      updateAtomStatus(event.payload.atom_id, event.payload.status);
    });

    // Listen for tagging-complete events (slower, has tag info)
    const unlistenTaggingComplete = listen<TaggingCompletePayload>('tagging-complete', (event) => {
      console.log('Tagging complete event:', event.payload);

      // If new tags were created, refresh the tag tree
      if (event.payload.new_tags_created && event.payload.new_tags_created.length > 0) {
        console.log('New tags created:', event.payload.new_tags_created);
        const opId = `fetch-tags-${Date.now()}`;
        addLoadingOperation(opId, 'Refreshing tags...');
        fetchTags().finally(() => removeLoadingOperation(opId));
      }

      // If tags were extracted, refresh atoms to show updated tags
      if (event.payload.tags_extracted && event.payload.tags_extracted.length > 0) {
        console.log('Tags extracted:', event.payload.tags_extracted);
        const opId = `fetch-atoms-${Date.now()}`;
        addLoadingOperation(opId, 'Updating atoms...');
        fetchAtoms().finally(() => removeLoadingOperation(opId));
      }
    });

    // Listen for embeddings-reset events (provider/model change triggers re-embedding)
    const unlistenEmbeddingsReset = listen<EmbeddingsResetPayload>('embeddings-reset', (event) => {
      console.log('Embeddings reset event:', event.payload);
      // Re-fetch atoms to show updated pending status
      const opId = `fetch-atoms-reset-${Date.now()}`;
      addLoadingOperation(opId, `Re-embedding ${event.payload.pending_count} atoms...`);
      fetchAtoms().finally(() => removeLoadingOperation(opId));
    });

    return () => {
      unlistenAtomCreated.then(fn => fn());
      unlistenEmbeddingComplete.then(fn => fn());
      unlistenTaggingComplete.then(fn => fn());
      unlistenEmbeddingsReset.then(fn => fn());
    };
  }, [updateAtomStatus, fetchTags, fetchAtoms, addAtomToStore, addLoadingOperation, removeLoadingOperation]);
}
