/**
 * 悬浮球颜色主题配置
 * 被 App.vue 和 FloatingBall.vue 共享使用
 */

export interface ColorTheme {
  primary: string
  glow: string
}

export const colorThemes: Record<string, ColorTheme> = {
  'cyan-purple': { primary: '#667eea', glow: 'rgba(102, 126, 234, 0.4)' },
  'ocean': { primary: '#0052d4', glow: 'rgba(0, 82, 212, 0.4)' },
  'forest': { primary: '#11998e', glow: 'rgba(17, 153, 142, 0.4)' },
  'fire': { primary: '#f12711', glow: 'rgba(241, 39, 17, 0.4)' },
  'midnight': { primary: '#1a1a1a', glow: 'rgba(50, 50, 50, 0.4)' },
}
