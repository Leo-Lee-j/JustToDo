import { listen } from "@tauri-apps/api/event";
import { onScopeDispose } from "vue";

// One subscription per window. If a change arrives during a read, fetch again.
export function syncSnapshot(event: string, read: () => Promise<void>, onError: (error: unknown) => void) {
  let pending = false;
  let running: Promise<void> | undefined;
  let subscribed: Promise<void> | undefined;
  let unsubscribe: (() => void) | undefined;
  let timer: ReturnType<typeof setInterval> | undefined;
  let disposed = false;

  function refresh(): Promise<void> {
    pending = true;
    if (!running) {
      running = (async () => {
        try {
          while (pending) {
            pending = false;
            await read();
          }
        } catch (error) {
          onError(error);
        } finally {
          running = undefined;
        }
      })();
    }
    return running;
  }

  async function load() {
    subscribed ??= listen(event, () => void refresh()).then((stop) => {
      if (disposed) { stop(); return; }
      unsubscribe = stop;
      window.addEventListener("focus", onFocus);
      timer = setInterval(onFocus, 60_000);
    }).catch((error) => {
      subscribed = undefined;
      throw error;
    });
    try {
      await subscribed;
      await refresh();
    } catch (error) {
      onError(error);
    }
  }

  function onFocus() { void refresh(); }

  onScopeDispose(() => {
    disposed = true;
    unsubscribe?.();
    clearInterval(timer);
    window.removeEventListener("focus", onFocus);
  });

  return { load, refresh };
}
