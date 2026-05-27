/**
 * Provider 白名单：在 AddAccountDialog 中需要展开 OSS 4 字段表单的 provider。
 *
 * 与 spec-16 一致：MS2 仅 `bailian` 需要把本地参考图上传到公网 HTTPS URL
 * 才能投喂给 happyhorse-1.0-r2v 这类视频模型。MS3+ 若有更多 provider 需要
 * OSS，可考虑迁移到 provider 表的 capabilities_json 字段。
 */
export const PROVIDERS_WITH_OSS = ["bailian"] as const;

export function providerNeedsOss(providerId: string): boolean {
  return (PROVIDERS_WITH_OSS as readonly string[]).includes(providerId);
}
