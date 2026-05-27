import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ApiAccount, Provider } from "@/lib/bindings/commands";

const listProvidersMock = vi.fn<
  () => Promise<{ status: "ok"; data: Provider[] }>
>();
const listAccountsMock = vi.fn<
  (providerId: string | null) => Promise<{ status: "ok"; data: ApiAccount[] }>
>();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listProviders: () => listProvidersMock(),
    listApiAccounts: (providerId: string | null) =>
      listAccountsMock(providerId),
  },
}));

import { ModelPicker, type ModelChoice } from "./ModelPicker";

function makeProvider(): Provider {
  return {
    id: "bailian",
    name: "阿里百炼",
    base_url: "https://dashscope.aliyuncs.com",
    auth_type: "bearer",
    docs_url: "",
  };
}

function makeAccount(id: string, label: string, last4 = "1234"): ApiAccount {
  return {
    id,
    provider_id: "bailian",
    label,
    key_last4: last4,
    oss_config: null,
    usage_quota: null,
    usage_used: 0,
    last_used_at: null,
    created_at: "2026-05-27 00:00:00",
  };
}

function renderPicker(props: {
  accounts: ApiAccount[];
  value?: ModelChoice | null;
  onChange?: (next: ModelChoice | null) => void;
}) {
  listProvidersMock.mockResolvedValue({
    status: "ok",
    data: [makeProvider()],
  });
  listAccountsMock.mockResolvedValue({ status: "ok", data: props.accounts });
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onChange = props.onChange ?? vi.fn();
  const utils = render(
    <QueryClientProvider client={qc}>
      <ModelPicker value={props.value ?? null} onChange={onChange} />
    </QueryClientProvider>,
  );
  return { ...utils, onChange };
}

describe("ModelPicker", () => {
  beforeEach(() => {
    listProvidersMock.mockReset();
    listAccountsMock.mockReset();
  });

  it("hides the account picker when only one account exists", async () => {
    renderPicker({
      accounts: [makeAccount("a1", "主账号")],
      value: {
        providerId: "bailian",
        modelId: "wan2.7-image-pro",
        taskType: "image",
        accountId: "a1",
      },
    });
    await waitFor(() => {
      expect(screen.getByText(/通义万相 2.7/)).toBeInTheDocument();
    });
    expect(screen.queryByText("使用账号")).not.toBeInTheDocument();
  });

  it("shows the account picker when multiple accounts share a provider", async () => {
    renderPicker({
      accounts: [
        makeAccount("a1", "主账号", "1111"),
        makeAccount("a2", "测试账号", "2222"),
      ],
      value: {
        providerId: "bailian",
        modelId: "wan2.7-image-pro",
        taskType: "image",
        accountId: "a2",
      },
    });
    await waitFor(() => {
      expect(screen.getByText("使用账号")).toBeInTheDocument();
    });
  });

  it("renders the empty state when no provider has an account", async () => {
    renderPicker({ accounts: [] });
    await waitFor(() => {
      expect(
        screen.getByText(/尚未配置任何 Provider 账号/),
      ).toBeInTheDocument();
    });
  });

  it("re-anchors a stale accountId to the first account of the provider", async () => {
    const onChange = vi.fn();
    renderPicker({
      accounts: [makeAccount("a1", "主账号"), makeAccount("a2", "备用账号")],
      value: {
        providerId: "bailian",
        modelId: "wan2.7-image-pro",
        taskType: "image",
        // Account that no longer exists — happens after account deletion.
        accountId: "deleted-account",
      },
      onChange,
    });
    await waitFor(() => {
      expect(onChange).toHaveBeenCalledWith({
        providerId: "bailian",
        modelId: "wan2.7-image-pro",
        taskType: "image",
        accountId: "a1",
      });
    });
  });
});
