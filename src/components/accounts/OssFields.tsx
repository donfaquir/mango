import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

/**
 * 表单内部使用的 OSS 草稿状态，字段命名采用 camelCase 以贴合前端习惯。
 * 提交前由父组件转成后端 `OssConfigInput`（snake_case）。
 */
export interface OssDraft {
  endpoint: string;
  bucket: string;
  accessKeyId: string;
  accessKeySecret: string;
  region: string;
  urlExpiresSeconds: number;
}

export const EMPTY_OSS_DRAFT: OssDraft = {
  endpoint: "",
  bucket: "",
  accessKeyId: "",
  accessKeySecret: "",
  region: "",
  urlExpiresSeconds: 3600,
};

interface Props {
  value: OssDraft;
  onChange: (next: OssDraft) => void;
  disabled?: boolean;
  /** 当 provider 切换时父组件可以指明 secret 是否「视为已有」——本组件目前
   *  仅用于添加场景（编辑场景由 EditAccountDialog 处理），保留接口便于复用。 */
  secretPlaceholder?: string;
}

export function OssFields({
  value,
  onChange,
  disabled,
  secretPlaceholder,
}: Props) {
  const update = <K extends keyof OssDraft>(key: K, next: OssDraft[K]) =>
    onChange({ ...value, [key]: next });

  return (
    <div className="space-y-4 rounded-md border bg-muted/30 p-4">
      <div className="space-y-1">
        <p className="text-sm font-medium">OSS 配置（用于参考图上传）</p>
        <p className="text-xs text-muted-foreground">
          百炼 happyhorse 等视频模型要求参考图通过公网 HTTPS URL 提交，Mango
          会通过此 OSS 桶临时上传并在任务结束后清理。
        </p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="oss-endpoint">Endpoint *</Label>
        <Input
          id="oss-endpoint"
          value={value.endpoint}
          onChange={(e) => update("endpoint", e.target.value)}
          placeholder="oss-cn-hangzhou.aliyuncs.com"
          autoComplete="off"
          disabled={disabled}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="oss-bucket">Bucket *</Label>
        <Input
          id="oss-bucket"
          value={value.bucket}
          onChange={(e) => update("bucket", e.target.value)}
          placeholder="mango-ref-images"
          autoComplete="off"
          disabled={disabled}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="oss-ak-id">AccessKey ID *</Label>
        <Input
          id="oss-ak-id"
          value={value.accessKeyId}
          onChange={(e) => update("accessKeyId", e.target.value)}
          placeholder="LTAI5t..."
          autoComplete="off"
          disabled={disabled}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="oss-ak-secret">AccessKey Secret *</Label>
        <Input
          id="oss-ak-secret"
          type="password"
          value={value.accessKeySecret}
          onChange={(e) => update("accessKeySecret", e.target.value)}
          placeholder={secretPlaceholder ?? "OSS 子账号 SecretKey"}
          autoComplete="off"
          disabled={disabled}
        />
        <p className="text-xs text-muted-foreground">
          Secret 仅写入系统 keyring，与主 API Key 分两条 entry 存储。
        </p>
      </div>
    </div>
  );
}
