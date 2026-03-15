<template>
  <div class="keyboard-container">
    <div class="octave-row" v-for="(row, rowIndex) in keyLayout" :key="rowIndex">
      <div
          v-for="key in row"
          :key="key.note"
          class="piano-key"
          :class="{ 'is-active': engineStore.activeNotes.has(key.note) }"
          @mousedown="playManualNote(key.note)"
          @mouseup="stopManualNote(key.note)"
          @mouseleave="stopManualNote(key.note)"
      >
        <span class="key-label">{{ key.label }}</span>
        <span class="pc-key">{{ key.pcBind }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useEngineStore } from '../stores/engine'
import { invoke } from '@tauri-apps/api/core'
const engineStore = useEngineStore()
const playManualNote = async (note) => { engineStore.setNoteActive(note, true); await invoke('manual_note_on', { note: note }) }
const stopManualNote = async (note) => { if (!engineStore.activeNotes.has(note)) return; engineStore.setNoteActive(note, false); await invoke('manual_note_off', { note: note }) }

const keyLayout = [
  [ { note: 72, label: 'Do', pcBind: 'Q' }, { note: 74, label: 'Re', pcBind: 'W' }, { note: 76, label: 'Mi', pcBind: 'E' }, { note: 77, label: 'Fa', pcBind: 'R' }, { note: 79, label: 'So', pcBind: 'T' }, { note: 81, label: 'La', pcBind: 'Y' }, { note: 83, label: 'Ti', pcBind: 'U' } ],
  [ { note: 60, label: 'Do', pcBind: 'A' }, { note: 62, label: 'Re', pcBind: 'S' }, { note: 64, label: 'Mi', pcBind: 'D' }, { note: 65, label: 'Fa', pcBind: 'F' }, { note: 67, label: 'So', pcBind: 'G' }, { note: 69, label: 'La', pcBind: 'H' }, { note: 71, label: 'Ti', pcBind: 'J' } ],
  [ { note: 48, label: 'Do', pcBind: 'Z' }, { note: 50, label: 'Re', pcBind: 'X' }, { note: 52, label: 'Mi', pcBind: 'C' }, { note: 53, label: 'Fa', pcBind: 'V' }, { note: 55, label: 'So', pcBind: 'B' }, { note: 57, label: 'La', pcBind: 'N' }, { note: 59, label: 'Ti', pcBind: 'M' } ]
]
</script>

<style scoped>
.keyboard-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: linear-gradient(180deg, #1e1e1e 0%, #151515 100%);
  padding: 20px; /* 内边距减小 */
  border-radius: 12px;
  border: 2px solid #2a2a2a;
  border-bottom: 6px solid #111;
  box-shadow: 0 10px 30px rgba(0,0,0,0.6), inset 0 2px 0 rgba(255,255,255,0.05);
  width: 100%;
  box-sizing: border-box; /* 核心修复：防止内边距撑爆右侧屏幕 */
}

.octave-row {
  display: flex;
  justify-content: center;
  gap: 10px; /* 按键间距减小 */
}

/* 按键集体瘦身，节省垂直高度 */
.piano-key {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  background: linear-gradient(145deg, #2c2c2c, #202020);
  box-shadow: 4px 4px 8px #0a0a0a, -4px -4px 8px #2a2a2a;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: #777;
  transition: transform 0.05s ease, background 0.05s, box-shadow 0.05s;
  user-select: none;
  cursor: pointer;
}

.key-label { font-size: 11px; font-weight: bold; margin-bottom: 2px; }
.pc-key { font-size: 10px; opacity: 0.6; font-family: monospace; background: #111; padding: 2px 4px; border-radius: 3px; }

.piano-key.is-active {
  background: #4ade80;
  color: #000;
  box-shadow: 0 0 15px rgba(74, 222, 128, 0.6), inset 2px 2px 6px rgba(255,255,255,0.6);
  transform: translateY(3px) scale(0.96);
}
.piano-key.is-active .pc-key { background: rgba(0,0,0,0.3); color: #000; font-weight: bold; }
</style>