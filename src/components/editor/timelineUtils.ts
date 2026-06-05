export function msToPixel(ms: number, zoom: number): number {
  return (ms / 100) * zoom;
}

export function pixelToMs(px: number, zoom: number): number {
  return (px / zoom) * 100;
}
