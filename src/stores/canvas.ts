import { create } from 'zustand';
import type { GlobalCanvasData } from '../lib/api';

export interface CanvasController {
  zoomToCluster: (label: string) => void;
  focusAtom: (atomId: string) => void;
}

interface CanvasStore {
  // Main canvas controller — owned by the full SigmaCanvas view, driven by the
  // chat agent's tool calls (zoom_to_cluster, focus_atom).
  controller: CanvasController | null;
  registerController: (ctrl: CanvasController) => void;
  unregisterController: () => void;

  // Preview canvas controller — owned by whichever <SigmaCanvas mode="preview" />
  // is currently mounted (e.g. the dashboard briefing widget). Distinct from the
  // main controller so chat agent actions never accidentally drive a thumbnail.
  previewController: CanvasController | null;
  registerPreviewController: (ctrl: CanvasController) => void;
  unregisterPreviewController: () => void;

  // Canvas data (clusters for chat context)
  canvasData: GlobalCanvasData | null;
  setCanvasData: (data: GlobalCanvasData) => void;
}

export const useCanvasStore = create<CanvasStore>()((set) => ({
  controller: null,
  previewController: null,
  canvasData: null,

  registerController: (ctrl) => set({ controller: ctrl }),
  unregisterController: () => set({ controller: null }),

  registerPreviewController: (ctrl) => set({ previewController: ctrl }),
  unregisterPreviewController: () => set({ previewController: null }),

  setCanvasData: (data) => set({ canvasData: data }),
}));
