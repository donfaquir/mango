import { create } from "zustand";
import { temporal } from "zundo";
import { commands } from "@/lib/bindings/commands";
import type {
  TimelineTrack,
  TimelineItem,
  CreateTimelineItemInput,
  CreateTimelineTrackInput,
  UpdateTimelineItemInput,
  MoveTimelineItemInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export type ProxyState = "idle" | "rendering" | "ready" | "stale";

export interface MultiTrackState {
  tracks: TimelineTrack[];
  items: Record<string, TimelineItem>;

  selection: Set<string>;
  playhead: number;
  zoom: number;
  scrollX: number;
  totalDuration: number;
  importing: boolean;
  proxyState: ProxyState;
  proxyPath: string | null;

  init: (episodeId: string) => Promise<void>;
  reset: () => void;

  addTrack: (input: CreateTimelineTrackInput) => Promise<void>;
  removeTrack: (id: string) => Promise<void>;
  addItem: (input: CreateTimelineItemInput) => Promise<void>;
  updateItem: (id: string, input: UpdateTimelineItemInput) => Promise<void>;
  moveItem: (id: string, input: MoveTimelineItemInput) => Promise<void>;
  removeItem: (id: string) => Promise<void>;
  importFromShots: (episodeId: string) => Promise<void>;

  selectItem: (id: string, multi?: boolean) => void;
  deselectAll: () => void;

  setPlayhead: (ms: number) => void;
  setZoom: (zoom: number) => void;
  setScrollX: (x: number) => void;
  invalidateProxy: () => void;
}

const UNDO_STACK_LIMIT = 50;

function computeTotalDuration(items: Record<string, TimelineItem>): number {
  let max = 0;
  for (const item of Object.values(items)) {
    const end = item.position_ms + item.duration_ms;
    if (end > max) max = end;
  }
  return max;
}

export const useMultiTrackStore = create<MultiTrackState>()(
  temporal(
    (set) => ({
      tracks: [],
      items: {},

      selection: new Set<string>(),
      playhead: 0,
      zoom: 1,
      scrollX: 0,
      totalDuration: 0,
      importing: false,
      proxyState: "idle" as ProxyState,
      proxyPath: null,

      init: async (episodeId: string) => {
        let tracks = await unwrap(commands.listTimelineTracks(episodeId));
        if (tracks.length === 0) {
          tracks = await unwrap(commands.createDefaultTracks(episodeId));
        }

        const items: Record<string, TimelineItem> = {};
        for (const track of tracks) {
          const trackItems = await unwrap(commands.listTimelineItems(track.id));
          for (const item of trackItems) {
            items[item.id] = item;
          }
        }

        const t = useMultiTrackStore.temporal.getState();
        t.pause();
        set({
          tracks,
          items,
          totalDuration: computeTotalDuration(items),
          selection: new Set(),
          playhead: 0,
          proxyState: "idle",
          proxyPath: null,
        });
        t.resume();
        t.clear();
      },

      reset: () => {
        const t = useMultiTrackStore.temporal.getState();
        t.pause();
        set({
          tracks: [],
          items: {},
          selection: new Set(),
          playhead: 0,
          zoom: 1,
          scrollX: 0,
          totalDuration: 0,
          importing: false,
          proxyState: "idle",
          proxyPath: null,
        });
        t.resume();
        t.clear();
      },

      addTrack: async (input: CreateTimelineTrackInput) => {
        const track = await unwrap(commands.createTimelineTrack(input));
        set((s) => ({ tracks: [...s.tracks, track] }));
      },

      removeTrack: async (id: string) => {
        await unwrap(commands.deleteTimelineTrack(id));
        set((s) => {
          const tracks = s.tracks.filter((t) => t.id !== id);
          const next = { ...s.items };
          const sel = new Set(s.selection);
          for (const [itemId, item] of Object.entries(next)) {
            if (item.track_id === id) {
              delete next[itemId];
              sel.delete(itemId);
            }
          }
          return { tracks, items: next, selection: sel, totalDuration: computeTotalDuration(next) };
        });
      },

      addItem: async (input: CreateTimelineItemInput) => {
        const item = await unwrap(commands.createTimelineItem(input));
        set((s) => {
          const next = { ...s.items, [item.id]: item };
          return { items: next, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
        });
      },

      updateItem: async (id: string, input: UpdateTimelineItemInput) => {
        const updated = await unwrap(commands.updateTimelineItem(id, input));
        set((s) => {
          const next = { ...s.items, [updated.id]: updated };
          return { items: next, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
        });
      },

      moveItem: async (id: string, input: MoveTimelineItemInput) => {
        const moved = await unwrap(commands.moveTimelineItem(id, input));
        set((s) => {
          const next = { ...s.items, [moved.id]: moved };
          return { items: next, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
        });
      },

      removeItem: async (id: string) => {
        await unwrap(commands.deleteTimelineItem(id));
        set((s) => {
          const next = { ...s.items };
          delete next[id];
          const sel = new Set(s.selection);
          sel.delete(id);
          return { items: next, selection: sel, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
        });
      },

      importFromShots: async (episodeId: string) => {
        set({ importing: true });
        const t = useMultiTrackStore.temporal.getState();
        t.pause();
        try {
          const videoItems = await unwrap(commands.importVideoFromClips(episodeId));
          const audioItems = await unwrap(commands.importAudioFromShots(episodeId));
          set((s) => {
            const next = { ...s.items };
            for (const item of [...videoItems, ...audioItems]) {
              next[item.id] = item;
            }
            return { items: next, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
          });
        } finally {
          t.resume();
          t.clear();
          set({ importing: false });
        }
      },

      selectItem: (id: string, multi = false) => {
        set((s) => {
          const next = multi ? new Set(s.selection) : new Set<string>();
          if (next.has(id)) {
            next.delete(id);
          } else {
            next.add(id);
          }
          return { selection: next };
        });
      },

      deselectAll: () => set({ selection: new Set() }),
      setPlayhead: (ms: number) => set({ playhead: Math.max(0, ms) }),
      setZoom: (zoom: number) => set({ zoom: Math.max(0.1, Math.min(20, zoom)) }),
      setScrollX: (x: number) => set({ scrollX: Math.max(0, x) }),
      invalidateProxy: () => set({ proxyState: "stale" }),
    }),
    {
      limit: UNDO_STACK_LIMIT,
      partialize: (state) => ({
        tracks: state.tracks,
        items: state.items,
      }),
      equality: (a, b) => a.tracks === b.tracks && a.items === b.items,
    },
  ),
);
