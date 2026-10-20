<script setup lang="ts">
/** Session metadata summaries without fabricated usage measurements. */
import { ref, computed } from "vue";
import { books } from "../stores/library";

const userName = ref("本地读者 (Local Reader)");

// Count persisted section metadata, without inventing storage or reading metrics.
const totalChapters = computed(() => books.value.reduce((count, book) => count + book.chapters, 0));
</script>

<template>
  <div class="profile-container">
    <header class="profile-header">
      <h1 class="page-title">个人中心</h1>
      <span class="status-badge">🟢 本地优先模式</span>
    </header>

    <!-- User Card -->
    <div class="user-card">
      <div class="user-avatar">
        <span>👤</span>
      </div>
      <div class="user-info">
        <h2 class="user-name">{{ userName }}</h2>
        <p class="user-desc">桌面端支持本地 TXT 阅读；语音伴读和跨设备同步尚未实现</p>
      </div>
    </div>

    <!-- Stats Grid -->
    <div class="stats-grid">
      <div class="stat-card">
        <span class="stat-label">书架藏书</span>
        <span class="stat-value">{{ books.length }} <small>本</small></span>
      </div>
      <div class="stat-card">
        <span class="stat-label">目录章节</span>
        <span class="stat-value">{{ totalChapters }} <small>章</small></span>
      </div>
      <div class="stat-card">
        <span class="stat-label">本地占用</span>
        <span class="stat-value">未测量</span>
      </div>
      <div class="stat-card">
        <span class="stat-label">缓存完整性巡检</span>
        <span class="stat-value active-cas">未实现</span>
      </div>
    </div>

    <!-- Reading History Section -->
    <div class="section-wrap">
      <h3 class="section-title">📖 会话书籍</h3>
      <div v-if="books.length > 0" class="history-list">
        <div v-for="b in books" :key="b.id" class="history-item">
          <div class="history-cover" :style="{ background: b.coverGradient }">
            <span>{{ b.title[0] || '书' }}</span>
          </div>
          <div class="history-info">
            <h4 class="history-title">{{ b.title }}</h4>
            <p class="history-sub">{{ b.author }} · {{ b.currentChapter }}</p>
          </div>
          <div class="history-meta">
            <span class="history-progress">{{ b.currentChapter }}</span>
            <span class="history-time">{{ b.lastRead }}</span>
          </div>
        </div>
      </div>
      <div v-else class="empty-history-box">
        <span class="empty-history-text">导入书籍后在此展示本次会话的章节信息，阅读进度尚未持久化</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.profile-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 32px 40px;
  overflow-y: auto;
  user-select: none;
}

.profile-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.page-title {
  font-size: 26px;
  font-weight: 700;
  color: #f8fafc;
}

.status-badge {
  font-size: 12px;
  color: #34d399;
  background: rgba(52, 211, 153, 0.1);
  border: 1px solid rgba(52, 211, 153, 0.3);
  padding: 4px 10px;
  border-radius: 12px;
  font-weight: 600;
}

.user-card {
  display: flex;
  align-items: center;
  gap: 20px;
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 14px;
  padding: 24px;
  margin-bottom: 24px;
}

.user-avatar {
  width: 60px;
  height: 60px;
  border-radius: 50%;
  background: #334155;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 28px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.user-name {
  font-size: 18px;
  font-weight: 700;
  color: #f8fafc;
  margin-bottom: 4px;
}

.user-desc {
  font-size: 13px;
  color: #94a3b8;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 32px;
}

.stat-card {
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 10px;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
}

.stat-label {
  font-size: 12px;
  color: #94a3b8;
  margin-bottom: 6px;
}

.stat-value {
  font-size: 22px;
  font-weight: 700;
  color: #f8fafc;
}

.stat-value small {
  font-size: 13px;
  font-weight: normal;
  color: #64748b;
}

.active-cas {
  color: #38bdf8;
  font-size: 18px;
}

.section-wrap {
  margin-bottom: 24px;
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: #cbd5e1;
  margin-bottom: 12px;
}

.history-list {
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 12px;
  overflow: hidden;
}

.history-item {
  display: flex;
  align-items: center;
  padding: 14px 20px;
  border-bottom: 1px solid #334155;
  gap: 14px;
}

.history-item:last-child {
  border-bottom: none;
}

.history-cover {
  width: 36px;
  height: 48px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-weight: 700;
  font-size: 15px;
  flex-shrink: 0;
}

.history-info {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.history-title {
  font-size: 14px;
  font-weight: 600;
  color: #f1f5f9;
}

.history-sub {
  font-size: 12px;
  color: #94a3b8;
  margin-top: 2px;
}

.history-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
}

.history-progress {
  font-size: 12px;
  color: #38bdf8;
  font-weight: 600;
}

.history-time {
  font-size: 11px;
  color: #64748b;
}

.empty-history-box {
  background: #1e293b;
  border: 1px dashed #334155;
  border-radius: 12px;
  padding: 32px 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.empty-history-text {
  font-size: 13px;
  color: #64748b;
}
</style>
