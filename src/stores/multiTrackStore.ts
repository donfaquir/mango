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
  isPlaying: boolean;
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

  addTransition: (afterClipId: string, transitionType: string, durationMs: number) => Promise<void>;
  splitItem: (id: string, atMs: number) => Promise<void>;
  duplicateItem: (id: string) => Promise<void>;
  toggleMuted: (trackId: string) => Promise<void>;
  toggleLocked: (trackId: string) => Promise<void>;

  selectItem: (id: string, multi?: boolean) => void;
  deselectAll: () => void;

  play: () => void;
  pause: () => void;
  togglePlay: () => void;
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
      isPlaying: false,
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
          isPlaying: false,
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
          isPlaying: false,
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

      addTransition: async (afterClipId: string, transitionType: string, durationMs: number) => {
        const state = useMultiTrackStore.getState();
        const afterClip = state.items[afterClipId];
        if (!afterClip || afterClip.item_type !== "clip") return;

        const sameTrackClips = Object.values(state.items)
          .filter((i) => i.track_id === afterClip.track_id && i.item_type === "clip")
          .sort((a, b) => a.position_ms - b.position_ms);
        const idx = sameTrackClips.findIndex((c) => c.id === afterClipId);
        if (idx < 0 || idx >= sameTrackClips.length - 1) return;

        const overlayTrack = state.tracks.find((t) => t.track_type === "overlay");
        if (!overlayTrack) return;

        const positionMs = afterClip.position_ms + afterClip.duration_ms;
        const paramsJson = JSON.stringify({
          type: "Transition",
          transition_type: transitionType,
          duration_ms: durationMs,
        });

        const item = await unwrap(commands.createTimelineItem({
          track_id: overlayTrack.id,
          item_type: "transition",
          position_ms: positionMs,
          duration_ms: durationMs,
          in_point_ms: 0,
          out_point_ms: durationMs,
          asset_id: null,
          params_json: paramsJson,
        }));
        set((s) => {
          const next = { ...s.items, [item.id]: item };
          return {
            items: next,
            selection: new Set([item.id]),
            totalDuration: computeTotalDuration(next),
            proxyState: "stale" as ProxyState,
          };
        });
      },

      splitItem: async (id: string, atMs: number) => {
        const item = useMultiTrackStore.getState().items[id];
        if (!item) return;
        const start = item.position_ms;
        const end = start + item.duration_ms;
        if (atMs <= start || atMs >= end) return;

        const splitSourceMs = item.in_point_ms + (atMs - start);
        const updated = await unwrap(commands.updateTimelineItem(id, {
          position_ms: null,
          duration_ms: atMs - start,
          in_point_ms: null,
          out_point_ms: splitSourceMs,
          params_json: null,
        }));
        const created = await unwrap(commands.createTimelineItem({
          track_id: item.track_id,
          item_type: item.item_type,
          position_ms: atMs,
          duration_ms: end - atMs,
          in_point_ms: splitSourceMs,
          out_point_ms: item.out_point_ms,
          asset_id: item.asset_id ?? null,
          params_json: item.params_json,
        }));
        set((s) => {
          const next = { ...s.items, [updated.id]: updated, [created.id]: created };
          return { items: next, totalDuration: computeTotalDuration(next), proxyState: "stale" as ProxyState };
        });
      },

      duplicateItem: async (id: string) => {
        const item = useMultiTrackStore.getState().items[id];
        if (!item) return;
        const created = await unwrap(commands.createTimelineItem({
          track_id: item.track_id,
          item_type: item.item_type,
          position_ms: item.position_ms + item.duration_ms,
          duration_ms: item.duration_ms,
          in_point_ms: item.in_point_ms,
          out_point_ms: item.out_point_ms,
          asset_id: item.asset_id ?? null,
          params_json: item.params_json,
        }));
        set((s) => {
          const next = { ...s.items, [created.id]: created };
          return {
            items: next,
            selection: new Set([created.id]),
            totalDuration: computeTotalDuration(next),
            proxyState: "stale" as ProxyState,
          };
        });
      },

      toggleMuted: async (trackId: string) => {
        const track = useMultiTrackStore.getState().tracks.find((t) => t.id === trackId);
        if (!track) return;
        const newMuted = !track.muted;
        await unwrap(commands.updateTrackMuted(trackId, newMuted));
        set((s) => ({
          tracks: s.tracks.map((t) => (t.id === trackId ? { ...t, muted: newMuted } : t)),
        }));
      },

      toggleLocked: async (trackId: string) => {
        const track = useMultiTrackStore.getState().tracks.find((t) => t.id === trackId);
        if (!track) return;
        const newLocked = !track.locked;
        await unwrap(commands.updateTrackLocked(trackId, newLocked));
        set((s) => ({
          tracks: s.tracks.map((t) => (t.id === trackId ? { ...t, locked: newLocked } : t)),
        }));
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
      play: () => set({ isPlaying: true }),
      pause: () => set({ isPlaying: false }),
      togglePlay: () => set((s) => ({ isPlaying: !s.isPlaying })),
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
