import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listApiAccounts = vi.fn();
const createApiAccount = vi.fn();
const deleteApiAccount = vi.fn();
const verifyApiAccountStorage = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listApiAccounts: (...args: unknown[]) => listApiAccounts(...args),
    createApiAccount: (...args: unknown[]) => createApiAccount(...args),
    deleteApiAccount: (...args: unknown[]) => deleteApiAccount(...args),
    verifyApiAccountStorage: (...args: unknown[]) =>
      verifyApiAccountStorage(...args),
  },
}));

import {
  useAccountList,
  useCreateAccount,
  useDeleteAccount,
  useVerifyApiAccountStorage,
} from "./useAccounts";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

const sampleAccount = {
  id: "a1",
  provider_id: "kling",
  label: "main",
  key_last4: "abcd",
  usage_quota: null,
  usage_used: 0,
  last_used_at: null,
  created_at: "2026-01-01",
};

describe("useAccountList", () => {
  beforeEach(() => {
    listApiAccounts.mockReset();
    createApiAccount.mockReset();
    deleteApiAccount.mockReset();
    verifyApiAccountStorage.mockReset();
  });

  it("fetches all accounts when no provider filter", async () => {
    listApiAccounts.mockResolvedValueOnce({
      status: "ok",
      data: [sampleAccount],
    });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useAccountList(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]?.id).toBe("a1");
    expect(listApiAccounts).toHaveBeenCalledWith(null);
  });

  it("scopes by provider when filter set", async () => {
    listApiAccounts.mockResolvedValueOnce({ status: "ok", data: [] });
    const { wrapper } = makeWrapper();
    renderHook(() => useAccountList("kling"), { wrapper });
    await waitFor(() => expect(listApiAccounts).toHaveBeenCalledTimes(1));
    expect(listApiAccounts).toHaveBeenCalledWith("kling");
  });

  it("never exposes api_key_ref on the returned shape", async () => {
    listApiAccounts.mockResolvedValueOnce({
      status: "ok",
      data: [sampleAccount],
    });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useAccountList(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]).not.toHaveProperty("api_key_ref");
    expect(result.current.data?.[0]).not.toHaveProperty("api_key");
  });
});

describe("useCreateAccount", () => {
  beforeEach(() => {
    listApiAccounts.mockReset();
    createApiAccount.mockReset();
  });

  it("invalidates the accounts list after success", async () => {
    createApiAccount.mockResolvedValueOnce({
      status: "ok",
      data: sampleAccount,
    });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["accounts"], [sampleAccount]);

    const { result } = renderHook(() => useCreateAccount(), { wrapper });
    await result.current.mutateAsync({
      provider_id: "kling",
      label: "main",
      api_key: "sk-xyz",
    });

    expect(queryClient.getQueryState(["accounts"])?.isInvalidated).toBe(true);
  });
});

describe("useDeleteAccount", () => {
  beforeEach(() => {
    deleteApiAccount.mockReset();
  });

  it("invalidates after delete", async () => {
    deleteApiAccount.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["accounts"], [sampleAccount]);

    const { result } = renderHook(() => useDeleteAccount(), { wrapper });
    await result.current.mutateAsync("a1");
    expect(queryClient.getQueryState(["accounts"])?.isInvalidated).toBe(true);
  });
});

describe("useVerifyApiAccountStorage", () => {
  beforeEach(() => {
    verifyApiAccountStorage.mockReset();
  });

  it("calls the verify command with the account id", async () => {
    verifyApiAccountStorage.mockResolvedValueOnce({
      status: "ok",
      data: null,
    });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useVerifyApiAccountStorage(), {
      wrapper,
    });
    await result.current.mutateAsync("a1");
    expect(verifyApiAccountStorage).toHaveBeenCalledWith("a1");
  });
});
