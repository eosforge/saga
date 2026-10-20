<script setup lang="ts">
/** Application navigation and global input ownership. */
import { ref, onMounted, onUnmounted } from "vue";
import ShelfView from "./views/Shelf.vue";
import ReaderView from "./views/Reader.vue";
import AuditView from "./views/Audit.vue";
import ProfileView from "./views/Profile.vue";
import SettingsView from "./views/Settings.vue";
import MiniBar from "./components/Bar.vue";
import { togglePlay, nextSentence, prevSentence, activeAuditReport, pause, hasAudio } from "./stores/library";

type TabType = "shelf" | "reader" | "audit" | "settings" | "profile";

const currentTab = ref<TabType>("shelf");
const isCollapsed = ref(false);
const historyStack = ref<TabType[]>(["shelf"]);
const historyIndex = ref(0);

function switchTab(tab: TabType, pushHistory = true): void {
  if (currentTab.value === tab) return;
  currentTab.value = tab;
  if (pushHistory) {
    historyStack.value = historyStack.value.slice(0, historyIndex.value + 1);
    historyStack.value.push(tab);
    historyIndex.value = historyStack.value.length - 1;
  }
}

function goBack(): void {
  if (historyIndex.value > 0) {
    historyIndex.value -= 1;
    currentTab.value = historyStack.value[historyIndex.value];
  } else if (currentTab.value !== "shelf") {
    switchTab("shelf", false);
  }
}

function goForward(): void {
  if (historyIndex.value < historyStack.value.length - 1) {
    historyIndex.value += 1;
    currentTab.value = historyStack.value[historyIndex.value];
  }
}

function handleMouseNavigation(e: MouseEvent): void {
  if (e.button === 3) {
    // Mouse Back (XButton1)
    e.preventDefault();
    e.stopPropagation();
    goBack();
  } else if (e.button === 4) {
    // Mouse Forward (XButton2)
    e.preventDefault();
    e.stopPropagation();
    goForward();
  }
}

function toggleSidebar(): void {
  isCollapsed.value = !isCollapsed.value;
}

function handleGlobalKeydown(e: KeyboardEvent): void {
  if (currentTab.value === "reader" || currentTab.value === "audit" || e.repeat || e.altKey || e.ctrlKey || e.metaKey) return;
  // Preserve native editing and focused control activation.
  const activeEl = document.activeElement;
  if (activeEl?.matches("input, textarea, select, button, a, [contenteditable=true]")) {
    return;
  }

  if (e.code === "Space") {
    if (!hasAudio.value) return;
    e.preventDefault();
    togglePlay();
  } else if (e.code === "ArrowRight") {
    nextSentence();
  } else if (e.code === "ArrowLeft") {
    prevSentence();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleGlobalKeydown);
  window.addEventListener("mouseup", handleMouseNavigation);

});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  window.removeEventListener("mouseup", handleMouseNavigation);
  pause();
});
</script>

<template>
  <div class="saga-app">
    <!-- Desktop Left Sidebar -->
    <aside class="sidebar" :class="{ collapsed: isCollapsed }">
      <div class="brand-block" title="点击折叠/展开侧边栏" @click="toggleSidebar">
        <img src="/logo.png" class="brand-logo" alt="Saga" />
        <span v-show="!isCollapsed" class="brand-title">Saga</span>
      </div>

      <nav class="nav-menu">
        <button
          class="nav-item"
          :class="{ active: currentTab === 'shelf' }"
          :title="isCollapsed ? '书架' : ''"
          @click="switchTab('shelf')"
        >
          <span class="nav-icon">📚</span>
          <span v-show="!isCollapsed" class="nav-label">书架</span>
        </button>

        <button
          v-if="activeAuditReport && activeAuditReport.anomalies.length > 0"
          class="nav-item audit-nav-btn"
          :class="{ active: currentTab === 'audit' }"
          :title="isCollapsed ? '核验' : ''"
          @click="switchTab('audit')"
        >
          <span class="nav-icon">🔍</span>
          <span v-show="!isCollapsed" class="nav-label">
            核验 ({{ activeAuditReport.anomalies.length }})
          </span>
        </button>

        <button
          class="nav-item"
          :class="{ active: currentTab === 'reader' }"
          :title="isCollapsed ? '阅读' : ''"
          @click="switchTab('reader')"
        >
          <span class="nav-icon">📖</span>
          <span v-show="!isCollapsed" class="nav-label">阅读</span>
        </button>

        <button
          class="nav-item"
          :class="{ active: currentTab === 'settings' }"
          :title="isCollapsed ? '设置' : ''"
          @click="switchTab('settings')"
        >
          <span class="nav-icon">⚙️</span>
          <span v-show="!isCollapsed" class="nav-label">设置</span>
        </button>

        <button
          class="nav-item"
          :class="{ active: currentTab === 'profile' }"
          :title="isCollapsed ? '我的' : ''"
          @click="switchTab('profile')"
        >
          <span class="nav-icon">👤</span>
          <span v-show="!isCollapsed" class="nav-label">我的</span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <span v-show="!isCollapsed" class="version-tag">v0.1.0</span>
      </div>
    </aside>

    <!-- Main Content Area -->
    <main class="content-stage">
      <div class="view-viewport">
        <ShelfView
          v-if="currentTab === 'shelf'"
          @open-reader="switchTab('reader')"
          @open-audit="switchTab('audit')"
        />
        <AuditView
          v-else-if="currentTab === 'audit'"
          @open-reader="switchTab('reader')"
          @back-to-shelf="switchTab('shelf')"
        />
        <ReaderView v-else-if="currentTab === 'reader'" @back-to-shelf="goBack" />
        <SettingsView v-else-if="currentTab === 'settings'" />
        <ProfileView v-else-if="currentTab === 'profile'" />
      </div>

      <!-- Persistent Mini Player Bar (shown in shelf & profile) -->
      <MiniBar v-if="currentTab !== 'reader' && currentTab !== 'audit'" @open-reader="switchTab('reader')" />
    </main>

    <!-- Mobile Adaptive Bottom TabBar (shown on narrow screens) -->
    <nav class="mobile-tabbar">
      <button
        class="tab-btn"
        :class="{ active: currentTab === 'shelf' }"
        @click="switchTab('shelf')"
      >
        <span class="tab-icon">📚</span>
        <span class="tab-text">书架</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'reader' }"
        @click="switchTab('reader')"
      >
        <span class="tab-icon">📖</span>
        <span class="tab-text">阅读</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'settings' }"
        @click="switchTab('settings')"
      >
        <span class="tab-icon">⚙️</span>
        <span class="tab-text">设置</span>
      </button>

      <button
        class="tab-btn"
        :class="{ active: currentTab === 'profile' }"
        @click="switchTab('profile')"
      >
        <span class="tab-icon">👤</span>
        <span class="tab-text">我的</span>
      </button>
    </nav>
  </div>
</template>

<style scoped>
.saga-app {
  display: flex;
  width: 100vw;
  height: 100vh;
  background: #0b1120;
  color: #f1f5f9;
  overflow: hidden;
}

/* Sidebar Styles */
.sidebar {
  width: 180px;
  background: #0f172a;
  border-right: 1px solid #1e293b;
  display: flex;
  flex-direction: column;
  padding: 20px 10px;
  flex-shrink: 0;
  user-select: none;
  transition: width 0.22s cubic-bezier(0.4, 0, 0.2, 1), padding 0.22s ease;
}

.sidebar.collapsed {
  width: 62px;
  padding: 20px 6px;
  align-items: center;
}

.brand-block {
  margin-bottom: 24px;
  padding: 6px 10px;
  border-radius: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 100%;
  transition: background-color 0.15s ease;
}

.brand-block:hover {
  background: rgba(255, 255, 255, 0.06);
}

.sidebar.collapsed .brand-block {
  padding: 6px 0;
  justify-content: center;
  width: 100%;
}

.brand-logo {
  width: 28px;
  height: 28px;
  object-fit: cover;
  filter: drop-shadow(0 0 6px rgba(56, 189, 248, 0.25));
  flex-shrink: 0;
}

.sidebar.collapsed .brand-logo {
  width: 32px;
  height: 32px;
}

.brand-title {
  font-size: 20px;
  font-weight: 800;
  letter-spacing: 0.04em;
  color: #f8fafc;
}

.nav-menu {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  width: 100%;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid transparent;
  color: #94a3b8;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: color 0.15s ease, background-color 0.15s ease, border-color 0.15s ease;
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 10px 0;
  width: 100%;
}

.nav-item:hover {
  color: #f1f5f9;
  background: rgba(30, 41, 59, 0.6);
}

.nav-item.active {
  background: #1e293b;
  color: #38bdf8;
  border-color: #334155;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
}

.audit-nav-btn {
  color: #f59e0b;
}

.audit-nav-btn:hover {
  background: rgba(245, 158, 11, 0.1);
  color: #fbbf24;
}

.audit-nav-btn.active {
  background: rgba(245, 158, 11, 0.15);
  border-color: #f59e0b;
  color: #fbbf24;
  box-shadow: 0 0 12px rgba(245, 158, 11, 0.25);
}

.nav-icon {
  font-size: 18px;
  flex-shrink: 0;
}

.nav-label {
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-footer {
  padding: 8px;
  display: flex;
  justify-content: center;
}

.version-tag {
  font-size: 11px;
  color: #475569;
  font-family: monospace;
}

/* Content Stage Styles */
.content-stage {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #0b1120;
  overflow: hidden;
}

.view-viewport {
  flex: 1;
  overflow: hidden;
}

/* Mobile TabBar */
.mobile-tabbar {
  display: none;
  height: 60px;
  background: #0f172a;
  border-top: 1px solid #1e293b;
  justify-content: space-around;
  align-items: center;
  flex-shrink: 0;
  user-select: none;
}

.tab-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  background: transparent;
  border: none;
  color: #94a3b8;
  gap: 3px;
  cursor: pointer;
  padding: 4px 16px;
}

.tab-btn.active {
  color: #38bdf8;
}

.tab-icon {
  font-size: 18px;
}

.tab-text {
  font-size: 11px;
  font-weight: 600;
}

@media (max-width: 640px) {
  .sidebar {
    display: none;
  }
  .mobile-tabbar {
    display: flex;
  }
}
</style>
