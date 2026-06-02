import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type WorkspaceProbe,
  type WorkspaceStatus,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const workspaceKeys = {
  status: ["workspace", "status"] as const,
};

/**
 * Read the current workspace mount state. Drives the app's top-level routing
 * decision (onboarding vs main app). `staleTime: Infinity` because the value
 * only changes via `set_workspace_and_relaunch`, which restarts the process
 * — there's nothing to refetch during a normal session.
 */
export function useWorkspaceStatus() {
  return useQuery<WorkspaceStatus>({
    queryKey: workspaceKeys.status,
    queryFn: () => unwrap(commands.getWorkspaceStatus()),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
}

/**
 * Classify a candidate workspace directory. The frontend uses the returned
 * variant to render the right confirmation copy before committing.
 */
export function useProbeWorkspace() {
  return useMutation<WorkspaceProbe, Error, string>({
    mutationFn: (path) => unwrap(commands.probeWorkspace(path)),
  });
}

/**
 * Hot-mount a workspace at the given path. Used for first-time onboarding
 * (no workspace is currently mounted). The backend installs MountedState
 * into its OnceLock and returns the new status; callers should invalidate
 * the workspace-status query so React Query re-fetches and the App routing
 * flips from onboarding to main.
 */
export function useMountWorkspace() {
  const qc = useQueryClient();
  return useMutation<WorkspaceStatus, Error, string>({
    mutationFn: (path) => unwrap(commands.mountWorkspace(path)),
    onSuccess: (status) => {
      qc.setQueryData(workspaceKeys.status, status);
    },
  });
}

/**
 * Persist the pointer config and schedule a clean exit; the user relaunches
 * into the new workspace. Used for *switching* from an already-mounted
 * workspace — hot-swap isn't feasible because the React Query cache and
 * router state still reference projects under the old workspace.
 *
 * The IPC response delivers before the exit fires (200 ms grace), so the
 * promise normally resolves to `void` and the caller can show a toast.
 */
export function useSetWorkspaceAndRelaunch() {
  return useMutation<void, Error, string>({
    mutationFn: async (path) => {
      await unwrap(commands.setWorkspaceAndRelaunch(path));
    },
  });
}
