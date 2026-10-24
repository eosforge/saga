/** Own native close/background hooks and release asynchronous registrations after teardown. */
import { ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, type CloseRequestedEvent } from "@tauri-apps/api/window";
import { failure, flushReadingProgress, isGlobalBusy } from "./stores/library";

/** Blocks interaction while a normal close drains outstanding reading positions. */
export const closing = ref(false);

/** Register application-wide persistence hooks; returned cleanup also owns late registrations. */
export function register(): () => void {
  if (!isTauri()) return () => { };
  const host = getCurrentWindow();
  const listeners: (() => void)[] = [];
  let disposed = false;
  let requesting = false;

  /** Remove a native listener without leaving an asynchronous unsubscribe rejection unobserved. */
  function release(listener: () => void): void {
    void Promise.resolve(listener()).catch(() => { });
  }

  /** Keep the registration only while this application view still owns its callbacks. */
  function retain(listener: () => void): void {
    if (disposed) release(listener);
    else listeners.push(listener);
  }

  /** Request an early save; the store retains the position and publishes any write error. */
  function background(): void {
    if (!disposed) void flushReadingProgress().catch(() => { });
  }

  /** Visibility is also delivered for a minimized or suspended webview where focus may differ. */
  function visibility(): void {
    if (document.visibilityState === "hidden") background();
  }

  /** Prevent native destruction until the latest queued position has committed successfully. */
  async function close(event: CloseRequestedEvent): Promise<void> {
    event.preventDefault();
    if (disposed || requesting) return;
    if (isGlobalBusy.value) {
      failure.value = "书籍操作尚未完成，请稍后再关闭";
      return;
    }
    requesting = true;
    closing.value = true;
    try {
      await flushReadingProgress();
    } catch (error) {
      if (!disposed) failure.value = "关闭前保存失败：" + error + "；窗口已保留，请重试";
      requesting = false;
      if (!disposed) closing.value = false;
      return;
    }
    try { if (!disposed) await host.destroy(); }
    catch (error) { if (!disposed) failure.value = "关闭窗口失败：" + error + "；阅读进度已保存"; }
    finally {
      requesting = false;
      if (!disposed) closing.value = false;
    }
  }

  document.addEventListener("visibilitychange", visibility);
  window.addEventListener("pagehide", background);
  void host.onCloseRequested(close).then(retain).catch(error => {
    if (!disposed) failure.value = "无法启用关闭前保存：" + error;
  });
  void host.onFocusChanged(event => { if (!event.payload) background(); }).then(retain).catch(error => {
    if (!disposed) failure.value = "无法启用后台保存：" + error;
  });
  return () => {
    disposed = true;
    if (requesting) closing.value = false;
    document.removeEventListener("visibilitychange", visibility);
    window.removeEventListener("pagehide", background);
    listeners.forEach(release);
  };
}
