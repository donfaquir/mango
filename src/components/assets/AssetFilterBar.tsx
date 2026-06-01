import { useCallback, useEffect, useState } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { AssetSource, AssetType } from "@/lib/bindings/commands";

export interface AssetFilterValues {
  type: AssetType | undefined;
  source: AssetSource | undefined;
  keyword: string;
  /**
   * Exact-match label filter. `undefined` = any label; `""` = unlabeled only;
   * a non-empty string filters to rows whose label equals it exactly.
   */
  label: string | undefined;
}

interface AssetFilterBarProps {
  value: AssetFilterValues;
  /** Distinct non-empty labels currently used in the project, alphabetised. */
  labels: string[];
  onChange: (value: AssetFilterValues) => void;
}

// Sentinel Select values. Radix Select rejects empty-string `value`, and we
// must distinguish "any" from "unlabeled" from a user-typed label.
const LABEL_ANY = "__all__";
const LABEL_NONE = "__none__";

export function AssetFilterBar({ value, labels, onChange }: AssetFilterBarProps) {
  const [localKeyword, setLocalKeyword] = useState(value.keyword);

  // Debounce keyword input by 300ms
  useEffect(() => {
    const timer = setTimeout(() => {
      if (localKeyword !== value.keyword) {
        onChange({ ...value, keyword: localKeyword });
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [localKeyword, value, onChange]);

  // Sync external keyword changes
  useEffect(() => {
    setLocalKeyword(value.keyword);
  }, [value.keyword]);

  const handleTypeChange = useCallback(
    (val: string) => {
      onChange({ ...value, type: val === "all" ? undefined : (val as AssetType) });
    },
    [value, onChange],
  );

  const handleSourceChange = useCallback(
    (val: string) => {
      onChange({ ...value, source: val === "all" ? undefined : (val as AssetSource) });
    },
    [value, onChange],
  );

  const handleLabelChange = useCallback(
    (val: string) => {
      let next: string | undefined;
      if (val === LABEL_ANY) next = undefined;
      else if (val === LABEL_NONE) next = "";
      else next = val;
      onChange({ ...value, label: next });
    },
    [value, onChange],
  );

  const labelSelectValue =
    value.label === undefined ? LABEL_ANY : value.label === "" ? LABEL_NONE : value.label;

  return (
    <div className="flex flex-wrap items-center gap-3">
      <Select
        value={value.type ?? "all"}
        onValueChange={handleTypeChange}
      >
        <SelectTrigger className="w-[100px]">
          <SelectValue placeholder="类型" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">全部类型</SelectItem>
          <SelectItem value="image">图片</SelectItem>
          <SelectItem value="video">视频</SelectItem>
        </SelectContent>
      </Select>

      <Select
        value={value.source ?? "all"}
        onValueChange={handleSourceChange}
      >
        <SelectTrigger className="w-[100px]">
          <SelectValue placeholder="来源" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">全部来源</SelectItem>
          <SelectItem value="imported">导入</SelectItem>
          <SelectItem value="generated">AI 生成</SelectItem>
        </SelectContent>
      </Select>

      <Select value={labelSelectValue} onValueChange={handleLabelChange}>
        <SelectTrigger className="w-[140px]">
          <SelectValue placeholder="标签" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={LABEL_ANY}>全部标签</SelectItem>
          <SelectItem value={LABEL_NONE}>无标签</SelectItem>
          {labels.map((l) => (
            <SelectItem key={l} value={l}>
              {l}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <div className="relative flex-1 min-w-[180px] max-w-[280px]">
        <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={localKeyword}
          onChange={(e) => setLocalKeyword(e.target.value)}
          placeholder="搜索素材..."
          className="pl-8"
        />
      </div>
    </div>
  );
}
