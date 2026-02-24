# AIDI 悬浮球图标设计规格

## 设计概览

- **尺寸**: 120x120px
- **主色调**: 珊瑚红 (#FF6B6B)
- **风格**: 科技感、动态、渐变

## 颜色系统

```css
:root {
  /* 主色 */
  --aidi-coral: #FF6B6B;
  --aidi-coral-light: #FF8A80;
  --aidi-coral-dark: #E53935;

  /* 辅助色 */
  --aidi-white: #FFFFFF;
  --aidi-transparent: transparent;

  /* 阴影色 */
  --aidi-shadow: rgba(255, 107, 107, 0.6);
}
```

## CSS 实现代码

### 完整组件代码 (HTML + CSS)

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AIDI 悬浮球图标</title>
  <link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700;800&display=swap" rel="stylesheet">
  <style>
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }

    body {
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: 100vh;
      background: #1a1a1a;
    }

    /* 悬浮球容器 */
    .floating-ball {
      position: relative;
      width: 120px;
      height: 120px;
    }

    /* 外发光 */
    .outer-glow {
      position: absolute;
      width: 115px;
      height: 115px;
      left: -8px;
      top: 2px;
      border-radius: 50%;
      background: radial-gradient(
        circle at center,
        rgba(255, 107, 107, 0) 40%,
        rgba(255, 107, 107, 0.2) 70%,
        rgba(255, 107, 107, 0) 100%
      );
      opacity: 0.5;
    }

    /* 轨道1 - 外层 */
    .orbit-1 {
      position: absolute;
      width: 110px;
      height: 110px;
      left: 5px;
      top: 5px;
      border-radius: 50%;
      border: 2px solid transparent;
      background: conic-gradient(
        from 0deg,
        transparent 0deg,
        #FF6B6B 108deg,
        #FF8A80 180deg,
        transparent 360deg
      );
      -webkit-mask: radial-gradient(
        circle at center,
        transparent 90px,
        black 90px,
        black 92px,
        transparent 92px
      );
      animation: rotate 8s linear infinite;
    }

    /* 轨道2 - 内层 */
    .orbit-2 {
      position: absolute;
      width: 95px;
      height: 95px;
      left: 12.5px;
      top: 12.5px;
      border-radius: 50%;
      border: 1.5px solid transparent;
      background: conic-gradient(
        from 180deg,
        transparent 0deg,
        #FF8A80 120deg,
        #FFAAAA 168deg,
        transparent 360deg
      );
      -webkit-mask: radial-gradient(
        circle at center,
        transparent 83.5px,
        black 83.5px,
        black 85px,
        transparent 85px
      );
      animation: rotate 12s linear infinite reverse;
    }

    /* 球体基础 */
    .ball-base {
      position: absolute;
      width: 100px;
      height: 100px;
      left: 10px;
      top: 10px;
      border-radius: 50%;
      background: radial-gradient(
        ellipse at 30% 30%,
        #FF8A80 0%,
        #FF6B6B 50%,
        #E53935 100%
      );
      box-shadow:
        0 4px 20px rgba(255, 107, 107, 0.6),
        inset 0 -10px 30px rgba(0, 0, 0, 0.1);
    }

    /* 内部光晕 */
    .inner-glow {
      position: absolute;
      width: 80px;
      height: 80px;
      left: 20px;
      top: 15px;
      border-radius: 50%;
      background: radial-gradient(
        ellipse at 30% 30%,
        rgba(255, 255, 255, 1) 0%,
        rgba(255, 255, 255, 0) 100%
      );
      opacity: 0.6;
    }

    /* 高光 */
    .highlight {
      position: absolute;
      width: 30px;
      height: 20px;
      left: 25px;
      top: 20px;
      border-radius: 50%;
      background: radial-gradient(
        ellipse at center,
        rgba(255, 255, 255, 1) 0%,
        rgba(255, 255, 255, 0) 100%
      );
      opacity: 0.7;
    }

    /* 脉冲环 */
    .pulse-ring {
      position: absolute;
      width: 50px;
      height: 50px;
      left: 35px;
      top: 35px;
      border-radius: 50%;
      border: 1px solid rgba(255, 255, 255, 0.3);
      animation: pulse 2s ease-in-out infinite;
    }

    /* AIDI 文字 */
    .aidi-text {
      position: absolute;
      left: 50%;
      top: 50%;
      transform: translate(-50%, -50%);
      font-family: 'DM Sans', sans-serif;
      font-size: 22px;
      font-weight: 800;
      color: #FFFFFF;
      letter-spacing: 1px;
    }

    /* 装饰点 */
    .accent-dot {
      position: absolute;
      width: 4px;
      height: 4px;
      left: 91px;
      top: 52px;
      border-radius: 50%;
      background: radial-gradient(
        circle at center,
        #FFFFFF 0%,
        #FF8A80 100%
      );
    }

    /* 下划线 */
    .underline {
      position: absolute;
      width: 30px;
      height: 2px;
      left: 45px;
      top: 72px;
      border-radius: 1px;
      background: linear-gradient(
        to bottom,
        transparent 0%,
        rgba(255, 255, 255, 1) 50%,
        transparent 100%
      );
    }

    /* 科技线条 */
    .tech-line-1,
    .tech-line-2 {
      position: absolute;
      width: 15px;
      height: 1px;
      background: rgba(255, 255, 255, 0.6);
      border-radius: 1px;
    }

    .tech-line-1 {
      left: 35px;
      top: 38px;
    }

    .tech-line-2 {
      left: 70px;
      top: 38px;
    }

    /* 电路纹理 */
    .circuit-1,
    .circuit-2 {
      position: absolute;
      width: 10px;
      height: 10px;
      stroke: rgba(255, 255, 255, 0.4);
      stroke-width: 1;
      stroke-linecap: round;
      fill: none;
    }

    .circuit-1 {
      left: 15px;
      top: 55px;
    }

    .circuit-2 {
      left: 95px;
      top: 55px;
    }

    /* 装饰粒子 */
    .dot {
      position: absolute;
      border-radius: 50%;
    }

    .dot-1 {
      width: 6px;
      height: 6px;
      left: 95px;
      top: 25px;
      background: #FFFFFF;
    }

    .dot-2 {
      width: 4px;
      height: 4px;
      left: 25px;
      top: 95px;
      background: #FF8A80;
    }

    .dot-3 {
      width: 5px;
      height: 5px;
      left: 88px;
      top: 90px;
      background: #FFFFFF;
      opacity: 0.8;
    }

    /* 动画 */
    @keyframes rotate {
      from {
        transform: rotate(0deg);
      }
      to {
        transform: rotate(360deg);
      }
    }

    @keyframes pulse {
      0%, 100% {
        transform: scale(1);
        opacity: 0.3;
      }
      50% {
        transform: scale(1.05);
        opacity: 0.5;
      }
    }

    /* 悬停效果 */
    .floating-ball:hover .orbit-1 {
      animation-duration: 4s;
    }

    .floating-ball:hover .orbit-2 {
      animation-duration: 6s;
    }

    .floating-ball:hover .pulse-ring {
      animation-duration: 1s;
    }
  </style>
</head>
<body>
  <div class="floating-ball">
    <!-- 外发光 -->
    <div class="outer-glow"></div>

    <!-- 轨道 -->
    <div class="orbit-1"></div>
    <div class="orbit-2"></div>

    <!-- 球体 -->
    <div class="ball-base"></div>
    <div class="inner-glow"></div>
    <div class="highlight"></div>

    <!-- 脉冲环 -->
    <div class="pulse-ring"></div>

    <!-- 中心内容 -->
    <div class="aidi-text">AIDI</div>
    <div class="accent-dot"></div>
    <div class="underline"></div>

    <!-- 科技装饰 -->
    <div class="tech-line-1"></div>
    <div class="tech-line-2"></div>

    <!-- 电路纹理 -->
    <svg class="circuit-1" viewBox="0 0 10 10">
      <path d="M0 5 L5 5 L5 0" />
    </svg>
    <svg class="circuit-2" viewBox="0 0 10 10">
      <path d="M10 5 L5 5 L5 10" />
    </svg>

    <!-- 装饰粒子 -->
    <div class="dot dot-1"></div>
    <div class="dot dot-2"></div>
    <div class="dot dot-3"></div>
  </div>
</body>
</html>
```

## Vue 组件实现

```vue
<template>
  <div class="floating-ball" :style="{ width: `${size}px`, height: `${size}px` }">
    <!-- 外发光 -->
    <div class="outer-glow"></div>

    <!-- 轨道 -->
    <div class="orbit orbit-1"></div>
    <div class="orbit orbit-2"></div>

    <!-- 球体 -->
    <div class="ball-base"></div>
    <div class="inner-glow"></div>
    <div class="highlight"></div>

    <!-- 脉冲环 -->
    <div class="pulse-ring"></div>

    <!-- 中心内容 -->
    <div class="aidi-text">AIDI</div>
    <div class="accent-dot"></div>
    <div class="underline"></div>

    <!-- 科技装饰 -->
    <div class="tech-line-1"></div>
    <div class="tech-line-2"></div>

    <!-- 电路纹理 -->
    <svg class="circuit-1" viewBox="0 0 10 10">
      <path d="M0 5 L5 5 L5 0" />
    </svg>
    <svg class="circuit-2" viewBox="0 0 10 10">
      <path d="M10 5 L5 5 L5 10" />
    </svg>

    <!-- 装饰粒子 -->
    <div class="dot dot-1"></div>
    <div class="dot dot-2"></div>
    <div class="dot dot-3"></div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  size?: number
}>()
</script>

<style scoped>
/* 复制上面的CSS样式 */
</style>
```

## React 组件实现

```tsx
import React from 'react';
import './FloatingBall.css';

interface FloatingBallProps {
  size?: number;
  className?: string;
}

export const FloatingBall: React.FC<FloatingBallProps> = ({
  size = 120,
  className = ''
}) => {
  return (
    <div
      className={`floating-ball ${className}`}
      style={{ width: size, height: size }}
    >
      {/* 外发光 */}
      <div className="outer-glow" />

      {/* 轨道 */}
      <div className="orbit orbit-1" />
      <div className="orbit orbit-2" />

      {/* 球体 */}
      <div className="ball-base" />
      <div className="inner-glow" />
      <div className="highlight" />

      {/* 脉冲环 */}
      <div className="pulse-ring" />

      {/* 中心内容 */}
      <div className="aidi-text">AIDI</div>
      <div className="accent-dot" />
      <div className="underline" />

      {/* 科技装饰 */}
      <div className="tech-line-1" />
      <div className="tech-line-2" />

      {/* 电路纹理 */}
      <svg className="circuit-1" viewBox="0 0 10 10">
        <path d="M0 5 L5 5 L5 0" />
      </svg>
      <svg className="circuit-2" viewBox="0 0 10 10">
        <path d="M10 5 L5 5 L5 10" />
      </svg>

      {/* 装饰粒子 */}
      <div className="dot dot-1" />
      <div className="dot dot-2" />
      <div className="dot dot-3" />
    </div>
  );
};
```

## SVG 独立版本

如果需要使用纯SVG（适合作为应用图标）：

```svg
<svg width="120" height="120" viewBox="0 0 120 120" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <!-- 径向渐变 - 球体 -->
    <radialGradient id="ballGradient" cx="30%" cy="30%">
      <stop offset="0%" stop-color="#FF8A80"/>
      <stop offset="50%" stop-color="#FF6B6B"/>
      <stop offset="100%" stop-color="#E53935"/>
    </radialGradient>

    <!-- 径向渐变 - 内部光晕 -->
    <radialGradient id="innerGlow" cx="30%" cy="30%">
      <stop offset="0%" stop-color="#FFFFFF"/>
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>

    <!-- 径向渐变 - 高光 -->
    <radialGradient id="highlight">
      <stop offset="0%" stop-color="#FFFFFF"/>
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>

    <!-- 阴影 -->
    <filter id="shadow">
      <feDropShadow dx="0" dy="4" stdDeviation="10" flood-color="#FF6B6B" flood-opacity="0.6"/>
    </filter>
  </defs>

  <!-- 外发光 -->
  <circle cx="60" cy="60" r="57.5" fill="url(#outerGlow)" opacity="0.5"/>

  <!-- 球体 -->
  <circle cx="60" cy="60" r="50" fill="url(#ballGradient)" filter="url(#shadow)"/>

  <!-- 内部光晕 -->
  <circle cx="60" cy="55" r="40" fill="url(#innerGlow)" opacity="0.6"/>

  <!-- 高光 -->
  <ellipse cx="40" cy="30" rx="15" ry="10" fill="url(#highlight)" opacity="0.7"/>

  <!-- AIDI文字 -->
  <text x="60" y="65" text-anchor="middle" font-family="DM Sans" font-size="22" font-weight="800" fill="#FFFFFF">AIDI</text>
</svg>
```

## 动画说明

### 1. 旋转动画
- **轨道1**: 8秒完成一次旋转，顺时针
- **轨道2**: 12秒完成一次旋转，逆时针
- **悬停时**: 速度加倍，增强互动感

### 2. 脉冲动画
- **脉冲环**: 2秒周期，从1到1.05的缩放
- **透明度**: 在0.3到0.5之间变化
- **悬停时**: 周期缩短为1秒

### 3. 建议的额外动画
```css
/* 悬浮球整体呼吸效果 */
@keyframes breathe {
  0%, 100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.02);
  }
}

.floating-ball {
  animation: breathe 3s ease-in-out infinite;
}

/* 点击效果 */
.floating-ball:active {
  transform: scale(0.95);
  transition: transform 0.1s;
}
```

## 集成建议

### 1. 性能优化
- 使用 `will-change: transform` 优化动画性能
- 考虑使用 CSS 变量实现主题切换
- 在低端设备上禁用动画

### 2. 可访问性
```css
@media (prefers-reduced-motion: reduce) {
  .orbit-1,
  .orbit-2,
  .pulse-ring {
    animation: none;
  }
}
```

### 3. 响应式
```css
/* 小屏幕适配 */
@media (max-width: 480px) {
  .floating-ball {
    width: 80px;
    height: 80px;
  }

  .aidi-text {
    font-size: 16px;
  }
}
```

## 设计系统扩展

### 颜色变量
```css
/* 浅色主题 */
[data-theme="light"] {
  --ball-primary: #FF6B6B;
  --ball-secondary: #FF8A80;
}

/* 深色主题 */
[data-theme="dark"] {
  --ball-primary: #FF6B6B;
  --ball-secondary: #FF8A80;
}

/* 其他品牌色 */
[data-theme="blue"] {
  --ball-primary: #4A90E2;
  --ball-secondary: #7AADF2;
}
```

## 导出资源

### 1. 图标尺寸
- **16x16**: 系统托盘图标
- **32x32**: 小图标
- **48x48**: 标准图标
- **64x64**: 大图标
- **128x128**: 高清图标
- **256x256**: Retina 图标

### 2. 文件格式
- **SVG**: 矢量格式，可缩放
- **PNG**: 带透明背景的位图
- **ICO**: Windows 应用图标
- **ICNS**: macOS 应用图标

## 设计文件

设计源文件位于 `.pen` 格式，包含完整的矢量设计和动画参数。
