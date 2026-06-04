import { create } from "zustand";

interface TimelineState {
  videoSrc: string | null;
  duration: number;
  trimStart: number;
  trimEnd: number;
  playbackPosition: number;
  zoom: number;
  isPlaying: boolean;

  setVideoSrc: (src: string, duration: number) => void;
  setTrim: (start: number, end: number) => void;
  setPlaybackPosition: (pos: number) => void;
  setZoom: (zoom: number) => void;
  togglePlay: () => void;
  reset: () => void;
}

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 20;

export const useTimelineStore = create<TimelineState>((set) => ({
  videoSrc: null,
  duration: 0,
  trimStart: 0,
  trimEnd: 0,
  playbackPosition: 0,
  zoom: 1.0,
  isPlaying: false,

  setVideoSrc: (src, duration) =>
    set({ videoSrc: src, duration, trimStart: 0, trimEnd: duration, playbackPosition: 0 }),

  setTrim: (start, end) => set({ trimStart: start, trimEnd: end }),

  setPlaybackPosition: (pos) => set({ playbackPosition: pos }),

  setZoom: (zoom) =>
    set({ zoom: Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom)) }),

  togglePlay: () => set((s) => ({ isPlaying: !s.isPlaying })),

  reset: () =>
    set({
      videoSrc: null,
      duration: 0,
      trimStart: 0,
      trimEnd: 0,
      playbackPosition: 0,
      zoom: 1.0,
      isPlaying: false,
    }),
}));
