<template>
  <div class="mixer-container">
    <div class="fixed-controls">
      <div class="header">
        <h2>音轨控制与引擎参数</h2>
      </div>

      <div class="controls-grid">
        <div class="control-panel">
          <span class="panel-label">降维算法：</span>
          <select class="strategy-select" v-model="engineStore.outOfBoundsStrategy" @change="updateStrategy">
            <option value="PureFold">🎵 纯净折叠 (零错音 - 推荐)</option>
            <option value="Consonance">💎 和声净化 (适合复杂和弦)</option>
            <option value="Drop">✂️ 严格丢弃越界音</option>
            <option value="AutoMelody">🌟 强制八度吸附 (易走调)</option>
            <option value="Original">🎹 原曲测试 (不降维)</option>
          </select>
        </div>

        <div class="control-panel">
          <span class="panel-label">手动移调：</span>
          <input type="range" min="-12" max="12" v-model.number="engineStore.globalTranspose" @input="updateTranspose" class="slider" style="min-width: 0;">
          <span class="transpose-value">{{ engineStore.globalTranspose > 0 ? '+' : '' }}{{ engineStore.globalTranspose }}</span>
          <div class="btn-group">
            <button class="tool-btn" @click="applyAutoTranspose" title="使用AI">Auto</button>
            <button class="tool-btn" @click="resetTranspose">归零</button>
          </div>
        </div>
      </div>
    </div>

    <div class="scrollable-track-area">
      <div class="track-list">
        <div v-for="track in engineStore.tracks" :key="track.channel" class="track-row" :class="{ muted: track.isMuted }">
          <div class="track-info">
            <span class="channel-badge">CH {{ track.channel.toString().padStart(2, '0') }}</span>
            <span class="instrument-name">{{ track.instrumentName }}</span>
          </div>
          <button class="mute-btn" :class="{ active: track.isMuted }" @click="engineStore.toggleMute(track.channel)">
            {{ track.isMuted ? '🔇 已静音' : '🔊 发声中' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useEngineStore } from '../stores/engine'
import { invoke } from '@tauri-apps/api/core'
const engineStore = useEngineStore()
const updateStrategy = async () => { try { await invoke('set_playback_strategy', { strategy: engineStore.outOfBoundsStrategy }) } catch (e) { console.error(e) } }
const updateTranspose = async () => { try { await invoke('set_global_transpose', { offset: engineStore.globalTranspose }) } catch (e) { console.error(e) } }
const applyAutoTranspose = () => { engineStore.globalTranspose = engineStore.suggestedTranspose; updateTranspose() }
const resetTranspose = () => { engineStore.globalTranspose = 0; updateTranspose() }
</script>

<style scoped>
/* 核心修复：让容器接管高度并成为 Flex */
.mixer-container { background: #121212; border-radius: 12px; border: 1px solid #2a2a2a; display: flex; flex-direction: column; height: 100%; box-sizing: border-box; overflow: hidden; }

/* 顶部控制区：不压缩 */
.fixed-controls { padding: 20px 20px 0 20px; flex-shrink: 0; }
.header { margin-bottom: 15px; border-bottom: 1px solid #2a2a2a; padding-bottom: 10px; }
.header h2 { margin: 0; font-size: 16px; color: #e5e5e5; }

/* 修复控制面板撑破：使用 box-sizing */
.controls-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 15px; margin-bottom: 15px; }
.control-panel { background: #1a1a1a; padding: 10px 12px; border-radius: 8px; display: flex; align-items: center; border: 1px solid #333; box-sizing: border-box; width: 100%; overflow: hidden;}
.panel-label { color: #888; font-size: 12px; margin-right: 8px; white-space: nowrap; font-weight: bold;}

.strategy-select { flex: 1; background: #111; color: #4ade80; border: 1px solid #333; padding: 4px 8px; border-radius: 4px; font-size: 12px; outline: none; min-width: 0; }
.slider { flex: 1; accent-color: #4ade80; cursor: pointer; }
.transpose-value { font-family: monospace; font-size: 14px; width: 24px; text-align: right; color: #4ade80; font-weight: bold; margin: 0 8px;}
.btn-group { display: flex; gap: 4px; }
.tool-btn { background: #222; border: 1px solid #444; color: #aaa; border-radius: 4px; padding: 4px 8px; cursor: pointer; font-size: 11px; white-space: nowrap;}
.tool-btn:hover { background: #444; color: white; }

/* 下部音轨区：接管剩余空间并独立滚动 */
.scrollable-track-area { flex: 1; overflow-y: auto; padding: 0 20px 20px 20px; }
.scrollable-track-area::-webkit-scrollbar { width: 6px; }
.scrollable-track-area::-webkit-scrollbar-thumb { background: #333; border-radius: 3px; }

.track-list { display: flex; flex-direction: column; gap: 8px; }
.track-row { display: flex; justify-content: space-between; align-items: center; background: #1a1a1a; padding: 10px 14px; border-radius: 8px; border: 1px solid #222; }
.track-row.muted { opacity: 0.3; filter: grayscale(100%); }
.channel-badge { background: #000; padding: 4px 6px; border-radius: 4px; font-size: 11px; margin-right: 10px; color: #888; font-family: monospace;}
.instrument-name { font-weight: 500; font-size: 12px;}
.mute-btn { background: #2a2a2a; color: #fff; border: 1px solid #444; padding: 5px 12px; border-radius: 4px; cursor: pointer; font-size: 11px; }
.mute-btn:hover { background: #3a3a3a; }
.mute-btn.active { background-color: #7f1d1d; color: #fca5a5; border-color: #991b1b; }
</style>