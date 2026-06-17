import {
  ArrowRightLeft,
  Smile,
  Sparkles,
  Type,
  MessageCircle,
  Brush,
  Zap,
} from "lucide-react";
import type { TimelineItem } from "@/lib/bindings/commands";

const TEXT_TYPE_ICONS: Record<string, React.ElementType> = {
  subtitle: Type,
  bubble: MessageCircle,
  fancy: Brush,
  onomatopoeia: Zap,
};

const OVERLAY_ICONS: Record<string, React.ElementType> = {
  transition: ArrowRightLeft,
  sticker: Smile,
  effect: Sparkles,
};

export function ItemLabel({
  item,
  trackType,
  params,
}: {
  item: TimelineItem;
  trackType: string;
  params: Record<string, unknown> | null;
}) {
  if (trackType === "text" && item.item_type === "text" && params) {
    const textType = (params.text_type as string) ?? "text";
    const content = typeof params.content === "string" ? params.content : "";
    const preview = content.length > 20 ? content.slice(0, 20) + "…" : content;
    const Icon = TEXT_TYPE_ICONS[textType] ?? Type;
    return (
      <>
        <Icon className="size-3 shrink-0 text-white drop-shadow-sm" />
        <span className="text-[10px] text-white font-medium truncate drop-shadow-sm">
          {preview || textType}
        </span>
      </>
    );
  }

  const OverlayIcon = OVERLAY_ICONS[item.item_type];
  if (OverlayIcon) {
    const label =
      item.item_type === "transition" && params
        ? (params.transition_type as string) ?? "Transition"
        : item.item_type === "effect" && params
          ? (params.effect_type as string) ?? "Effect"
          : item.item_type.charAt(0).toUpperCase() + item.item_type.slice(1);
    return (
      <>
        <OverlayIcon className="size-3 shrink-0 text-white drop-shadow-sm" />
        <span className="text-[10px] text-white font-medium truncate drop-shadow-sm">
          {label}
        </span>
      </>
    );
  }

  return (
    <span className="text-[10px] text-white font-medium truncate drop-shadow-sm">
      {item.item_type === "clip" ? "Clip" : item.item_type}
    </span>
  );
}
